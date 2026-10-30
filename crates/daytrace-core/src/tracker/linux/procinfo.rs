use std::fs;

/// The executable behind a process. Fails quietly when the process belongs to
/// someone else or has exited, or when confinement forbids reading it.
pub fn executable_path(pid: u32) -> Option<String> {
    let path = fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let path = path.to_string_lossy();
    Some(path.strip_suffix(" (deleted)").unwrap_or(&path).to_string())
}

pub fn process_name(pid: u32) -> Option<String> {
    let name = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_our_own_process() {
        let pid = std::process::id();
        assert!(executable_path(pid).is_some());
        assert!(process_name(pid).is_some());
    }

    #[test]
    fn missing_processes_are_not_an_error() {
        assert_eq!(executable_path(u32::MAX), None);
        assert_eq!(process_name(u32::MAX), None);
    }
}
