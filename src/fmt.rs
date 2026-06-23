use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};

use rayon::prelude::*;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_print, crab_log};
use std::io::ErrorKind;

pub struct CrabFmt;

impl Default for CrabFmt {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabFmt {
    pub fn new() -> Self {
        CrabFmt
    }

    // Проверка, что clang-format установлен и доступен
    fn ensure_clang_format(&self) -> std::io::Result<()> {
        crab_log!("INFO", "FMT", "Checking for clang-format");
        let ok = Command::new("clang-format")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !ok {
            crab_log!("ERROR", "FMT", "clang-format not found");
            crab_err!(
                ErrorKind::NotFound,
                "clang-format not found. Install LLVM/clang-format and make sure it is in PATH."
            );
        }

        Ok(())
    }

    // Сбор файлов C/C++ из каталогов исходников и заголовков
    fn collect_files(&self) -> std::io::Result<Vec<String>> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let dirs = [config.settings.source_dir, config.settings.header_dir];
        let exts = ["c", "cc", "cpp", "cxx", "h", "hh", "hpp", "hxx"];

        let mut set: HashSet<String> = HashSet::new();
        for dir in &dirs {
            let path = Path::new(dir);
            for ext in &exts {
                let mut found = Vec::new();
                CrabFind::collect_file_with_extension(path, ext, &mut found)?;
                set.extend(found);
            }
        }

        let mut files: Vec<String> = set.into_iter().collect();
        files.sort();
        Ok(files)
    }

    // Запуск clang-format. check = только проверка (без изменений),
    // style = переопределение стиля (по умолчанию clang-format уважает .clang-format)
    pub fn fmt(&self, check: bool, style: Option<&str>) -> std::io::Result<()> {
        self.ensure_clang_format()?;

        let files = self.collect_files()?;

        if files.is_empty() {
            crab_print!(yellow, "No source files to format");
            return Ok(());
        }

        if check {
            crab_print!(cyan, "Checking formatting:\n");
        } else {
            crab_print!(cyan, "Formatting:\n");
        }
        crab_log!("INFO", "FMT", "Running clang-format on {} file(s), check={}", files.len(), check);

        let results: Vec<(String, bool)> = files
            .par_iter()
            .map(|f| {
                let mut cmd = Command::new("clang-format");

                if let Some(s) = style {
                    cmd.arg(format!("--style={}", s));
                }

                if check {
                    cmd.args(["--dry-run", "-Werror"]);
                } else {
                    cmd.arg("-i");
                }

                let ok = cmd
                    .arg(f)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                (f.clone(), ok)
            })
            .collect();

        if check {
            let mut unformatted: Vec<&String> =
                results.iter().filter(|(_, ok)| !ok).map(|(f, _)| f).collect();
            unformatted.sort();

            if unformatted.is_empty() {
                crab_print!(green, "All files are formatted correctly");
                return Ok(());
            }

            for f in &unformatted {
                crab_print!(red, "needs formatting: {}", f);
            }
            crab_log!("WARNING", "FMT", "{} file(s) need formatting", unformatted.len());
            crab_err!(ErrorKind::Other, "{} file(s) need formatting", unformatted.len());
        } else {
            for (f, ok) in &results {
                if *ok {
                    crab_print!(green, "+ {}", f);
                } else {
                    crab_print!(red, "failed: {}", f);
                }
            }
            crab_print!(green, "\nDone! ({} file(s))", results.len());
            Ok(())
        }
    }
}
