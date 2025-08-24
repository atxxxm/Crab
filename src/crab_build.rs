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
    fn write_dependencies(&self, flag: &str, cpp: &Vec<String>) -> std::io::Result<()> {
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

        crab_log!("INFO", "BUILD","Checking for the existence of a dependency file: {}", path_to_dependencies_file.display());
        if !path_to_dependencies_file.exists() {
            crab_log!("INFO", "BUILD","Creating a dependency file: {}", path_to_dependencies_file.display());
            fs::File::create(&path_to_dependencies_file)?;
        }

        let mut file = OpenOptions::new().append(true).create(true).open(&path_to_dependencies_file)?;

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
    fn write_dependencies_module(&self, name: &str, flag: &str, cpp: &Vec<String>) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD","Module: Write dependencies");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let complier = config.settings.compiler;

        let path_to_dependencies_file = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name).join(CONFIG.release_dir).join(CONFIG.dependencies)
        };

        crab_log!("INFO", "BUILD","Module: Checking for the existence of a dependency file: {}", path_to_dependencies_file.display());
        if !path_to_dependencies_file.exists() {
            crab_log!("INFO", "BUILD","Module: Creating a dependency file: {}", path_to_dependencies_file.display());
            fs::File::create(&path_to_dependencies_file)?;
        }

        let mut file = OpenOptions::new().append(true).create(true).open(&path_to_dependencies_file)?;

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

    // Получение изменений в файле
    fn get_changed_files(&self, path_to_obj_data: &PathBuf, cpp: &[String]) -> std::io::Result<Vec<String>> {
        let mut changed = Vec::new();

        crab_log!("INFO", "BUILD", "Checking for file modification");
        if !path_to_obj_data.exists() {
            crab_log!("INFO", "BUILD", "There is no file for tracking modifications, create: {}", path_to_obj_data.display());
            fs::File::create(&path_to_obj_data)?;
            let mut files_changed = HashMap::new();
            for c in cpp {
                let time = self.get_file_mtime(c)?;
                files_changed.insert(c.to_string(), time);
                changed.push(c.clone());
            }

            let change = Changed {
                files: files_changed,
            };

            save_config(&change, &path_to_obj_data.display().to_string().as_str())?;
            crab_log!("INFO", "BUILD", "Modified files: {:?}", changed);
            return Ok(changed);
        }

        let mut config: Changed = load_config(&path_to_obj_data.display().to_string().as_str())?;
        let mut hash_file = config.files;

        for c in cpp {
            let new_time = self.get_file_mtime(c)?;

            if !hash_file.contains_key(c) {
                hash_file.insert(c.to_string(), new_time);
                changed.push(c.clone());
                continue;
            } 

            let old_time = hash_file.get(c).unwrap().to_string();

            if old_time != new_time {
                hash_file.insert(c.to_string(), new_time);
                changed.push(c.clone());
            }
        }

        config.files = hash_file;
        save_config(&config, &path_to_obj_data.display().to_string().as_str())?;
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
    fn write_file_in_config(&self, cpp: &Vec<String>) -> std::io::Result<()> {
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
    fn split_dep(&self, text: &str) -> std::io::Result<[String; 2]> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang = config.settings.lang;

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

pub struct CrabBuild;

impl CrabBuild {
    pub fn new() -> Self {
        CrabBuild
    }

    /* Функции для дебаг сборки */

    // Чтение и фортматирование файла с путями для стороних библиотек
    fn read_include_files_and_fmt(&self) -> std::io::Result<String> {
        let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.include_file);

        if !path.exists() {
            return Ok(String::new());
        }

        let file = File::open(path)?;

        let reader = BufReader::new(&file);

        let mut txt = String::new();

        for l in reader.lines() {
            let l = l?;

            if l.trim().is_empty() {
                continue;
            }

            let fmt_txt = format!("-I{} ", l);
            txt.push_str(&fmt_txt);
        }

        txt.pop();

        Ok(txt)
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
            let path = format!("-L{}", init_path.parent().unwrap().display().to_string());

            let file_name = init_path.file_stem().unwrap().to_str().unwrap();
            let clean_name = file_name.strip_prefix("lib").unwrap_or(file_name);
            let name = format!("-l{}", clean_name);

            lib_path.push(path);
            lib_name.push(name);
        }

        Ok((lib_path, lib_name))
    }

    // Компиляция в объектный файл (Дебаг)
    fn debug_complinig_to_object_file(&self, path_dep: &PathBuf, path_obj: &PathBuf, is_find: bool, changed: &[String]) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Debug compilation to an object file");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let cbf = CrabBuildFunc::new();

        let compiler = config.settings.compiler;
        let head = config.settings.header_dir;
        let is_head = cbf.is_header()?;
        

        if !path_dep.exists() {
            crab_log!("ERROR", "BUILD", "The dependency file was not found: {}", path_dep.display());
            crab_err!(ErrorKind::NotFound, "The dependency file was not found");
        }
        
        let file = fs::File::open(&path_dep)?;

        let debug_flags = vec!["-g", "-O0", "-Wall", "-Wextra", "-pedantic"];
        crab_log!("INFO", "BUILD", "Flags for debug compiling: {:?}", debug_flags);

        let reader = BufReader::new(&file);
        let lines: Vec<String> = reader.lines().collect::<std::io::Result<Vec<_>>>()?;

        lines.par_iter().try_for_each(|line| -> std::io::Result<()> {
            let line = line.trim();
            
            if line.is_empty() {
                return Ok(());
            }

            let result = cbf.split_dep(line)?;

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

            if is_head && is_find {
                let incl = self.read_include_files_and_fmt()?;
                let path_to_head = format!("-I{} {}", head, incl);
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &path_to_head]).args(&debug_flags).output())

            } else if is_head && !is_find {
                let path_to_head = format!("-I{}", head);
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &path_to_head]).args(&debug_flags).output())

            } else if is_find && !is_head {
                let incl = self.read_include_files_and_fmt()?;
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &incl]).args(&debug_flags).output())

            } else {
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj]).args(&debug_flags).output())
            }

        })?;
            

        Ok(())
    }

    // Линковка объектных файлов
    fn debug_linking(&self, path_obj: &PathBuf, is_find: bool, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Debug linking");
        if !path_obj.exists() {
            crab_log!("ERROR", "BUILD", "The directory with the object files was not found: {}", path_obj.display());
            crab_err!(ErrorKind::NotFound, "The directory with the object files was not found: {}", path_obj.display());
        }

        let mut obj_files = Vec::new();
        let mut out = String::new();

        let crb = CrabBuildFunc::new();

        for entry in fs::read_dir(&path_obj)? {
            let entry = entry?;

            if entry.metadata()?.is_file() {
                obj_files.push(entry.path().display().to_string());
                out.push_str(&format!("{} + ", entry.file_name().display().to_string()));
            }
        }
        
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;
        let project_name = config.project.name;
        let path_to_bin = if let (Some(m_name), Some(b_name)) = (mod_name, bin_name)  {
            format!("{}/{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.module_dir, m_name, CONFIG.debug_dir, CONFIG.binary_dir, b_name)
        } else {
            format!("{}/{}/{}/{}", CONFIG.build_dir, CONFIG.debug_dir, CONFIG.binary_dir, project_name)
        };
        
        crab_log!("INFO", "BUILD", "Creating a path for an executable file: {}", path_to_bin);

        if !is_find {
            crab_log!("INFO", "BUILD", "Linking without third-party libraries");
            crb.output_wrapper(Command::new(&compiler).args(obj_files).arg("-o").arg(&path_to_bin).output())?;
        } else {
            let (paths, names) = self.read_lib_path_and_fmt()?;

            crab_log!("INFO", "BUILD", "Linking with third-party libraries: {:?}", names);

            if !names.is_empty() {
                println!("\nlibaries:");
                
                for name in &names {
                    crab_print!(green, "+ {}", name);
                }
            }
            crb.output_wrapper(Command::new(&compiler).args(obj_files).arg("-o").arg(&path_to_bin).args(paths).args(names).output())?;
        };

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

    // Дебаг сборка 
    pub fn debug_building(&self, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        crab_print!(blue, "DEBUG BUILDING:\n");
        crab_log!("INFO", "BUILD", "START DEBUG BUILDING");

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            crb.create_module_dir("debug", m_name)?;
        } else {
            crb.create_build_dir("debug")?;
        }
    
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang = config.settings.lang;

        let source_dir = config.settings.source_dir;
        let path = Path::new(&source_dir);

        println!("collecting files: ");
        let mut source: Vec<String>;

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
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
            };

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

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            crb.write_dependencies_module(m_name, "debug", &source)?;
        } else {
            crb.write_dependencies("debug", &source)?;
        }
        
        
        let find = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            let module = config.module.get(m_name).ok_or_else(|| std::io::Error::new
                (std::io::ErrorKind::NotFound, format!("Module {} not found", m_name)))?;

            let mod_path = module.path.clone();
            CrabFind::new(&mod_path).parsing_include()?
        } else {
            CrabFind::new(".").parsing_include()?
        };

        println!("\ncompiling to an object file: ");

        let changed = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            let p_o = PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.debug_dir).join(CONFIG.object_data);
            crb.get_changed_files(&p_o, &source)?
        } else {
            let p_o= PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.object_data);
            crb.get_changed_files(&p_o, &source)?
        };

        let path_dep = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.dependencies)
        };

        let path_obj = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.debug_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.object_dir)

        };

        self.debug_complinig_to_object_file(&path_dep, &path_obj, find, &changed)?;

        println!("\nlinking: ");

        let path_obj = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.debug_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.object_dir)
        };

        self.debug_linking(&path_obj, find, mod_name, bin_name)?;

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f64());
        crab_log!("INFO", "BUILD", "End of the debug build");

        Ok(())
    }

    /* Релиз сборка */

    // Компиляция в объектный файл (Релиз)
    fn release_complinig_to_object_file(&self, path_dep: &PathBuf, path_obj: &PathBuf, is_find: bool, changed: &[String]) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Release compilation to an object file");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let crb = CrabBuildFunc::new();
        let compiler = config.settings.compiler;
        let is_head = crb.is_header()?;
        let head = config.settings.header_dir;

        if !path_dep.exists() {
            crab_log!("ERROR", "BUILD", "The dependency file was not found: {}", path_dep.display());
            crab_err!(ErrorKind::NotFound, "The dependency file was not found: {}", path_dep.display());
        }
        
        let release_flags = vec!["-O2", "-flto"];
        crab_log!("INFO", "BUILD", "Flags for release compiling: {:?}", release_flags);

        let file = File::open(&path_dep)?;

        let reader = BufReader::new(&file);
        let lines = reader.lines().collect::<std::io::Result<Vec<_>>>()?;

        lines.par_iter().try_for_each(|line| -> std::io::Result<()> {
            let line = line.trim();

            if line.is_empty() {
                return Ok(());
            }

            let result = crb.split_dep(&line)?;

            if !result[0].ends_with(".o") {
                return Ok(());
            }

            if !changed.contains(&result[1]) {
                crab_print!(purple, "Skipping: {} (Has not been changed)", &result[1]);
                crab_log!("INFO", "BUILD", "Skipping file: {}", &result[1]);
                return Ok(());
            }

            let path_to_obj = format!("{}/{}", path_obj.display(), &result[0]);
            crab_print!(blue, "{} -> {}", &result[1], &path_to_obj);

            if is_head && is_find {
                let incl = self.read_include_files_and_fmt()?;
                let path_to_head = format!("-I{} {}", head, incl);
                crb.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &path_to_head]).args(&release_flags).output())

            } 
            else if is_head && !is_find {
                let path_to_head = format!("-I{}", head);
                crb.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &path_to_head]).args(&release_flags).output())

            } else if is_find && !is_head {
                let incl = self.read_include_files_and_fmt()?;
                crb.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &incl]).args(&release_flags).output())

            } else {
                crb.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj]).args(&release_flags).output())
            }

        })?;

        Ok(())
    }

    // Линковка объектных файлов
    fn release_linking(&self, path_obj: &PathBuf, is_find: bool, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        crab_log!("INFO", "BUILD", "Release linking");
        if !path_obj.exists() {
            crab_log!("ERROR", "BUILD", "The directory with the object files was not found: {}", path_obj.display());
            crab_err!(ErrorKind::NotFound, "The directory with the object files was not found: {}", path_obj.display());
        }

        let mut obj_files = Vec::new();
        let mut out = String::new();

        let crb = CrabBuildFunc::new();

        for entry in fs::read_dir(&path_obj)? {
            let entry = entry?;

            if entry.metadata()?.is_file() {
                obj_files.push(entry.path().display().to_string());
                out.push_str(&format!("{} + ", entry.file_name().display().to_string()));
            }
        }

        let release_flag = vec!["-O2", "-flto", "-s"];
        
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;
        let project_name = config.project.name;

        let path_to_bin = if let (Some(m_name), Some(b_name)) = (mod_name, bin_name) {
            format!("{}/{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.module_dir, m_name, CONFIG.release_dir, CONFIG.binary_dir, b_name)
        } else {
            format!("{}/{}/{}/{}", CONFIG.build_dir, CONFIG.release_dir, CONFIG.binary_dir, project_name)
        };

        crab_log!("INFO", "BUILD", "Creating a path for an executable file: {}", path_to_bin);

        if !is_find {
            crab_log!("INFO", "BUILD", "Linking without third-party libraries");
            crb.output_wrapper(Command::new(&compiler).args(obj_files).arg("-o").arg(&path_to_bin).args(release_flag).output())?;
        } else {
            let (paths, names) = self.read_lib_path_and_fmt()?;

            crab_log!("INFO", "BUILD", "Linking with third-party libraries: {:?}", names);

            if !names.is_empty() {
                println!("\nlibaries:");
                
                for name in &names {
                    crab_print!(green, "+ {}", name);
                }
            }
            crb.output_wrapper(Command::new(&compiler).args(obj_files).arg("-o").arg(&path_to_bin).args(release_flag).args(paths).args(names).output())?;
            
        };

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

    // Релиз сборка
    pub fn release_building(&self, mod_name: Option<&str>, bin_name: Option<&str>) -> std::io::Result<()> {
        
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        crab_print!(green, "RELEASE BUILDING:\n");
        crab_log!("INFO", "BUILD", "START RELEASE BUILDING");

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            crb.create_module_dir("release", m_name)?;
        } else {
            crb.create_build_dir("release")?;
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;

        let source_dir = config.settings.source_dir;
        let lang = config.settings.lang;
        let path = Path::new(&source_dir);

        println!("collecting files: ");
        let mut source: Vec<String>;

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            let module = config.module.get(m_name).ok_or_else(|| std::io::Error::new
                (std::io::ErrorKind::NotFound, format!("Module {} not found", m_name)))?;
            
            source = module.dependencies.clone();

            if source.is_empty() {
                crab_err!(ErrorKind::NotFound, "No files in module {}!", m_name);
            }
        } else {
            source = Vec::new();
            if lang == "c" {
                CrabBuildFunc::collect_file_with_extension(path, "c", &mut source)?;
            } else {
                CrabBuildFunc::collect_file_with_extension(path, "cpp", &mut source)?;
            };

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

        if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            crb.write_dependencies_module(m_name, "release", &source)?;
        } else {
            crb.write_dependencies("release", &source)?;
        }
        
        let find = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            let module = config.module.get(m_name).ok_or_else(|| std::io::Error::new
                (std::io::ErrorKind::NotFound, format!("Module {} not found", m_name)))?;
            let mod_path = module.path.clone();
            CrabFind::new(&mod_path).parsing_include()?
        } else {
            CrabFind::new(".").parsing_include()?
        };

        println!("\ncompiling to an object file: ");

        let changed = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            let p_o = PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.release_dir).join(CONFIG.object_data);
            crb.get_changed_files(&p_o, &source)?
        } else {
            let p_o= PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.object_data);
            crb.get_changed_files(&p_o, &source)?
        };

        let path_dep = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.release_dir).join(CONFIG.dependencies)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.dependencies)
        };

        let path_obj = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.release_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.object_dir)

        };

        self.release_complinig_to_object_file(&path_dep, &path_obj, find, &changed)?;

        println!("\nlinking: ");
        let path_obj = if let (Some(m_name), Some(_)) = (mod_name, bin_name) {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(m_name).join(CONFIG.release_dir).join(CONFIG.object_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.object_dir)
        };
        self.release_linking(&path_obj, find, mod_name, bin_name)?;

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f32());
        crab_log!("INFO", "BUILD", "End of the release build");

        Ok(())
    }

}

