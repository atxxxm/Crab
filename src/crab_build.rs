use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Instant, UNIX_EPOCH};
use std::collections::{BTreeMap, HashMap};
use regex::Regex;

use chrono::DateTime;
use rayon::prelude::*;

use crate::func::crab_config::CONFIG;
use crate::func::crab_ini::{load_config, save_config, Changed, CrabConfig};
use crate::func::crab_find::CrabFind;
use crate::{crab_err, crab_print, crab_log};
use std::io::ErrorKind;

pub struct CrabBuildFunc;

impl CrabBuildFunc {
    pub fn new() -> Self {
        CrabBuildFunc
    }

    // Проверка на наличие компилятора перед сборкой
    fn is_compiler(&self) -> std::io::Result<()>  {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;

        crab_log!("INFO", "BUILD" ,"Checking the compiler: {}", compiler);

        if compiler.is_empty() {
            crab_log!("ERROR", "BUILD", "The compiler is missing");
            crab_err!(ErrorKind::NotFound, "The compiler is missing: {}", compiler);
        } else {
            crab_log!("INFO", "BUILD", "Health check of the {} compiler", compiler);
            let status = Command::new(&compiler).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status()?;

            if !status.success() {
                crab_log!("ERROR", "BUILD", "Incorrect compiler name or missing compiler: {}", compiler);
                crab_err!(ErrorKind::Other, "Incorrect compiler name or missing compiler: {}", compiler);
            }

        }

        Ok(())
    }

    // Обёртка для вывода компилятора
    fn output_wrapper(&self, output: std::io::Result<Output>) -> std::io::Result<()> {
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();

                if !stdout.is_empty() {
                    crab_print!("{}", &stdout);
                }

                if !stderr.is_empty() {
                    if out.status.success() {
                        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
                        let re = Regex::new(r"^(.*?):(\d+):(\d+):\s+(предупреждение|warning|error):\s+(.*)$").unwrap();

                        for line in stderr.lines() {
                            if let Some(caps) = re.captures(line) {
                                let file = caps[1].to_string();
                                let line_num = &caps[2];
                                let col = &caps[3];
                                let msg = &caps[5];
                                grouped.entry(file).or_default()
                                    .push(format!("{:>5}:{:<3}  {}", line_num, col, msg));
                            } else if !line.contains("In constructor")
                                && !line.contains("In member function")
                                && !line.contains("In file included from") {
                                crab_print!(yellow, "{}", line);
                            }
                        }
                        for (file, warns) in grouped {
                            crab_print!(yellow, "⚠ {}", file);
                            for w in warns {
                                crab_print!(yellow, "    {}", w);
                            }
                        }
                    } else {
                        for line in stderr.lines() {
                            crab_print!(red, "{}", line);
                        }
                        crab_err!(ErrorKind::Other, "Error in the code");
                    }
                }

                Ok(())
            }

