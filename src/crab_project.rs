use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::prelude::*;
use std::process::{Command, Stdio};

use crate::func::crab_config::CONFIG;
use crate::func::crab_ini::{load_config, save_config, CrabConfig, Libraries, Project, Settings};
use crate::func::crab_find::CrabFind;
use crate::{crab_err, crab_print, crab_log};
use std::io::{BufRead, ErrorKind, Write};

pub struct CrabProject {
    name: String,
}

impl CrabProject {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }

    // Проверка на существование компилятора
    fn checking_compilers() -> std::io::Result<[bool; 3]> {
        let gpp_ok = Command::new("g++").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
        let clang_ok = Command::new("clang").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
        let gcc_ok = Command::new("gcc").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();

        Ok([gpp_ok, clang_ok, gcc_ok])
    }

    // Проверка на git
    fn is_git(&self) -> bool {
        Command::new("git").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
    }

    // Проверка на сущестование проекта
    pub fn is_exists(&self) -> std::io::Result<bool> {
        
        let path = Path::new(&self.name);

        if fs::metadata(path).is_ok() {
            crab_err!(ErrorKind::AlreadyExists, "A directory with that name already exists: {}", &self.name);
        }

        Ok(false)
    }

    // Заполнение конфигурационого файла
    pub fn init_config(&self, project_name: &str, is_new: bool, lang: &str) -> std::io::Result<()> {
        let path_to_config =  if is_new {
            PathBuf::from(project_name).join(CONFIG.config_file)
        } else {
            PathBuf::from(CONFIG.config_file)
        };


        let is_compiler = Self::checking_compilers()?;

        let compiler = if lang == "c" {
            if is_compiler[2] == true {
                "gcc"
            } else {
                ""
            }
        } else {
            if is_compiler[0] == true && is_compiler[1] == true {
                "g++"
            } else if is_compiler[0] == true {
                "g++"
            } else if is_compiler[1] == true {
                "clang"
            } else {
                ""
            }
        };

        let config = CrabConfig {
            project: Project {
                name: project_name.to_string(),
                version: "0.0.1".to_string(),
                created: Utc::now().year(),
            },

            settings: Settings {
                lang: lang.to_string(),
                compiler: compiler.to_string(),
                source_dir: "src".to_string(),
                header_dir: "include".to_string(),
            },

            files: if is_new {
                let mut files = HashMap::new();
                files.insert(if lang == "c" { "main.c".to_string() } else { "main.cpp".to_string() }, "on".to_string());
                files
            } else {
                HashMap::new()
            },

            libraries: Libraries {
                path: vec![],
            },

            module: HashMap::new(),
        };

        save_config(&config, &path_to_config.display().to_string().as_str())?;

        Ok(())
    }

    // Создание проекта
    pub fn create(&self, git: bool, lang: &str, cli: bool) -> std::io::Result<()> {
        // Создание путей
        let source_dir_name = PathBuf::from(&self.name).join("src");
        let build_dir_name = PathBuf::from(&self.name).join(CONFIG.build_dir);
        let config_file_name= PathBuf::from(&self.name).join(CONFIG.config_file);
        

        fs::create_dir_all(source_dir_name)?;

        fs::create_dir_all(build_dir_name)?;

        fs::File::create(&config_file_name)?;

        // Создание main.cpp и заполнение базовым кодом
        
        let main_code = if lang == "c++" {
            if cli {
                String::from("#include <iostream>\n\nint main(int argc, char const *argv[])\n{\n\tstd::cout << \"Hello Crab!\" << std::endl;\n}\n")
            } else {
                String::from("#include <iostream>\n\nint main()\n{\n\tstd::cout << \"Hello Crab!\" << std::endl;\n}\n")
            }
            
        } else {
            if cli {
                String::from("#include <stdio.h>\n\nint main(int argc, char const *argv[])\n{\n\tprintf(\"Hello Crab!\");\n}\n")
            } else {
                String::from("#include <stdio.h>\n\nint main()\n{\n\tprintf(\"Hello Crab!\");\n}\n")
            }
            
        };

        let path_to_main = if lang == "c++" {
            PathBuf::from(&self.name).join("src").join("main.cpp")
        } else {
            PathBuf::from(&self.name).join("src").join("main.c")
        };

        let mut file = fs::File::create(path_to_main)?;
        file.write_all(main_code.as_bytes())?;        
        self.init_config(&self.name, true, lang)?;

        crab_print!(green, "The \"{}\" project has been successfully created!", &self.name);

        if git {
            if self.is_git() {
                std::env::set_current_dir(&self.name)?;
                Command::new("git").arg("init").stdout(Stdio::null()).stderr(Stdio::null()).status()?;
                fs::write(".gitignore", &format!("{}/", CONFIG.build_dir))?;
            } else {
                crab_print!(yellow, "Git is not installed on your computer!");
            }
        }

        Ok(())
    }

    // Иницализация проекта
    pub fn init(&self) -> std::io::Result<()> {

        fs::File::create(CONFIG.config_file)?;

        let source_dir = "src";
        let header_dir = "include";

        let mut files: Vec<String> = Vec::new();
        let mut headers: Vec<String> = Vec::new();

        let to_path = Path::new(".");

        CrabFind::collect_file_with_extension(to_path, "cpp", &mut files)?;
        CrabFind::collect_file_with_extension(to_path, "c", &mut files)?;
        CrabFind::collect_file_with_extension(to_path, "hpp", &mut headers)?;
        CrabFind::collect_file_with_extension(to_path, "h", &mut headers)?;

        fn remove_empty_parents(start: &Path, stop_at: &Path) -> std::io::Result<()> {
            let mut current = start.parent();

            while let Some(dir) = current {
                if dir == stop_at {
                    break;
                }
                match fs::remove_dir(dir) {
                    Ok(_) => current = dir.parent(),
                    Err(_) => break, 
                }
            }

            Ok(())
        }


        fn copy_files(files: &[String], base_dir: &str, root: &Path, skip: &str) -> std::io::Result<()> {
            for file_path_str in files {
                let file_path = Path::new(file_path_str);
                
                let rel_path = file_path.strip_prefix(root).unwrap_or(file_path);

                if let Some(is_skip) = rel_path.parent() {
                    if is_skip.to_string_lossy() == skip {
                        continue;
                    }
                }

                let new_path = Path::new(base_dir).join(rel_path);

                if let Some(parent) = new_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                match fs::copy(&file_path, &new_path) {
                    Ok(_) => {
                        fs::remove_file(&file_path)?;
                        remove_empty_parents(&file_path, root)?;
                    }
                    Err(_) => (),
                }
            }

            Ok(())
        }

        if !files.is_empty() {
            if !Path::new(&source_dir).exists() {
                fs::create_dir_all(&source_dir)?;
            }

            copy_files(&files, &source_dir, to_path, &source_dir)?;
        }

        if !headers.is_empty() {
            if !Path::new(&header_dir).exists() {
                fs::create_dir_all(&header_dir)?;
            }

            copy_files(&headers, &header_dir, to_path, &header_dir)?;
        }

        fs::File::create(CONFIG.config_file)?;
        fs::create_dir(CONFIG.build_dir)?;

        let cur_dir = std::env::current_dir()?;

        fn is_file(ext: &str, v: &Vec<String>) -> bool {
            v.iter().any(|f| {
                Path::new(f).extension().and_then(|e| e.to_str()) == Some(ext)
            })
        }
            

        if let Some(name) = cur_dir.file_name() {
            let project_name=  name.to_string_lossy();

            let lang = if is_file("cpp", &files) {
                "c++"
            } else if is_file("c", &files) {
                "c"
            } else {
                "c++"
            };

            self.init_config(&project_name, false, lang)?;

            crab_print!(cyan, "The \"{}\" project has been successfully initialized!", project_name);
        }

        Ok(())
    }

}