pub struct CrabLib;

impl CrabLib {
    pub fn new() -> Self {
        CrabLib
    }

    // Создание директорий для библиотеки
    fn create_build_lib_dir(&self, flag: &str) -> std::io::Result<()> {
        let path_to_lib_dir = if flag == "static" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.static_dir)
        } else {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dynamic_dir)
        };

        let path_to_dep = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dependencies);

        crab_log!("INFO", "LIB", "Checking the existence of a directory for library: {}", path_to_lib_dir.display());
        if !path_to_lib_dir.exists() {
            crab_log!("INFO", "LIB", "The directory does not exist, create: {}", path_to_lib_dir.display());
            fs::create_dir_all(path_to_lib_dir)?;
        }
        crab_log!("INFO", "BUILD", "Checking for the existence of a dependency file: {}", path_to_dep.display());
        if !path_to_dep.exists() {
            crab_log!("INFO", "BUILD", "The file does not exist, create: {}", path_to_dep.display());
            File::create(path_to_dep)?;
        }

        Ok(())
    }

    // Компиляция в объектный файл статической бибилиотеки
    fn compiling_static_libary(&self) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Static compilation to an object file");
        let path_to_object_dir = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.static_dir).join(CONFIG.object_dir);

        if !path_to_object_dir.exists() {
            crab_log!("INFO", "LIB", "There is no directory for the object files. To create: {}", path_to_object_dir.display());
            fs::create_dir(path_to_object_dir)?;
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let cbf = CrabBuildFunc::new();

        let compiler = config.settings.compiler;
        let head = config.settings.header_dir;
        let is_head = cbf.is_header()?;
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

            let result = cbf.split_dep(line)?;

            if !result[0].ends_with(".o") {
                return Ok(());
            }

            let path_to_obj = format!("{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.library_dir, CONFIG.static_dir ,CONFIG.object_dir, result[0]);
            crab_print!(blue, "{} -> {}", &result[1], &path_to_obj);

            if is_head {
                let h = format!("-I{}", head);
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj, &h]).output())
            } else {
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", &result[1], "-o", &path_to_obj]).output())
            }

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

    // Компиляция с позиционно-независимым кодом (динамическая библиотека)
    fn compiling_dynamic_libary(&self) -> std::io::Result<()> {
        crab_log!("INFO", "LIB", "Dynamic compilation to an object file");
        let path_to_object_dir = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir).join(CONFIG.dynamic_dir).join(CONFIG.object_dir);

        if !path_to_object_dir.exists() {
            crab_log!("INFO", "LIB", "There is no directory for the object files. To create: {}", path_to_object_dir.display());
            fs::create_dir_all(&path_to_object_dir)?;
        }

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let cbf = CrabBuildFunc::new();

        let compiler = config.settings.compiler;
        let head = config.settings.header_dir;
        let is_head = cbf.is_header()?;
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

            let result = cbf.split_dep(line)?;

            if !result[0].ends_with(".o") {
                return Ok(());
            }

            let path_to_obj = format!("{}/{}/{}/{}/{}", CONFIG.build_dir, CONFIG.library_dir, CONFIG.dynamic_dir, CONFIG.object_dir, result[0]);
            crab_print!(blue, "{} -> {}", &result[1], &path_to_obj);

            if is_head {
                let h = format!("-I{}", head);
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", "-fPIC", &result[1], "-o", &path_to_obj, &h]).output())
            } else {
                cbf.output_wrapper(Command::new(&compiler).args(&["-c", "-fPIC", &result[1], "-o", &path_to_obj]).output())
            }

        })?;

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

    // Статическая библиотека
    pub fn static_lib_build(&self) -> std::io::Result<()> {
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        crab_print!(cyan, "STATIC LIBRARY BUILDING:\n");
        crab_log!("INFO", "LIB", "START STATIC LIBARY BUILDING");

        self.create_build_lib_dir("static")?;
    
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
        };

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_file_in_config(&source)?;

        println!("\nchecking ignored files: ");
        
        crb.check_ignore_files(&mut source)?;

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_dependencies("static", &source)?;

        println!("\ncompiling to an object file: ");
        self.compiling_static_libary()?;

        println!("\ncreate static libary: ");

        self.create_archive()?;

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f64());
        crab_log!("INFO", "LIB", "End of the static library build");
        Ok(())
    }

    // Динамическая библиотека
    pub fn dynamic_lib_build(&self) -> std::io::Result<()> {
        let crb = CrabBuildFunc::new();

        crb.is_compiler()?;

        let start = Instant::now();

        crab_print!(purple, "DYNAMIC LIBRARY BUILDING:\n");

        self.create_build_lib_dir("dynamic")?;
    
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
        };

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_file_in_config(&source)?;

        println!("\nchecking ignored files: ");
        
        crb.check_ignore_files(&mut source)?;

        if source.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no files to build!");
        }

        crb.write_dependencies("dynamic", &source)?;

        println!("\ncompiling to an object file: ");
        self.compiling_dynamic_libary()?;

        println!("\ncreate dynamic libary: ");

        self.create_dynamic_libary()?;

        crab_print!(green, "\nDone! ({:.2} sec)", start.elapsed().as_secs_f64());
        crab_log!("INFO", "LIB", "End of the dynamic library build");

        Ok(())
    }
}