            Err(e) => {
                crab_err!(ErrorKind::Other, "Console output error: {}", e);
            }
        }
    }

    // Создания дебаг или релиз директорий со всем содержимым
    fn create_build_dir(&self, flag: &str) -> std::io::Result<()> {
        let object_dir = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.object_dir)
        };

        let binary_dir = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.binary_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.binary_dir)
        };

        let dependencies_file = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.dependencies)
        };

        crab_log!("INFO", "BUILD", "Checking the existence of a directory for object files: {}", dependencies_file.display());
        if !object_dir.exists() {
            crab_log!("INFO", "BUILD", "The directory does not exist, create: {}", dependencies_file.display());
            fs::create_dir_all(object_dir)?;
        }
        crab_log!("INFO", "BUILD", "Checking the existence of a directory for binary files: {}", binary_dir.display());
        if !binary_dir.exists() {
            crab_log!("INFO", "BUILD", "The directory does not exist, create: {}", binary_dir.display());
            fs::create_dir_all(binary_dir)?;
        }

        crab_log!("INFO", "BUILD", "Checking for the existence of a dependency file: {}", dependencies_file.display());
        if !dependencies_file.exists() {
            crab_log!("INFO", "BUILD", "The file does not exist, create: {}", dependencies_file.display());
            fs::File::create(dependencies_file)?;
        }

        Ok(())
    }

    // Создание директорий для модуля
    fn create_module_dir(&self, flag: &str, name: &str) -> std::io::Result<()> {
        let object_dir = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.debug_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.release_dir).join(CONFIG.object_dir)
        };

        let binary_dir = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.debug_dir).join(CONFIG.binary_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.release_dir).join(CONFIG.binary_dir)
        };

        let dependencies_file = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.release_dir).join(CONFIG.dependencies)
        };

        crab_log!("INFO", "BUILD", "Module: Checking the existence of a directory for object files: {}", dependencies_file.display());
        if !object_dir.exists() {
            crab_log!("INFO", "BUILD", "Module: The directory does not exist, create: {}", dependencies_file.display());
            fs::create_dir_all(object_dir)?;
        }
        crab_log!("INFO", "BUILD", "Module: Checking the existence of a directory for binary files: {}", binary_dir.display());
        if !binary_dir.exists() {
            crab_log!("INFO", "BUILD", "Module: The directory does not exist, create: {}", binary_dir.display());
            fs::create_dir_all(binary_dir)?;
        }

        crab_log!("INFO", "BUILD", "Module: Checking for the existence of a dependency file: {}", dependencies_file.display());
        if !dependencies_file.exists() {
            crab_log!("INFO", "BUILD", "Module: The file does not exist, create: {}", dependencies_file.display());
            fs::File::create(dependencies_file)?;
        }

        Ok(())
    }

    // Запись зависимостей
    fn write_dependencies(&self, flag: &str, cpp: &[String]) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD","Write dependencies");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let complier = config.settings.compiler;

        let path_to_dependencies_file = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else if flag == "release"{
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.dependencies)
        } else if flag == "static" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dependencies)
        };

        // Перезаписываем файл с нуля, чтобы зависимости не накапливались между сборками
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&path_to_dependencies_file)?;

        crab_log!("INFO", "BUILD", "Collecting dependencies");
        let result: Vec<std::io::Result<Output>> = cpp.par_iter().map(|c| {
            Command::new(&complier).arg("-MM").arg(c).output()
        }).collect();

        crab_log!("INFO", "BUILD", "Writing dependencies to a file: {}", path_to_dependencies_file.display());

        for (dep, _) in result.into_iter().zip(cpp.iter()) {
            let dep = dep?;

            if !dep.status.success() {
                let stderr = String::from_utf8_lossy(&dep.stderr).to_string();
                crab_log!("ERROR", "BUILD", "Error when collecting dependencies: {}", stderr);
                crab_err!(ErrorKind::Other, "Error when collecting dependencies: {}", stderr);
            }

            file.write_all(&dep.stdout)?;
        }

        Ok(())
    }

    // Запись зависимостей для модуля
    fn write_dependencies_module(&self, name: &str, flag: &str, cpp: &[String]) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD","Module: Write dependencies");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let complier = config.settings.compiler;

        let path_to_dependencies_file = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.release_dir).join(CONFIG.dependencies)
        };

        // Перезаписываем файл с нуля, чтобы зависимости не накапливались между сборками
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&path_to_dependencies_file)?;

        crab_log!("INFO", "BUILD", "Module: Collecting dependencies");
        let result: Vec<std::io::Result<Output>> = cpp.par_iter().map(|c| {
            Command::new(&complier).arg("-MM").arg(c).output()
        }).collect();

        crab_log!("INFO", "BUILD", "Module: Writing dependencies to a file: {}", path_to_dependencies_file.display());

        for (dep, _) in result.into_iter().zip(cpp.iter()) {
            let dep = dep?;

            if !dep.status.success() {
                let stderr = String::from_utf8_lossy(&dep.stderr).to_string();
                crab_log!("ERROR", "BUILD", "Module: Error when collecting dependencies: {}", stderr);
                crab_err!(ErrorKind::Other, "Error when collecting dependencies: {}", stderr);
            }

            file.write_all(&dep.stdout)?;
        }

        Ok(())
    }

    // Получени времени последей модификации файла
    fn get_file_mtime(&self, path: &str) -> std::io::Result<String> {
            crab_log!("INFO", "BUILD", "Getting the file modification time: {}", path);
            let metadata = fs::metadata(path)?;
            let modified_time = metadata.modified()?;

            let duration_since_epoch = modified_time
                .duration_since(UNIX_EPOCH)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            let seconds = duration_since_epoch.as_secs();
            let datetime = DateTime::from_timestamp(seconds as i64, 0)
                .unwrap()
                .format("%d:%m:%Y %H:%M:%S")
                .to_string();

            Ok(datetime)
        }

    // Функции для проверки существование папки header и файлов в ней
    fn is_header(&self) -> std::io::Result<bool> {

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let header_dir = config.settings.header_dir;
        crab_log!("INFO", "BUILD", "Checking the existence of a directory: {}", header_dir);

        let path = Path::new(header_dir.as_str());

        if path.exists() && path.is_dir() && fs::read_dir(&header_dir).unwrap().count() > 0 {
            crab_log!("INFO", "BUILD", "The directory {} exists", header_dir);
            return Ok(true)
        }

        Ok(false)
    }

    // Парсинг .d файла: исходник -> список всех его зависимостей (сам исходник + заголовки)
    fn parse_dependencies(&self, path_dep: &Path, lang: &str) -> std::io::Result<HashMap<String, Vec<String>>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        if !path_dep.exists() {
            return Ok(map);
        }

        let ext = if lang == "c" { ".c" } else { ".cpp" };
        let content = fs::read_to_string(path_dep)?;
        // Склеиваем переносы строк вида "... \<newline>" в одну запись
        let joined = content.replace("\\\r\n", " ").replace("\\\n", " ");

        for entry in joined.lines() {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }

            let rhs = match entry.split_once(':') {
                Some((target, rhs)) if target.trim().ends_with(".o") => rhs,
                _ => continue,
            };

            let prereqs: Vec<String> = rhs.split_whitespace().map(|s| s.to_string()).collect();

            // Исходник — это зависимость с нужным расширением, по ней и индексируем
            if let Some(src) = prereqs.iter().find(|p| p.ends_with(ext)) {
                map.entry(src.clone()).or_default().extend(prereqs.iter().cloned());
            }
        }

        Ok(map)
    }

    // Получение списка исходников, которые нужно пересобрать (с учётом изменений заголовков)
    fn get_changed_files(&self, path_to_obj_data: &Path, path_dep: &Path, cpp: &[String]) -> std::io::Result<Vec<String>> {
        crab_log!("INFO", "BUILD", "Checking for file modification");

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let deps_map = self.parse_dependencies(path_dep, &config.settings.lang)?;

        // Снимок прошлого состояния (read-only для сравнения)
        let old: HashMap<String, String> = if path_to_obj_data.exists() {
            load_config::<Changed>(path_to_obj_data.display().to_string().as_str())?.files
        } else {
            crab_log!("INFO", "BUILD", "There is no file for tracking modifications, create: {}", path_to_obj_data.display());
            fs::File::create(path_to_obj_data)?;
            HashMap::new()
        };

        let mut changed = Vec::new();
        let mut new_state: HashMap<String, String> = HashMap::new();

        for c in cpp {
            // Все файлы, влияющие на этот исходник: сам исходник + его заголовки из .d
            let mut prereqs = deps_map.get(c).cloned().unwrap_or_default();
            if !prereqs.iter().any(|p| p == c) {
                prereqs.push(c.clone());
            }

            let mut need_rebuild = false;

            for p in &prereqs {
                let new_time = match self.get_file_mtime(p) {
                    Ok(t) => t,
                    Err(_) => continue, // зависимость могла исчезнуть — пропускаем
                };

                if old.get(p).map_or(true, |o| o != &new_time) {
                    need_rebuild = true;
                }

                new_state.insert(p.clone(), new_time);
            }

            if need_rebuild {
                changed.push(c.clone());
            }
        }

        let change = Changed { files: new_state };
        save_config(&change, path_to_obj_data.display().to_string().as_str())?;
        crab_log!("INFO", "BUILD", "Modified files: {:?}", changed);
        Ok(changed)
    }

    // Проверка игнорируемых файлов
    fn check_ignore_files(&self, cpp: &mut Vec<String>) -> std::io::Result<()> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let file_list = config.files;

        crab_log!("INFO", "BUILD", "Checking ignored files");

        let mut files_to_conf: Vec<String> = Vec::new();
        let mut is_ignore_out = false;

        for c in cpp.iter() {
            let file = file_list.get(c).unwrap();

            if file == "on" {
                files_to_conf.push(c.to_string());
            } else {
                is_ignore_out = true;
                crab_print!(red, "ignore: {}", c);
            }

        }

        if !is_ignore_out {
            crab_print!(red, "None");
            crab_log!("INFO", "BUILD", "There are no ignored files");
        }

        cpp.retain(|item| files_to_conf.contains(item));

        Ok(())
    }

    // Запись файлов из source_dir в файл конфигурации
    fn write_file_in_config(&self, cpp: &[String]) -> std::io::Result<()> {
        if cpp.is_empty() {
            crab_log!("WARRNIG", "BUILD","Files are missing");
            return Ok(());
        }

        crab_log!("INFO", "BUILD", "Writing files to the configuration");
        let mut config: CrabConfig = load_config(CONFIG.config_file)?;
        let mut files = config.files;

        for c in cpp {
            if files.contains_key(c) {
                continue;
            }

            files.insert(c.to_string(), "on".to_string());
        }

        config.files = files;
        save_config(&config, CONFIG.config_file)?;

        crab_log!("INFO", "BUILD", "Written files to the configuration: {:?}", cpp);

        Ok(())
    }

    // Функция очистки от hpp и разделения файла на .o и путь
    fn split_dep(&self, text: &str, lang: &str) -> std::io::Result<[String; 2]> {
        let clean_text = text.trim();

        let o = clean_text.split(':').next().unwrap_or("");
        let o_str = o.to_string();

        let data_init = clean_text.split(':').nth(1).unwrap_or("");

        let data_vec: Vec<&str> = if lang == "c" {
            data_init.split_whitespace().filter(|s| !s.is_empty() && s.ends_with(".c")).collect()
        } else {
            data_init.split_whitespace().filter(|s| !s.is_empty() && s.ends_with(".cpp")).collect()
        };

        let data = data_vec.join("");

        Ok([o_str, data])

    }

    // Сбор всех файлов с определённым расширением
    fn collect_file_with_extension(dir: &Path, extension: &str, files: &mut Vec<String>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir(){

                    if let Some(dir_name) = path.file_name() {
                        if dir_name != CONFIG.build_dir {
                            Self::collect_file_with_extension(&path, extension, files)?;
                        }
                    }

                } else if path.extension().map_or(false, |ext| ext == extension) {
                    crab_print!(green, "{}", path.display());
                    files.push(path.display().to_string());
                }
            }
        }

        Ok(())
    }

}

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
    fn compiling_libary(&self, kind: LibKind) -> std::io::Result<()> {
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

            cbf.output_wrapper(Command::new("ar").args(&["rcs", &fmt_obj, &entry_str]).output())?;

            crab_print!(green, "+ {}", fmt_obj);
        }

        Ok(())
    }

    // Создание динамической библиотеки
    fn create_dynamic_libary(&self) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Create dynamic library");
        let cbf = CrabBuildFunc::new();
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;

        let path_to_obj = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dynamic_dir).join(CONFIG.object_dir);

        for entry in fs::read_dir(path_to_obj)? {
            let entry = entry?;

            let filename = entry.path().file_stem().unwrap().display().to_string();
            let fmt_obj = format!("{}/{}/{}/lib{}.so", CONFIG.build_dir, CONFIG.library_dir, CONFIG.dynamic_dir, filename);
            let entry_str = entry.path().display().to_string();

            cbf.output_wrapper(Command::new(&compiler).args(&["-shared", &entry_str, "-o", &fmt_obj]).output())?;

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
        self.compiling_libary(kind)?;

        match kind {
            LibKind::Static => {
                println!("\ncreate static libary: ");
                self.create_archive()?;
            }
            LibKind::Dynamic => {
                println!("\ncreate dynamic libary: ");
                self.create_dynamic_libary()?;
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