pub struct CrabRun;

impl CrabRun {
    pub fn new() -> Self {
        CrabRun
    }
    // Запуск исполняемого файла
    pub fn run(&self, flag: &str, args: &mut Vec<String>, gdb: bool, valgrind: bool) -> std::io::Result<()> {
        crab_log!("INFO", "RUN", "Start running an executable file");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let bin_name = config.project.name;

        let path_to_bin = if flag == "debug" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir).join(CONFIG.binary_dir).join(bin_name)
        } else if flag == "release" {
            PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir).join(CONFIG.binary_dir).join(bin_name)
        } else {
            PathBuf::new()
        };

        if !path_to_bin.exists() {
            crab_log!("ERROR", "RUN", "The executable file was not found: {}", path_to_bin.display());
            crab_err!(ErrorKind::NotFound, "The executable file was not found: {}", path_to_bin.display());
        }

        let mut cmd = Command::new(&path_to_bin);
        if gdb {
            cmd.arg("--gdb");
        }
        if valgrind {
            cmd.arg("--valgrind");
        }
        if !args.is_empty() {
            cmd.args(args);
        }

        let status = cmd.status()?;

        if !status.success() {
            crab_log!("ERROR", "RUN", "Error launching the executable file: {}", status);
            crab_err!(ErrorKind::Other, "Error launching the executable file!");
        }

        crab_log!("INFO", "RUN" ,"Running an executable file: {}", path_to_bin.display());

        Ok(())
    }

    pub fn run_module(&self, name: &str, flag: &str, args: &mut Vec<String>, gdb: bool, valgrind: bool) -> std::io::Result<()> {
        crab_log!("INFO", "RUN", "Start running an executable file");
        
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let module = config.module.get(name)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("Module {} not found", name)))?;

        let bin_name = module.output_name.as_ref().filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| name.to_string());

        let path_to_mod_bin = match flag {
            "release" => {
                PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name)
                .join(CONFIG.release_dir).join(CONFIG.binary_dir).join(bin_name)
            } 

            _ => {
                PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name)
                .join(CONFIG.debug_dir).join(CONFIG.binary_dir).join(bin_name)
            }
        };

        if !path_to_mod_bin.exists() {
            crab_log!("ERROR", "RUN", "Module: The executable file was not found: {}", path_to_mod_bin.display());
            crab_err!(ErrorKind::NotFound, "The executable file was not found: {}", path_to_mod_bin.display());
        }

        let mut cmd = Command::new(&path_to_mod_bin);
        if gdb {
            cmd.arg("--gdb");
        }
        if valgrind {
            cmd.arg("--valgrind");
        }
        if !args.is_empty() {
            cmd.args(args);
        }

        let status = cmd.status()?;
        if !status.success() {
            crab_log!("ERROR", "RUN", "Module: Error launching the executable file: {}", status);
            crab_err!(ErrorKind::Other, "Error launching the executable file!");
        }

        crab_log!("INFO", "RUN", "Module: Running an executable file: {}", path_to_mod_bin.display());
        
        Ok(())
    }
}



