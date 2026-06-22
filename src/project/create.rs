use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{Datelike, Utc};
use std::process::{Command, Stdio};

use crate::config::{save_config, Build, CrabConfig, Libraries, Project, Settings, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_print};
use std::io::{ErrorKind, Write};

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
            if is_compiler[2] {
                "gcc"
            } else {
                ""
            }
        } else if is_compiler[0] {
            "g++"
        } else if is_compiler[1] {
            "clang"
        } else {
            ""
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

            build: Build::default(),

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

        save_config(&config, path_to_config.display().to_string().as_str())?;

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
                fs::write(".gitignore", format!("{}/", CONFIG.build_dir))?;
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

                if let Some(is_skip) = rel_path.parent()
                    && is_skip.to_string_lossy() == skip {
                        continue;
                    }

                let new_path = Path::new(base_dir).join(rel_path);

                if let Some(parent) = new_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                if fs::copy(file_path, &new_path).is_ok() {
                    fs::remove_file(file_path)?;
                    remove_empty_parents(file_path, root)?;
                }
            }

            Ok(())
        }

        if !files.is_empty() {
            if !Path::new(&source_dir).exists() {
                fs::create_dir_all(source_dir)?;
            }

            copy_files(&files, source_dir, to_path, source_dir)?;
        }

        if !headers.is_empty() {
            if !Path::new(&header_dir).exists() {
                fs::create_dir_all(header_dir)?;
            }

            copy_files(&headers, header_dir, to_path, header_dir)?;
        }

        fs::File::create(CONFIG.config_file)?;
        fs::create_dir(CONFIG.build_dir)?;

        let cur_dir = std::env::current_dir()?;

        fn is_file(ext: &str, v: &[String]) -> bool {
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
