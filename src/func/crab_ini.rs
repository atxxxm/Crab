use std::{fs::File, io::{Read, Write}};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/*=====ОСНОВНОЙ КОНИФГ=====*/
#[derive(Deserialize, Serialize, Debug)]
pub struct CrabConfig {
    pub project: Project,
    pub settings: Settings,
    #[serde(default)]
    pub files: HashMap<String, String>,
    pub libraries: Libraries,
    pub module: HashMap<String, Module>,
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
    let mut file = File::open(&path)?;
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


pub struct CrabUpdateINI {
    file: String,
}

impl CrabUpdateINI {
    pub fn new(filename: &str) -> Self {
        Self { file: filename.to_string() }
    }

    pub fn update_lang(&self, lang: &str) -> std::io::Result<()> {
        let mut config: CrabConfig = load_config(&self.file)?;
        config.settings.lang = lang.to_string();
        save_config(&config, &self.file)?;
        Ok(())
    }

    pub fn update_compiler(&self, compiler: &str) -> std::io::Result<()> {
        let mut config: CrabConfig = load_config(&self.file)?;
        config.settings.compiler = compiler.to_string();
        save_config(&config, &self.file)?;
        Ok(())
    }
}