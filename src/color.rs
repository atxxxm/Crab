use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

// Включить/выключить цветной вывод (выставляется в main по tty/NO_COLOR/--no-color)
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

// Обернуть текст в ANSI-код, если цвет включён; иначе вернуть без изменений
pub fn paint(code: &str, text: &str) -> String {
    if enabled() {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}
