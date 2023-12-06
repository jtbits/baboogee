#[macro_export]
macro_rules! log {
    ($print_macro:tt, $log_level:expr, $($arg:tt)*) => {
        $print_macro!("{}[{}:{}]: {}", $log_level, file!(), line!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        log!(println, "INFO", $($arg)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        log!(eprintln, "ERROR", $($arg)*)
    };
}
