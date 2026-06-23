use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::build::{BuildProfile, CrabBuild};
use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_log, crab_print, crab_status};

pub struct CrabWatch;

impl Default for CrabWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabWatch {
    pub fn new() -> Self {
        CrabWatch
    }

    // Собирает карту path -> mtime для всех исходников/заголовков в указанных каталогах
    fn collect_mtimes(dirs: &[&str], exts: &[&str]) -> HashMap<String, SystemTime> {
        let mut map = HashMap::new();
        for dir in dirs {
            let path = Path::new(dir);
            if !path.exists() {
                continue;
            }
            for ext in exts {
                let mut files = Vec::new();
                let _ = CrabFind::collect_file_with_extension(path, ext, &mut files);
                for f in files {
                    if let Ok(m) = fs::metadata(&f).and_then(|m| m.modified()) {
                        map.insert(f, m);
                    }
                }
            }
        }
        map
    }

    // Возвращает список файлов, у которых изменился mtime или которые появились/исчезли
    fn changed_files(old: &HashMap<String, SystemTime>, new: &HashMap<String, SystemTime>) -> Vec<String> {
        let mut changed = Vec::new();

        for (path, mtime) in new {
            if old.get(path) != Some(mtime) {
                changed.push(path.clone());
            }
        }
        // удалённые файлы тоже считаются изменением
        for path in old.keys() {
            if !new.contains_key(path) {
                changed.push(path.clone());
            }
        }

        changed.sort();
        changed
    }

    pub fn watch(&self, release: bool) -> std::io::Result<()> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let source_dir = config.settings.source_dir.clone();
        let header_dir = config.settings.header_dir.clone();
        let lang       = config.settings.lang.clone();

        let exts: &[&str] = if lang == "c" {
            &["c", "h"]
        } else {
            &["cpp", "cc", "cxx", "c", "hpp", "hh", "h"]
        };

        let profile = if release { BuildProfile::Release } else { BuildProfile::Debug };

        crab_status!("Watching", "{} {} (Ctrl+C to stop)", source_dir, header_dir);

        // Начальная сборка
        if let Err(e) = CrabBuild::new().building(profile, None, None) {
            crab_print!(red, "error: {}", e);
        }

        let mut prev = Self::collect_mtimes(&[&source_dir, &header_dir], exts);

        loop {
            std::thread::sleep(Duration::from_millis(500));

            let curr = Self::collect_mtimes(&[&source_dir, &header_dir], exts);
            let changed = Self::changed_files(&prev, &curr);

            if !changed.is_empty() {
                for f in &changed {
                    crab_status!("Changed", "{}", f);
                }
                crab_log!("INFO", "WATCH", "changed: {:?}", changed);

                prev = curr;

                if let Err(e) = CrabBuild::new().building(profile, None, None) {
                    crab_print!(red, "error: {}", e);
                    // не выходим — продолжаем смотреть, чтобы пользователь мог исправить ошибку
                }
            } else {
                prev = curr;
            }
        }
    }
}
