use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_log, crab_status};
use super::binary::{BuildProfile, CrabBuild};
use super::helpers::CrabBuildFunc;

pub struct CrabTest;

impl Default for CrabTest {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabTest {
    pub fn new() -> Self {
        CrabTest
    }

    // Сборка и запуск тестов из каталога tests/ (или настроенного в [test].dir).
    // filter — опциональная подстрока: запускать только тесты, в имени файла которых она есть.
    pub fn run_tests(&self, filter: Option<&str>, release: bool) -> std::io::Result<()> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let test_dir    = config.test.dir.clone();
        let lang        = config.settings.lang.clone();
        let compiler    = config.settings.compiler.clone();
        let header_dir  = config.settings.header_dir.clone();

        crab_log!("INFO", "TEST", "test dir: {}, lang: {}", test_dir, lang);

        // Собираем тестовые файлы
        let ext = if lang == "c" { "c" } else { "cpp" };
        let test_path = Path::new(&test_dir);

        if !test_path.exists() {
            crab_err!(ErrorKind::NotFound,
                "Test directory '{}' not found. Create it and add test files.", test_dir);
        }

        let mut test_files: Vec<String> = Vec::new();
        CrabBuildFunc::collect_file_with_extension(test_path, ext, &mut test_files)?;
        test_files.sort();

        if let Some(f) = filter {
            test_files.retain(|t| {
                Path::new(t).file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.contains(f))
            });
        }

        if test_files.is_empty() {
            match filter {
                Some(f) => crab_err!(ErrorKind::NotFound, "No tests matching '{}'", f),
                None    => crab_err!(ErrorKind::NotFound, "No test files found in '{}'", test_dir),
            }
        }

        // Инкрементально собираем основной проект (объектные файлы должны быть актуальны)
        let profile = if release { BuildProfile::Release } else { BuildProfile::Debug };
        CrabBuild::new().building(profile.clone(), None, None)?;

        // Каталоги с .o файлами основного проекта
        let obj_dir = PathBuf::from(CONFIG.build_dir).join(profile.dir()).join(CONFIG.object_dir);

        // Объектные файлы проекта без main.o (чтобы не было конфликта символа main)
        let mut project_objs: Vec<String> = Vec::new();
        if obj_dir.exists() {
            for entry in fs::read_dir(&obj_dir)? {
                let entry = entry?;
                let path  = entry.path();
                if path.extension().is_some_and(|e| e == "o") {
                    let stem = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if stem != "main" {
                        project_objs.push(path.display().to_string());
                    }
                }
            }
        }

        // Флаги для сторонних библиотек (из детекта)
        let build     = CrabBuild::new();
        let is_find   = CrabFind::new(".").parsing_include()?;
        let inc_flags = if is_find { build.read_include_files_and_fmt()? } else { Vec::new() };
        let (lib_paths, lib_names) = if is_find { build.read_lib_path_and_fmt()? } else { (Vec::new(), Vec::new()) };

        let cbf = CrabBuildFunc::new();
        let is_head = cbf.is_header()?;

        let profile_cflags: Vec<String> = profile.compile_flags().iter().map(|s| s.to_string()).collect();
        let profile_lflags: Vec<String> = profile.link_flags().iter().map(|s| s.to_string()).collect();
        let user_compile = config.build.compile_args();
        let user_link    = config.build.link_args();

        // Создаём каталоги для тестовых бинарников
        let test_out = PathBuf::from(CONFIG.build_dir).join("test");
        let test_obj = test_out.join("obj");
        let test_bin = test_out.join("bin");
        fs::create_dir_all(&test_obj)?;
        fs::create_dir_all(&test_bin)?;

        let exe_suffix = std::env::consts::EXE_SUFFIX;
        let start      = Instant::now();
        let mut passed = 0usize;
        let mut failed = 0usize;

        crab_status!("Running", "{} test file(s)", test_files.len());

        for tf in &test_files {
            let stem = Path::new(tf)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("test");

            let obj_path = test_obj.join(format!("{}.o", stem));
            let bin_path = test_bin.join(format!("{}{}", stem, exe_suffix));

            // Компилируем тестовый файл в объектный
            let mut cargs: Vec<String> = vec![
                "-c".to_string(),
                tf.clone(),
                "-o".to_string(),
                obj_path.display().to_string(),
            ];
            if is_head {
                cargs.push(format!("-I{}", header_dir));
            }
            cargs.extend(inc_flags.clone());
            cargs.extend(profile_cflags.clone());
            cargs.extend(user_compile.clone());

            let cout = Command::new(&compiler).args(&cargs).output()?;
            if !cout.status.success() {
                println!("  test {} ... {}", tf, crate::color::paint("31", "FAILED (compile error)"));
                eprint!("{}", String::from_utf8_lossy(&cout.stderr));
                failed += 1;
                continue;
            }

            // Линкуем тест + объектные файлы проекта
            let mut largs: Vec<String> = vec![obj_path.display().to_string()];
            largs.extend(project_objs.clone());
            largs.push("-o".to_string());
            largs.push(bin_path.display().to_string());
            largs.extend(profile_lflags.clone());
            largs.extend(lib_paths.clone());
            largs.extend(lib_names.clone());
            largs.extend(user_link.clone());

            let lout = Command::new(&compiler).args(&largs).output()?;
            if !lout.status.success() {
                println!("  test {} ... {}", tf, crate::color::paint("31", "FAILED (link error)"));
                eprint!("{}", String::from_utf8_lossy(&lout.stderr));
                failed += 1;
                continue;
            }

            // Запускаем тест и смотрим на код выхода
            let rout = Command::new(&bin_path).output()?;
            if rout.status.success() {
                println!("  test {} ... {}", tf, crate::color::paint("32", "ok"));
                passed += 1;
            } else {
                println!("  test {} ... {}", tf, crate::color::paint("31", "FAILED"));
                let stdout = String::from_utf8_lossy(&rout.stdout);
                let stderr = String::from_utf8_lossy(&rout.stderr);
                if !stdout.is_empty() { print!("{}", stdout); }
                if !stderr.is_empty() { eprint!("{}", stderr); }
                failed += 1;
            }
        }

        let elapsed = start.elapsed().as_secs_f64();
        println!();

        if failed == 0 {
            println!("test result: {}. {} passed; 0 failed; finished in {:.2}s",
                crate::color::paint("32", "ok"), passed, elapsed);
        } else {
            println!("test result: {}. {} passed; {} failed; finished in {:.2}s",
                crate::color::paint("31", "FAILED"), passed, failed, elapsed);
            crab_err!(ErrorKind::Other, "{} test(s) failed", failed);
        }

        Ok(())
    }
}
