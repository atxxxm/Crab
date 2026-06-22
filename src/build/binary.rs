use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use rayon::prelude::*;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_print, crab_log};
use super::helpers::CrabBuildFunc;
use std::io::ErrorKind;

// Профиль сборки: отличаются каталогом и набором флагов компиляции/линковки
#[derive(Copy, Clone)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    // Имя каталога режима ("debug" | "release")
    fn dir(&self) -> &'static str {
        match self {
            BuildProfile::Debug => CONFIG.debug_dir,
            BuildProfile::Release => CONFIG.release_dir,
        }
    }

    // Флаги компиляции в объектный файл
    fn compile_flags(&self) -> &'static [&'static str] {
        match self {
            BuildProfile::Debug => &["-g", "-O0", "-Wall", "-Wextra", "-pedantic"],
            BuildProfile::Release => &["-O2", "-flto"],
        }
    }

    // Флаги линковки
    fn link_flags(&self) -> &'static [&'static str] {
        match self {
            BuildProfile::Debug => &[],
            BuildProfile::Release => &["-O2", "-flto", "-s"],
        }
    }
}

pub struct CrabBuild;

impl Default for CrabBuild {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabBuild {
    pub fn new() -> Self {
        CrabBuild
    }

    // Чтение файла с путями для сторонних библиотек -> список флагов -I (по одному на аргумент)
    fn read_include_files_and_fmt(&self) -> std::io::Result<Vec<String>> {
        let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.include_file);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;

        let reader = BufReader::new(&file);

        let mut includes = Vec::new();

        for l in reader.lines() {
            let l = l?;

            if l.trim().is_empty() {
                continue;
            }

            includes.push(format!("-I{}", l));
        }

        Ok(includes)
    }

    // Чтение и фортматирование файла с путями для библиотек
    fn read_lib_path_and_fmt(&self) -> std::io::Result<(Vec<String>, Vec<String>)> {
        let path_to_file = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.lib_file);

        if !path_to_file.exists() {
            return Ok((Vec::new(), Vec::new()));
        }

        let file = File::open(path_to_file)?;

        let reader = BufReader::new(&file);
        let mut lib_path = Vec::new();
        let mut lib_name = Vec::new();

        for line in reader.lines() {
            let line = line?;

            if line.trim().is_empty() {
                continue;
            }

            let init_path = Path::new(&line);
            let path = format!("-L{}", init_path.parent().unwrap().display());

            let file_name = init_path.file_stem().unwrap().to_str().unwrap();
            let clean_name = file_name.strip_prefix("lib").unwrap_or(file_name);
            let name = format!("-l{}", clean_name);

            lib_path.push(path);
            lib_name.push(name);
        }

