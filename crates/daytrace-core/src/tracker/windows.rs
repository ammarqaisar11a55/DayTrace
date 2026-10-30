//! Windows 10 and 11, through Win32. No administrator rights are needed.
//!
//! A hidden top-level window receives session and power broadcasts. It cannot
//! be a message-only window, because those do not receive broadcasts.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

use log::debug;
use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DispatchMessageW, EnumChildWindows, GetForegroundWindow,
    GetIconInfo, GetMessageW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, RegisterClassW,
    TranslateMessage, CW_USEDEFAULT, EVENT_SYSTEM_FOREGROUND, ICONINFO, MSG, PBT_APMRESUMEAUTOMATIC,
    PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_ENDSESSION,
    WM_POWERBROADCAST, WM_QUERYENDSESSION, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_OVERLAPPED,
};

use super::{
    ActivityTracker, Capabilities, Capability, EventSink, Foreground, ForegroundApp, Sample, SampleOptions,
    SystemEvent,
};

const WTS_SESSION_LOCK: usize = 0x7;
const WTS_SESSION_UNLOCK: usize = 0x8;

struct Listener {
    sink: EventSink,
    locked: Arc<AtomicBool>,
}

/// Win32 callbacks take no user data here, so the single listener lives in a
/// process-wide cell.
static LISTENER: OnceLock<Listener> = OnceLock::new();

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(listener) = LISTENER.get() {
        match message {
            WM_WTSSESSION_CHANGE => match wparam.0 {
                WTS_SESSION_LOCK => {
                    listener.locked.store(true, Ordering::SeqCst);
                    listener.sink.send(SystemEvent::Lock);
                }
                WTS_SESSION_UNLOCK => {
                    listener.locked.store(false, Ordering::SeqCst);
                    listener.sink.send(SystemEvent::Unlock);
                }
                _ => {}
            },
            WM_POWERBROADCAST => match wparam.0 as u32 {
                PBT_APMSUSPEND => {
                    listener.sink.send(SystemEvent::Suspend);
                }
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                    listener.sink.send(SystemEvent::Resume);
                }
                _ => {}
            },
            WM_ENDSESSION if wparam.0 != 0 => {
                listener.sink.send(SystemEvent::Shutdown);
            }
            WM_QUERYENDSESSION => return LRESULT(1),
            _ => {}
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

unsafe extern "system" fn on_foreground_change(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(listener) = LISTENER.get() {
        listener.sink.send(SystemEvent::FocusChanged);
    }
}

/// Creates the hidden window and runs its message loop. Returns once the
/// session is ending.
fn run_listener() -> windows::core::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null())?;
        let class_name = w!("DayTraceTrackerWindow");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&class);

        let window = CreateWindowExW(
            Default::default(),
            class_name,
            w!("DayTrace"),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION)?;
        let _hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None::<HMODULE>,
            Some(on_foreground_change),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn process_id(window: HWND) -> Option<u32> {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

fn executable_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut length = buffer.len() as u32;
        let result =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length);
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}

/// Reads the product's `FileDescription` from its version resource, which is
/// the name users recognize, such as "Visual Studio Code" for `Code.exe`.
fn file_description(path: &str) -> Option<String> {
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), None, size, data.as_mut_ptr().cast()).ok()?;

        let mut translation = std::ptr::null_mut();
        let mut length = 0;
        let root: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(Some(0)).collect();
        VerQueryValueW(data.as_ptr().cast(), PCWSTR(root.as_ptr()), &mut translation, &mut length)
            .ok()
            .ok()?;
        if length < 4 {
            return None;
        }
        let codes = translation as *const u16;
        let (language, codepage) = (*codes, *codes.add(1));

        let key = format!("\\StringFileInfo\\{language:04x}{codepage:04x}\\FileDescription");
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let mut value = std::ptr::null_mut();
        let mut characters = 0;
        VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut value, &mut characters).ok().ok()?;
        if characters == 0 {
            return None;
        }
        let text = std::slice::from_raw_parts(value as *const u16, characters as usize);
        let name = String::from_utf16_lossy(text).trim_end_matches('\0').trim().to_string();
        (!name.is_empty()).then_some(name)
    }
}

