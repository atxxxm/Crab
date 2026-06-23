#[macro_export]
macro_rules! crab_err {
    ($kind:expr, $($arg:tt)*) => {
        return Err(std::io::Error::new($kind, format!($($arg)*)))
    };
}

#[macro_export]
macro_rules! crab_print {
    (red, $($arg:tt)*) => {
        println!("\x1b[31m{}\x1b[0m", format!($($arg)*));
    };
    (green, $($arg:tt)*) => {
        println!("\x1b[32m{}\x1b[0m", format!($($arg)*));
    };
    (yellow, $($arg:tt)*) => {
        println!("\x1b[33m{}\x1b[0m", format!($($arg)*));
    };
    (blue, $($arg:tt)*) => {
        println!("\x1b[34m{}\x1b[0m", format!($($arg)*));
    };
    (purple, $($arg:tt)*) => {
        println!("\x1b[35m{}\x1b[0m", format!($($arg)*));
    };
    (cyan, $($arg:tt)*) => {
        println!("\x1b[36m{}\x1b[0m", format!($($arg)*));
    };
    ($($arg:tt)*) => {
        println!($($arg)*);
    };
}

// Статусная строка в стиле cargo: жирный зелёный глагол, выровненный по правому краю (12)
#[macro_export]
macro_rules! crab_status {
    ($verb:expr, $($arg:tt)*) => {
        println!("\x1b[1;32m{:>12}\x1b[0m {}", $verb, format!($($arg)*));
    };
}

#[macro_export]
macro_rules! crab_log {
    ($level:expr, $module:expr, $($arg:tt)*) => {{
        // Логирование опционально и нефатально: форматируем и пишем только когда включено
        if $crate::log::is_enabled() {
            $crate::log::write($level, $module, &format!($($arg)*));
        }
    }};
}