        Ok((lib_path, lib_name))
    }

    // Компиляция исходников в объектные файлы (debug/release)
    fn compile_to_object(&self, profile: BuildProfile, path_dep: &Path, path_obj: &Path, is_find: bool, changed: &[String]) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Compilation to an object file");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let cbf = CrabBuildFunc::new();

        let compiler = config.settings.compiler;
        let head = config.settings.header_dir;
        let lang = config.settings.lang;
        let is_head = cbf.is_header()?;

        if !path_dep.exists() {
            crab_log!("ERROR", "BUILD", "The dependency file was not found: {}", path_dep.display());
            crab_err!(ErrorKind::NotFound, "The dependency file was not found");
        }

        let flags = profile.compile_flags();
        crab_log!("INFO", "BUILD", "Flags for compiling: {:?}", flags);

        let file = fs::File::open(path_dep)?;
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

            if !changed.contains(&result[1]) {
                crab_print!(purple, "Skipping: {} (Has not been changed)", &result[1]);
                crab_log!("INFO", "BUILD", "Skipping file: {}", &result[1]);
                return Ok(());
            }

            let path_to_obj = format!("{}/{}", path_obj.display(), result[0]);
            crab_print!(blue, "{} -> {}", &result[1], path_to_obj);

            let mut compile_args: Vec<String> = vec![
                "-c".to_string(),
                result[1].clone(),
                "-o".to_string(),
                path_to_obj,
            ];

            if is_head {
                compile_args.push(format!("-I{}", head));
            }

            if is_find {
                compile_args.extend(self.read_include_files_and_fmt()?);
            }

            cbf.output_wrapper(Command::new(&compiler).args(&compile_args).args(flags).output())
        })?;

        Ok(())
    }

    // Линковка объектных файлов в исполняемый
    fn linking(&self, profile: BuildProfile, path_obj: &Path, is_find: bool, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Linking");
        if !path_obj.exists() {
            crab_log!("ERROR", "BUILD", "The directory with the object files was not found: {}", path_obj.display());
            crab_err!(ErrorKind::NotFound, "The directory with the object files was not found: {}", path_obj.display());
        }

        let mut obj_files = Vec::new();
        let mut out = String::new();

        let crb = CrabBuildFunc::new();

        for entry in fs::read_dir(path_obj)? {
            let entry = entry?;

            if entry.metadata()?.is_file() {
                obj_files.push(entry.path().display().to_string());
                out.push_str(&format!("{} + ", entry.file_name().display()));
            }
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;
        let project_name = config.project.name;
        let link_flags = profile.link_flags();

        let path_to_bin = if let (Some(m_name), Some(b_name)) = (mod_name, bin_name) {
            format!("{}/{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.module_dir, m_name, profile.dir(), CONFIG.binary_dir, b_name)
        } else {
            format!("{}/{}/{}/{}", CONFIG.build_dir, profile.dir(), CONFIG.binary_dir, project_name)
        };

        crab_log!("INFO", "BUILD", "Creating a path for an executable file: {}", path_to_bin);

        if !is_find {
            crab_log!("INFO", "BUILD", "Linking without third-party libraries");
            crb.output_wrapper(Command::new(&compiler).args(&obj_files).arg("-o").arg(&path_to_bin).args(link_flags).output())?;
        } else {
            let (paths, names) = self.read_lib_path_and_fmt()?;

            crab_log!("INFO", "BUILD", "Linking with third-party libraries: {:?}", names);

            if !names.is_empty() {
                println!("\nlibaries:");

                for name in &names {
                    crab_print!(green, "+ {}", name);
                }
            }
            crb.output_wrapper(Command::new(&compiler).args(&obj_files).arg("-o").arg(&path_to_bin).args(link_flags).args(paths).args(names).output())?;
        }

        if out.len() > 1 {
            out.truncate(out.len() - 2);
        }

        if let Some(b_name) = bin_name {
            out.push_str(&format!("-> {}", b_name));
        } else {
            out.push_str(&format!("-> {}", project_name));
        }

        crab_print!(cyan, "{}", out);

        Ok(())
    }

    // Сборка бинарника или модуля в заданном профиле (debug/release)
    pub fn building(&self, profile: BuildProfile, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        match profile {
            BuildProfile::Debug => { crab_print!(blue, "DEBUG BUILDING:\n"); }
            BuildProfile::Release => { crab_print!(green, "RELEASE BUILDING:\n"); }
        }
        crab_log!("INFO", "BUILD", "START {} BUILDING", profile.dir());

        let flag = profile.dir();
        let is_module = mod_name.is_some() && bin_name.is_some();

        if is_module {
            crb.create_module_dir(flag, mod_name.unwrap())?;
        } else {
            crb.create_build_dir(flag)?;
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang = config.settings.lang;
        let source_dir = config.settings.source_dir;
        let path = Path::new(&source_dir);

        println!("collecting files: ");
        let mut source: Vec<String>;

        if is_module {
            let m_name = mod_name.unwrap();
            let module = config.module.get(m_name).ok_or_else(|| std::io::Error::new
                (std::io::ErrorKind::NotFound, format!("Module {} not found", m_name)))?;

            source = module.dependencies.clone();

            if source.is_empty() {
                crab_err!(ErrorKind::Other, "No files in module {}!", m_name);
            }
        } else {
            source = Vec::new();
            if lang == "c" {
                CrabBuildFunc::collect_file_with_extension(path, "c", &mut source)?;
            } else {
                CrabBuildFunc::collect_file_with_extension(path, "cpp", &mut source)?;
            }

            if source.is_empty() {
                crab_err!(ErrorKind::NotFound, "There are no files to build!");
            }
        }

        crb.write_file_in_config(&source)?;

        println!("\nchecking ignored files: ");
        crb.check_ignore_files(&mut source)?;

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        if is_module {
            crb.write_dependencies_module(mod_name.unwrap(), flag, &source)?;
        } else {
            crb.write_dependencies(flag, &source)?;
        }

        let find = if is_module {
            let m_name = mod_name.unwrap();
            let module = config.module.get(m_name).ok_or_else(|| std::io::Error::new
                (std::io::ErrorKind::NotFound, format!("Module {} not found", m_name)))?;
            let mod_path = module.path.clone();
            CrabFind::new(&mod_path).parsing_include()?
        } else {
            CrabFind::new(".").parsing_include()?
        };

        println!("\ncompiling to an object file: ");

        let base = if is_module {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(mod_name.unwrap()).join(flag)
        } else {
            PathBuf::from(CONFIG.build_dir).join(flag)
        };

        let path_dep = base.join(CONFIG.dependencies);
        let path_obj = base.join(CONFIG.object_dir);
        let path_obj_data = base.join(CONFIG.object_data);

        let changed = crb.get_changed_files(&path_obj_data, &path_dep, &source)?;

        self.compile_to_object(profile, &path_dep, &path_obj, find, &changed)?;

        println!("\nlinking: ");
        self.linking(profile, &path_obj, find, mod_name, bin_name)?;

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f64());
        crab_log!("INFO", "BUILD", "End of the build");

        Ok(())
    }

    // Дебаг сборка
    pub fn debug_building(&self, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        self.building(BuildProfile::Debug, mod_name, bin_name)
    }

    // Релиз сборка
    pub fn release_building(&self, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        self.building(BuildProfile::Release, mod_name, bin_name)
    }
}
