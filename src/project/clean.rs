use std::fs;
use std::path::PathBuf;

use crate::config::CONFIG;
use crate::{crab_err, crab_print, crab_log};
use std::io::ErrorKind;

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
