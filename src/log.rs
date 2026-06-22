use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Local;

use crate::config::CONFIG;

static ENABLED: AtomicBool = AtomicBool::new(false);

// Включить/выключить запись лога (вызывается из main по флагу -v / env CRAB_LOG)
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

// Запись строки в лог. Полностью нефатальна: любые ошибки ввода-вывода
// (нет каталога crb/, нет прав и т.п.) тихо игнорируются и не роняют программу.
pub fn write(level: &str, module: &str, message: &str) {
    let now = Local::now().format("%d-%m-%Y %H:%M:%S");
    let line = format!("|{}| [{}] (Crab::{}) -> {}", now, level, module, message);

    let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.log);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", line);
    }
}
