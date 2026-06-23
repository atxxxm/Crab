use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use rayon::prelude::*;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_log, crab_status};
use super::binary::{BuildProfile, CrabBuild};
use super::helpers::CrabBuildFunc;

pub struct CrabCheck;

impl Default for CrabCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabCheck {
    pub fn new() -> Self {
        CrabCheck
    }

    // Синтаксическая проверка всех исходников через -fsyntax-only.
    // Никаких .o-файлов и линковки — существенно быстрее полного build.
    pub fn check(&self, release: bool) -> std::io::Result<()> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang        = config.settings.lang.clone();
        let compiler    = config.settings.compiler.clone();
        let header_dir  = config.settings.header_dir.clone();

        let ext = if lang == "c" { "c" } else { "cpp" };
        let mut sources: Vec<String> = Vec::new();
        CrabBuildFunc::collect_file_with_extension(Path::new(&config.settings.source_dir), ext, &mut sources)?;
        sources.sort();

        if sources.is_empty() {
            crab_err!(ErrorKind::NotFound, "No source files found");
        }

        // Сторонние библиотеки нужны для -I флагов (иначе заголовки не найдутся)
        let find  = CrabFind::new(".").parsing_include()?;
        let build = CrabBuild::new();
        let inc_flags = if find { build.read_include_files_and_fmt()? } else { Vec::new() };

        let profile = if release { BuildProfile::Release } else { BuildProfile::Debug };
        let profile_flags: Vec<String> = profile.compile_flags().iter().map(|s| s.to_string()).collect();
        let user_flags = config.build.compile_args();

        let cbf    = CrabBuildFunc::new();
        let is_head = cbf.is_header()?;

        crab_status!("Checking", "{} v{} [{}]", config.project.name, config.project.version, profile.dir());
        crab_log!("INFO", "CHECK", "checking {} files", sources.len());
        let start = Instant::now();

        // Параллельная проверка; собираем весь вывод компилятора, чтобы не перемешивался
        let results: Vec<std::io::Result<(String, std::process::Output)>> = sources
            .par_iter()
            .map(|src| {
                let mut args = vec!["-fsyntax-only".to_string(), src.clone()];
                if is_head {
                    args.push(format!("-I{}", header_dir));
                }
                args.extend(inc_flags.clone());
                args.extend(profile_flags.clone());
                args.extend(user_flags.clone());

                let out = Command::new(&compiler).args(&args).output()?;
                Ok((src.clone(), out))
            })
            .collect();

        let mut had_error = false;
        for res in results {
            let (src, out) = res?;
            if !out.status.success() {
                had_error = true;
                crab_log!("ERROR", "CHECK", "syntax error in {}", src);
                // Вывод компилятора уже содержит имена файлов и номера строк
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
            }
        }

        if had_error {
            crab_err!(ErrorKind::Other, "check failed");
        }

        crab_status!("Finished", "checking {} in {:.2}s", config.project.name, start.elapsed().as_secs_f64());
        Ok(())
    }
}
