use std::{fs::File, io::{Read, Write}};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/*=====ОСНОВНОЙ КОНИФГ=====*/
#[derive(Deserialize, Serialize, Debug)]
pub struct CrabConfig {
    pub project: Project,
    pub settings: Settings,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub files: HashMap<String, String>,
    pub libraries: Libraries,
    pub module: HashMap<String, Module>,
}

// Пользовательские параметры сборки (секция [build] в config.toml).
// Все поля опциональны и добавляются поверх встроенных флагов профиля.
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Build {
    #[serde(default)]
    pub standard: String,          // стандарт языка, напр. "c++17" / "c11" -> -std=...
    #[serde(default)]
    pub defines: Vec<String>,      // макросы -D, напр. "DEBUG" / "VER=2"
    #[serde(default)]
    pub include_dirs: Vec<String>, // дополнительные каталоги заголовков -I
    #[serde(default)]
    pub cflags: Vec<String>,       // произвольные флаги компиляции
    #[serde(default)]
    pub ldflags: Vec<String>,      // произвольные флаги линковки
}

impl Build {
    // Аргументы, добавляемые на этапе компиляции в объектный файл
    pub fn compile_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if !self.standard.trim().is_empty() {
            args.push(format!("-std={}", self.standard));
        }
        for d in &self.defines {
            args.push(format!("-D{}", d));
        }
        for inc in &self.include_dirs {
            args.push(format!("-I{}", inc));
        }
        args.extend(self.cflags.iter().cloned());

        args
    }

    // Аргументы, добавляемые на этапе линковки
    pub fn link_args(&self) -> Vec<String> {
        self.ldflags.clone()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub created: i32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Settings {
    pub lang: String,  // "c" или "c++"
    pub compiler: String,
    pub source_dir: String,
    pub header_dir: String,
}


#[derive(Deserialize, Serialize, Debug)]
pub struct Libraries {
    pub path: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Module {
    pub path: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub output_name: Option<String>,
}

/*=====ДОП КОНИФГ=====*/
#[derive(Deserialize, Serialize, Debug)]
pub struct Changed {
    pub files: HashMap<String, String>,
}


pub fn load_config<T: DeserializeOwned>(path: &str) -> std::io::Result<T> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    toml::from_str(&contents).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}


pub fn save_config<T: Serialize>(config: &T, path: &str) -> std::io::Result<()> {
    let toml_str = toml::to_string(config).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut file = File::create(path)?;
    file.write_all(toml_str.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_build_yields_no_args() {
        let b = Build::default();
        assert!(b.compile_args().is_empty());
        assert!(b.link_args().is_empty());
    }

    #[test]
    fn build_translates_fields_to_flags() {
        let b = Build {
            standard: "c++17".to_string(),
            defines: vec!["DEBUG".to_string(), "VER=2".to_string()],
            include_dirs: vec!["third_party/include".to_string()],
            cflags: vec!["-Wpedantic".to_string()],
            ldflags: vec!["-lpthread".to_string()],
        };

        assert_eq!(
            b.compile_args(),
            vec!["-std=c++17", "-DDEBUG", "-DVER=2", "-Ithird_party/include", "-Wpedantic"]
        );
        assert_eq!(b.link_args(), vec!["-lpthread"]);
    }

    #[test]
    fn blank_standard_is_skipped() {
        let b = Build {
            standard: "   ".to_string(),
            ..Default::default()
        };
        assert!(b.compile_args().is_empty());
    }
}
