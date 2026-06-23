#[macro_export]
macro_rules! crab_err {
    ($kind:expr, $($arg:tt)*) => {
        return Err(std::io::Error::new($kind, format!($($arg)*)))
    };
}

#[macro_export]
macro_rules! crab_print {
    (red, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("31", &format!($($arg)*)));
    };
    (green, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("32", &format!($($arg)*)));
    };
    (yellow, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("33", &format!($($arg)*)));
    };
    (blue, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("34", &format!($($arg)*)));
    };
    (purple, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("35", &format!($($arg)*)));
    };
    (cyan, $($arg:tt)*) => {
        println!("{}", $crate::color::paint("36", &format!($($arg)*)));
    };
    ($($arg:tt)*) => {
        println!($($arg)*);
    };
}

// Статусная строка в стиле cargo: жирный зелёный глагол, выровненный по правому краю (12)
#[macro_export]
macro_rules! crab_status {
    ($verb:expr, $($arg:tt)*) => {
        println!("{} {}", $crate::color::paint("1;32", &format!("{:>12}", $verb)), format!($($arg)*));
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