/// The executable's large shell icon, encoded as a PNG.
pub fn executable_icon_png(path: &str) -> Option<Vec<u8>> {
    const SIZE: i32 = 32;
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut file_info = SHFILEINFOW::default();

    unsafe {
        let found = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut file_info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if found == 0 || file_info.hIcon.is_invalid() {
            return None;
        }

        let mut icon = ICONINFO::default();
        let info = GetIconInfo(file_info.hIcon, &mut icon);
        let _ = DestroyIcon(file_info.hIcon);
        info.ok()?;

        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
        let context = CreateCompatibleDC(None);
        let lines = GetDIBits(
            context,
            icon.hbmColor,
            0,
            SIZE as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(context);
        let _ = DeleteObject(icon.hbmColor.into());
        let _ = DeleteObject(icon.hbmMask.into());
        if lines == 0 {
            return None;
        }

        // Windows stores pixels as BGRA.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let mut output = Vec::new();
        let mut encoder = png::Encoder::new(&mut output, SIZE as u32, SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        writer.finish().ok()?;
        Some(output)
    }
}

fn window_title(window: HWND) -> Option<String> {
    unsafe {
        let length = GetWindowTextLengthW(window);
        if length <= 0 {
            return None;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(window, &mut buffer);
        (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
    }
}

struct HostedChild {
    host_pid: u32,
    child_pid: Option<u32>,
}

unsafe extern "system" fn find_hosted_child(window: HWND, data: LPARAM) -> BOOL {
    let search = unsafe { &mut *(data.0 as *mut HostedChild) };
    match process_id(window) {
        Some(pid) if pid != search.host_pid => {
            search.child_pid = Some(pid);
            BOOL(0)
        }
        _ => BOOL(1),
    }
}

/// Store applications run inside `ApplicationFrameHost.exe`, and the real
/// process owns a child window of the frame.
fn resolve_hosted_process(window: HWND, pid: u32) -> u32 {
    let mut search = HostedChild { host_pid: pid, child_pid: None };
    unsafe {
        let _ = EnumChildWindows(
            Some(window),
            Some(find_hosted_child),
            LPARAM(&mut search as *mut HostedChild as isize),
        );
    }
    search.child_pid.unwrap_or(pid)
}

fn is_frame_host(path: &str) -> bool {
    Path::new(path).file_name().is_some_and(|name| name.eq_ignore_ascii_case("ApplicationFrameHost.exe"))
}

fn friendly_exe_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn idle_ms() -> Option<i64> {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        GetLastInputInfo(&mut info).ok().ok()?;
        // Both counters wrap every 49.7 days, so subtract as 32-bit values.
        Some(GetTickCount().wrapping_sub(info.dwTime) as i64)
    }
}

pub struct WindowsTracker {
    locked: Arc<AtomicBool>,
    listening: bool,
}

impl WindowsTracker {
    pub fn new(sink: EventSink) -> Self {
        let locked = Arc::new(AtomicBool::new(false));
        let listener = Listener { sink, locked: locked.clone() };
        let listening = LISTENER.set(listener).is_ok()
            && thread::Builder::new()
                .name("daytrace-win32".into())
                .spawn(|| {
                    if let Err(error) = run_listener() {
                        debug!("Windows event listener stopped: {error}");
                    }
                })
                .is_ok();
        Self { locked, listening }
    }

    fn foreground(&self, include_title: bool) -> Foreground {
        let window = unsafe { GetForegroundWindow() };
        if window.0.is_null() {
            return Foreground::NoFocus;
        }
        let Some(mut pid) = process_id(window) else { return Foreground::Unknown };

        // Elevated and protected processes refuse to be opened. The application
        // is then unknown rather than guessed.
        let Some(mut path) = executable_path(pid) else { return Foreground::Unknown };
        if is_frame_host(&path) {
            pid = resolve_hosted_process(window, pid);
            match executable_path(pid) {
                Some(child_path) => path = child_path,
                None => return Foreground::Unknown,
            }
        }

        Foreground::App(ForegroundApp {
            identifier: path.to_lowercase(),
            name: file_description(&path).unwrap_or_else(|| friendly_exe_name(&path)),
            icon_path: Some(path.clone()),
            executable_path: Some(path),
            window_title: if include_title { window_title(window) } else { None },
        })
    }
}

impl ActivityTracker for WindowsTracker {
    fn capabilities(&mut self) -> Capabilities {
        let events = if self.listening {
            Capability::available()
        } else {
            Capability::limited("System events could not be subscribed to.")
        };
        Capabilities {
            platform: "windows".to_string(),
            display_server: "win32".to_string(),
            desktop: "windows".to_string(),
            backend: "win32".to_string(),
            foreground_app: Capability::limited(
                "Applications running as administrator may be reported as unidentified.",
            ),
            executable_path: Capability::available(),
            window_title: Capability::available(),
            idle: Capability::available(),
            lock: events.clone(),
            suspend: events,
            integration: None,
        }
    }

    fn sample(&mut self, options: SampleOptions) -> Sample {
        Sample {
            foreground: self.foreground(options.include_window_title),
            idle_ms: idle_ms(),
            locked: self.locked.load(Ordering::SeqCst),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_host_is_recognized_in_any_case() {
        assert!(is_frame_host(r"C:\Windows\System32\applicationframehost.exe"));
        assert!(!is_frame_host(r"C:\Program Files\App\App.exe"));
    }

    #[test]
    fn executable_names_drop_the_extension() {
        assert_eq!(friendly_exe_name(r"C:\Tools\notepad++.exe"), "notepad++");
    }

    #[test]
    fn own_process_can_be_inspected() {
        let path = executable_path(std::process::id()).unwrap();
        assert!(path.to_lowercase().ends_with(".exe"));
    }

    #[test]
    fn idle_time_is_available() {
        assert!(idle_ms().is_some());
    }
}
