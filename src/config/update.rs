use super::schema::{load_config, save_config, CrabConfig};

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