pub struct CrabClean;

impl CrabClean {
    pub fn new() -> Self {
        CrabClean
    }

    pub fn clean(&self, flag: &str) -> std::io::Result<()> {
        let path = match flag {
            "debug" => PathBuf::from(CONFIG.build_dir).join(CONFIG.debug_dir),
            "release" => PathBuf::from(CONFIG.build_dir).join(CONFIG.release_dir),
            _ => PathBuf::from(CONFIG.build_dir)
        };

        if !path.exists() {
            crab_err!(ErrorKind::NotFound, "The directory was not found: {}", path.display());
        }

        println!("Clearing: {}", path.display());

        fs::remove_dir_all(&path)?;
        fs::create_dir(&path)?;
        crab_print!(green, "Done!");

        Ok(())
    }

    pub fn clean_module(&self, name: &str) -> std::io::Result<()> {
        crab_log!("INFO", "CLEAN", "Starting to clean up the module directory");
        let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name);

        if !path.exists() {
            crab_log!("ERROR", "CLEAN", "The directory was not found: {}", path.display());
            crab_err!(ErrorKind::NotFound, "The directory was not found: {}", path.display());
        }

        println!("Clearing: {}", path.display());
        crab_log!("INFO", "CLEAN", "Clearing: {}", path.display());

        fs::remove_dir_all(&path)?;
        fs::create_dir(&path)?;
        crab_print!(green, "Done!");

        crab_log!("INFO", "CLEAN", "Cleaning is finished");

        Ok(())
    }

    pub fn clean_lib(&self) -> std::io::Result<()> {
        crab_log!("INFO", "CLEAN", "Starting to clean up the library directory");
        let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.library_dir);

        if !path.exists() {
            crab_log!("ERROR", "CLEAN", "The directory was not found: {}", path.display());
            crab_err!(ErrorKind::NotFound, "The directory was not found: {}", path.display());
        }

        println!("Clearing: {}", &path.display());
        crab_log!("INFO", "CLEAN", "Clearing: {}", path.display());

        fs::remove_dir_all(&path)?;
        fs::create_dir(&path)?;
        crab_print!(green, "Done!");
        crab_log!("INFO", "CLEAN", "Cleaning is finished");

        Ok(())
    }

}


