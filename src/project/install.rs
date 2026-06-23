use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::build::{BuildProfile, CrabBuild};
use crate::{crab_err, crab_log, crab_print, crab_status};

pub struct CrabInstall;

impl Default for CrabInstall {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabInstall {
    pub fn new() -> Self {
        CrabInstall
    }

    // ~/.local/bin на Unix; %USERPROFILE%\.local\bin на Windows
    fn default_bin_dir() -> std::io::Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| std::io::Error::new(ErrorKind::NotFound,
                "Cannot determine home directory (HOME / USERPROFILE not set)"))?;

        Ok(home.join(".local").join("bin"))
    }

    // Собрать проект и скопировать бинарник в целевой каталог
    pub fn install(&self, dest: Option<&str>, debug: bool) -> std::io::Result<()> {
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let name = config.project.name;

        let profile = if debug { BuildProfile::Debug } else { BuildProfile::Release };

        // Инкрементальная сборка
        CrabBuild::new().building(profile.clone(), None, None)?;

        // Целевой каталог
        let bin_dir = match dest {
            Some(p) => PathBuf::from(p),
            None    => Self::default_bin_dir()?,
        };

        fs::create_dir_all(&bin_dir).map_err(|e| {
            std::io::Error::new(e.kind(), format!("Cannot create {}: {}", bin_dir.display(), e))
        })?;

        let exe = std::env::consts::EXE_SUFFIX;
        let bin_name = format!("{}{}", name, exe);
        let src = PathBuf::from(CONFIG.build_dir)
            .join(profile.dir())
            .join(CONFIG.binary_dir)
            .join(&bin_name);
        let dst = bin_dir.join(&bin_name);

        if !src.exists() {
            crab_err!(ErrorKind::NotFound, "Binary not found: {}", src.display());
        }

        crab_log!("INFO", "INSTALL", "copy {} -> {}", src.display(), dst.display());
        fs::copy(&src, &dst)?;

        // Устанавливаем биты исполнения на Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
        }

        crab_status!("Installing", "{} -> {}", bin_name, dst.display());

        // Предупреждение если каталог не в PATH
        let in_path = std::env::var_os("PATH")
            .is_some_and(|p| std::env::split_paths(&p).any(|d| d == bin_dir));

        if !in_path {
            crab_print!(yellow, "note: {} is not in your PATH", bin_dir.display());
        }

        Ok(())
    }
}
