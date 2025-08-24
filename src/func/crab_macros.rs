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

#[macro_export]
macro_rules! crab_log {
    ($level:expr, $module:expr, $($arg:tt)*) => {{
        use chrono::Local;
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::path::PathBuf;

        let now = Local::now().format("%d-%m-%Y %H:%M:%S");
        let msg = format!("|{}| [{}] (Crab::{}) -> {}", now, $level, $module, format!($($arg)*));

        let path_to_log = PathBuf::from(crate::func::crab_config::CONFIG.build_dir).join(crate::func::crab_config::CONFIG.log);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path_to_log)
            .expect("Couldn't open the log file!");

        writeln!(file, "{}", msg).expect("Couldn't write the log to the file!");
    }};
}


