use std::{fs, path::{Path, PathBuf}};
use crate::{crab_err, crab_log, crab_print, func::{crab_config::CONFIG, crab_find::CrabFind, crab_ini::{load_config, save_config, CrabConfig, Module}}};
use crate::crab_build::CrabBuild;
use std::io::ErrorKind;



pub struct CrabModule;

impl CrabModule {
    pub fn new() -> Self {
        CrabModule
    }

    fn search_dir(&self, src: &Path, name: &str) -> std::io::Result<Option<PathBuf>> {
        if src.is_dir() {
            for entry in fs::read_dir(&src)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    if let Some(dir_name) = path.file_name() {
                        if dir_name == name {
                            return Ok(Some(path));
                        } else if let Some(found) = self.search_dir(&path, name)? {
                            return Ok(Some(found));
                        }
                    }
                } else {
                    continue;
                }

            }
        }

        Ok(None)
    }

    pub fn create(&self, name: &str) -> std::io::Result<()> {   
        crab_log!("INFO", "MODULE", "Creating a module");
        let mut config: CrabConfig = load_config(CONFIG.config_file)?;
        let src = &config.settings.source_dir;
        let src_path = Path::new(&src);
        let mut cpp: Vec<String> = Vec::new();

        if config.module.contains_key(name) {
            crab_log!("ERROR", "MODULE", "Module {} already exists", name);
            crab_err!(ErrorKind::AlreadyExists, "Module {} already exists", name);
        }

        if let Some(dir) = self.search_dir(src_path, name)? {

            CrabFind::collect_file_with_extension(&dir, "cpp", &mut cpp)?;

            let module = Module {
                path: dir.display().to_string(),
                dependencies: cpp,
                output_name: Some(name.to_string()),
            };

            config.module.insert(name.to_string(), module);

            save_config(&config, CONFIG.config_file)?;

            crab_print!(green, "+ module.{}", name);

        } else {
            crab_log!("ERROR", "MODULE", "The directory was not found: {}", name);
            crab_err!(ErrorKind::NotFound, "The directory was not found: {}", name);
        }

        crab_log!("INFO", "MODULE", "The {} module has been created", name);

        Ok(())
    }

    pub fn remove(&self, name: &str) -> std::io::Result<()> {
        crab_log!("INFO", "MODULE", "Deleting a module");
        let mut config: CrabConfig = load_config(CONFIG.config_file)?;
        
        config.module.remove(name)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("Module {} not found", name)))?;

        save_config(&config, CONFIG.config_file)?;

        let path_to_model = PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name);

        crab_log!("INFO", "MODULE", "Checking for the module's existence: {}", name);
        if path_to_model.exists() {
            crab_log!("INFO", "MODULE", "The {} module exists. Delete", path_to_model.display());
            fs::remove_dir_all(path_to_model)?;
        }

        crab_print!(red, "- module.{}", name);

        crab_log!("INFO", "MODULE", "The removal of the {} module has been completed", name);

        Ok(())
    }

    pub fn build_module(&mut self, name: &str, flag: &str) -> std::io::Result<()> {
        crab_log!("INFO", "MODULE", "Starting the module build");
        let mut config: CrabConfig = load_config(CONFIG.config_file)?;
        let bin_name = config.module
            .get(name)
            .and_then(|m| m.output_name.as_ref().filter(|s| !s.is_empty()).cloned())
            .unwrap_or_else(|| name.to_string());

        let source_dir = PathBuf::from(config.settings.source_dir.clone()).join(name);
        let lang = if config.settings.lang == "c" { "c" } else { "cpp" };

        let mut files_vec = Vec::new();
        CrabFind::collect_file_with_extension(&source_dir, lang, &mut files_vec)?;

        if let Some(module) = config.module.get_mut(name) {
            module.dependencies = files_vec;
        }
        save_config(&config, CONFIG.config_file)?;
    
        if flag == "debug" {
            CrabBuild::new().debug_building(Some(name), Some(&bin_name))?;
        } else {
            CrabBuild::new().release_building(Some(name), Some(&bin_name))?;
        }

        crab_log!("INFO", "MODULE", "Module build is complete");

        Ok(())
    }


}