pub struct CrabTree {
    deps: HashMap<String, Vec<String>>,
}

impl CrabTree {
    pub fn new() -> Self {
        Self {
            deps: HashMap::new(),
        }
    }

    // Рекурсивный обход папки с фильтром по расширениям
    fn collect_files(&self, dir: &Path, exts: &[&str], out: &mut Vec<String>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    self.collect_files(&path, exts, out)?;
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if exts.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                        out.push(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
        Ok(())
    }

    // Индекс для поиска по базовым именам
    fn build_name_index(files: &[String]) -> HashMap<String, Vec<String>> {
        let mut idx: HashMap<String, Vec<String>> = HashMap::new();
        for f in files {
            if let Some(name) = Path::new(f).file_name().and_then(|n| n.to_str()) {
                idx.entry(name.to_string()).or_default().push(f.clone());
            }
        }

        idx
    }

    // Разбор #include в исходных файлах
    fn parse_includes(&mut self, files: &[String], name_index: &HashMap<String, Vec<String>>) -> std::io::Result<()> {
        for f in files {
            let file = fs::File::open(f)?;
            let reader = std::io::BufReader::new(file);

            let mut includes = Vec::new();

            for line in reader.lines() {
                let line = line?;
                let line = line.trim();

                if line.starts_with("#include") && line.contains('"') {
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line[start + 1..].find('"') {
                            let raw = &line[start + 1..start + 1 + end];
                            let base = Path::new(raw).file_name().unwrap().to_string_lossy().into_owned();

                            if let Some(candidates) = name_index.get(&base) {
                                includes.extend(candidates.clone());
                            } else {
                                includes.push(base);
                            }
                        }
                    }
                }
            }

            self.deps.insert(f.clone(), includes);
        }
        Ok(())
    }

    // Печать дерева рекурсивно
    fn print_tree_rec(&self, file: &str, prefix: &str, on_stack: &mut HashSet<String>, expanded: &mut HashSet<String>) {
        if on_stack.contains(file) {
            println!("{}|-- {} (cycle)", prefix, file);
            return;
        }

        println!("{}|-- {}", prefix, file);

        if expanded.contains(file) {
            return;
        }

        on_stack.insert(file.to_string());

        if let Some(children) = self.deps.get(file) {
            let mut children_sorted = children.clone();
            children_sorted.sort();

            let last = children_sorted.len().saturating_sub(1);

            for (i, child) in children_sorted.into_iter().enumerate() {

                let new_prefix = if i == last {
                    format!("{}    ", prefix) 
                } else {
                    format!("{}|   ", prefix)
                };

                self.print_tree_rec(&child, &new_prefix, on_stack, expanded);
            }
        }
        on_stack.remove(file);
        expanded.insert(file.to_string());
    }

    // Cтроим дерево
    pub fn tree(&mut self) -> std::io::Result<()> {

        let config: CrabConfig = load_config(CONFIG.config_file)?;

        let mut c: Vec<String> = Vec::new();
        let mut h: Vec<String> = Vec::new();

        let src_path = PathBuf::from(config.settings.source_dir);
        let head_path = PathBuf::from(config.settings.header_dir);

        self.collect_files(&src_path, &["c", "cc", "cpp", "cxx"], &mut c)?;
        self.collect_files(&head_path, &["h", "hpp", "hh"], &mut h)?;

        let mut all = c.clone();
        all.extend(h.clone());
        let name_index = Self::build_name_index(&all);

        self.parse_includes(&c, &name_index)?;
        self.parse_includes(&h, &name_index)?;

        // печать дерева для каждого cpp
        let mut sorted_c = c.clone();
        sorted_c.sort();
        for file in sorted_c {
            let mut on_stack = HashSet::new();
            let mut expanded = HashSet::new();
            println!("{}", file);
            self.print_tree_rec(&file, "", &mut on_stack, &mut expanded);
            println!();
        }

        Ok(())
    }
}
