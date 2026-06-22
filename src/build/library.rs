use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use rayon::prelude::*;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::{crab_err, crab_print, crab_log};
use super::helpers::CrabBuildFunc;
use std::io::ErrorKind;

// Тип библиотеки: статическая или динамическая
#[derive(Copy, Clone)]
pub enum LibKind {
    Static,
    Dynamic,
}

impl LibKind {
    // Имя подкаталога библиотеки ("static" | "dynamic")
    fn dir(&self) -> &'static str {
        match self {
            LibKind::Static => CONFIG.static_dir,
            LibKind::Dynamic => CONFIG.dynamic_dir,
        }
    }
}

pub struct CrabLib;

impl Default for CrabLib {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabLib {
    pub fn new() -> Self {
        CrabLib
    }

    // Создание директорий для библиотеки
    fn create_build_lib_dir(&self, kind: LibKind) -> std::io::Result<()> {
        let path_to_lib_dir = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(kind.dir());
        let path_to_dep = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dependencies);

        crab_log!("INFO", "LIB", "Checking the existence of a directory for library: {}", path_to_lib_dir.display());
        if !path_to_lib_dir.exists() {
            crab_log!("INFO", "LIB", "The directory does not exist, create: {}", path_to_lib_dir.display());
            fs::create_dir_all(path_to_lib_dir)?;
        }
        crab_log!("INFO", "LIB", "Checking for the existence of a dependency file: {}", path_to_dep.display());
        if !path_to_dep.exists() {
            crab_log!("INFO", "LIB", "The file does not exist, create: {}", path_to_dep.display());
            File::create(path_to_dep)?;
        }

        Ok(())
    }

    // Компиляция исходников в объектные файлы библиотеки (для динамической добавляется -fPIC)
    fn compiling_library(&self, kind: LibKind) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Compilation to an object file");
        let path_to_object_dir = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(kind.dir()).join(CONFIG.object_dir);

        if !path_to_object_dir.exists() {
            crab_log!("INFO", "LIB", "There is no directory for the object files. To create: {}", path_to_object_dir.display());
            fs::create_dir_all(&path_to_object_dir)?;
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let cbf = CrabBuildFunc::new();

        let compiler = config.settings.compiler;
        let head = config.settings.header_dir;
        let lang = config.settings.lang;
        let is_head = cbf.is_header()?;
        let is_dynamic = matches!(kind, LibKind::Dynamic);
        let user_compile = config.build.compile_args();
        let path_to_dep_file = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dependencies);

        if !path_to_dep_file.exists() {
            crab_log!("ERROR", "LIB", "The dependency file was not found: {}", path_to_dep_file.display());
            crab_err!(ErrorKind::NotFound, "The dependency file was not found");
        }

        let file = fs::File::open(&path_to_dep_file)?;
        let reader = BufReader::new(&file);
        let lines: Vec<String> = reader.lines().collect::<std::io::Result<Vec<_>>>()?;

        lines.par_iter().try_for_each(|line| -> std::io::Result<()> {
            let line = line.trim();

            if line.is_empty() {
                return Ok(());
            }

            let result = cbf.split_dep(line, &lang)?;

            if !result[0].ends_with(".o") {
                return Ok(());
            }

            let path_to_obj = format!("{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.library_dir, kind.dir(), CONFIG.object_dir, result[0]);
            crab_print!(blue, "{} -> {}", &result[1], &path_to_obj);

            let mut args: Vec<String> = vec!["-c".to_string()];
            if is_dynamic {
                args.push("-fPIC".to_string());
            }
            args.push(result[1].clone());
            args.push("-o".to_string());
            args.push(path_to_obj);
            if is_head {
                args.push(format!("-I{}", head));
            }
            args.extend(user_compile.iter().cloned());

            cbf.output_wrapper(Command::new(&compiler).args(&args).output())
        })?;

        Ok(())
    }

    // Создание архива для статической библиотеки
    fn create_archive(&self) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Create static library");
        let cbf = CrabBuildFunc::new();

        let path_to_obj = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.static_dir).join(CONFIG.object_dir);

        for entry in fs::read_dir(path_to_obj)? {
            let entry = entry?;

            let filename = entry.path().file_stem().unwrap().display().to_string();
            let fmt_obj = format!("{}/{}/{}/lib{}.a", CONFIG.build_dir, CONFIG.library_dir, CONFIG.static_dir, filename);
            let entry_str = entry.path().display().to_string();

            cbf.output_wrapper(Command::new("ar").args(["rcs", &fmt_obj, &entry_str]).output())?;

            crab_print!(green, "+ {}", fmt_obj);
        }

        Ok(())
    }

    // Создание динамической библиотеки
    fn create_dynamic_library(&self) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Create dynamic library");
        let cbf = CrabBuildFunc::new();
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;
        let user_link = config.build.link_args();

        let path_to_obj = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dynamic_dir).join(CONFIG.object_dir);

        for entry in fs::read_dir(path_to_obj)? {
            let entry = entry?;

            let filename = entry.path().file_stem().unwrap().display().to_string();
            let fmt_obj = format!("{}/{}/{}/lib{}.so", CONFIG.build_dir, CONFIG.library_dir, CONFIG.dynamic_dir, filename);
            let entry_str = entry.path().display().to_string();

            cbf.output_wrapper(Command::new(&compiler).args(["-shared", &entry_str, "-o", &fmt_obj]).args(&user_link).output())?;

            crab_print!(green, "+ {}", fmt_obj);
        }

        Ok(())
    }

    // Сборка библиотеки (статической или динамической)
    pub fn build_lib(&self, kind: LibKind) -> std::io::Result<()> {
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        match kind {
            LibKind::Static => { crab_print!(cyan, "STATIC LIBRARY BUILDING:\n"); }
            LibKind::Dynamic => { crab_print!(purple, "DYNAMIC LIBRARY BUILDING:\n"); }
        }
        crab_log!("INFO", "LIB", "START LIBRARY BUILDING");

        self.create_build_lib_dir(kind)?;

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang = config.settings.lang;
        let source_dir = config.settings.source_dir;
        let path = Path::new(source_dir.as_str());

        println!("collecting files: ");
        let mut source: Vec<String> = Vec::new();

        if lang == "c" {
            CrabBuildFunc::collect_file_with_extension(path, "c", &mut source)?;
        } else {
            CrabBuildFunc::collect_file_with_extension(path, "cpp", &mut source)?;
        }

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_file_in_config(&source)?;

        println!("\nchecking ignored files: ");
        crb.check_ignore_files(&mut source)?;

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_dependencies(kind.dir(), &source)?;

        println!("\ncompiling to an object file: ");
        self.compiling_library(kind)?;

        match kind {
            LibKind::Static => {
                println!("\ncreate static library: ");
                self.create_archive()?;
            }
            LibKind::Dynamic => {
                println!("\ncreate dynamic library: ");
                self.create_dynamic_library()?;
            }
        }

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f64());
        crab_log!("INFO", "LIB", "End of the library build");

        Ok(())
    }

    // Статическая библиотека
    pub fn static_lib_build(&self) -> std::io::Result<()> {
        self.build_lib(LibKind::Static)
    }

    // Динамическая библиотека
    pub fn dynamic_lib_build(&self) -> std::io::Result<()> {
        self.build_lib(LibKind::Dynamic)
    }
}
