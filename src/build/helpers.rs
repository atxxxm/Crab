use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::UNIX_EPOCH;
use std::collections::{BTreeMap, HashMap};
use regex::Regex;

use chrono::DateTime;
use rayon::prelude::*;

use crate::config::{load_config, save_config, Changed, CrabConfig, CONFIG};
use crate::{crab_err, crab_print, crab_log};
use std::io::ErrorKind;

pub struct CrabBuildFunc;

impl CrabBuildFunc {
    pub fn new() -> Self {
        CrabBuildFunc
    }

    // Проверка на наличие компилятора перед сборкой
    pub(crate) fn is_compiler(&self) -> std::io::Result<()>  {
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
    pub(crate) fn output_wrapper(&self, output: std::io::Result<Output>) -> std::io::Result<()> {
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
    pub(crate) fn create_build_dir(&self, flag: &str) -> std::io::Result<()> {
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
    pub(crate) fn create_module_dir(&self, flag: &str, name: &str) -> std::io::Result<()> {
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
    pub(crate) fn write_dependencies(&self, flag: &str, cpp: &[String]) -> std::io::Result<()> {
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
    pub(crate) fn write_dependencies_module(&self, name: &str, flag: &str, cpp: &[String]) -> std::io::Result<()> {
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
    pub(crate) fn is_header(&self) -> std::io::Result<bool> {

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
    pub(crate) fn get_changed_files(&self, path_to_obj_data: &Path, path_dep: &Path, cpp: &[String]) -> std::io::Result<Vec<String>> {
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
    pub(crate) fn check_ignore_files(&self, cpp: &mut Vec<String>) -> std::io::Result<()> {
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
    pub(crate) fn write_file_in_config(&self, cpp: &[String]) -> std::io::Result<()> {
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
    pub(crate) fn split_dep(&self, text: &str, lang: &str) -> std::io::Result<[String; 2]> {
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
    pub(crate) fn collect_file_with_extension(dir: &Path, extension: &str, files: &mut Vec<String>) -> std::io::Result<()> {
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
