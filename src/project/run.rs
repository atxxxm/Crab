use std::path::PathBuf;
use std::process::Command;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::{crab_err, crab_log, crab_status};
use std::io::ErrorKind;

pub struct CrabRun;

impl Default for CrabRun {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabRun {
    pub fn new() -> Self {
        CrabRun
    }
    // Запуск исполняемого файла
    pub fn run(&self, flag: &str, args: &mut Vec<String>, gdb: bool, valgrind: bool) -> std::io::Result<()> {
        crab_log!("INFO", "RUN", "Start running an executable file");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let exe_name = format!("{}{}", config.project.name, std::env::consts::EXE_SUFFIX);

        let path_to_bin = PathBuf::from(CONFIG.build_dir).join(flag).join(CONFIG.binary_dir).join(&exe_name);

        if !path_to_bin.exists() {
            crab_log!("ERROR", "RUN", "The executable file was not found: {}", path_to_bin.display());
            crab_err!(ErrorKind::NotFound, "The executable file was not found: {}", path_to_bin.display());
        }

        crab_status!("Running", "{}", exe_name);

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
        let exe_name = format!("{}{}", bin_name, std::env::consts::EXE_SUFFIX);

        let path_to_mod_bin = match flag {
            "release" => {
                PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name)
                .join(CONFIG.release_dir).join(CONFIG.binary_dir).join(&exe_name)
            }

            _ => {
                PathBuf::from(CONFIG.build_dir).join(CONFIG.module_dir).join(name)
                .join(CONFIG.debug_dir).join(CONFIG.binary_dir).join(&exe_name)
            }
        };

        if !path_to_mod_bin.exists() {
            crab_log!("ERROR", "RUN", "Module: The executable file was not found: {}", path_to_mod_bin.display());
            crab_err!(ErrorKind::NotFound, "The executable file was not found: {}", path_to_mod_bin.display());
        }

        crab_status!("Running", "{}", exe_name);

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
