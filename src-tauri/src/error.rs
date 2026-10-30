use daytrace_core::CoreError;

pub type CommandResult<T> = Result<T, String>;

pub fn to_message(error: CoreError) -> String {
    error.to_string()
}
