use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{load_config, CrabConfig, CONFIG};
use crate::find::CrabFind;
use crate::{crab_err, crab_log, crab_status};
use super::binary::BuildProfile;
use super::helpers::CrabBuildFunc;
use std::io::ErrorKind;

// Запись в формате Clang Compilation Database (compile_commands.json)
#[derive(Serialize)]
struct Entry {
    directory: String,
    file: String,
    arguments: Vec<String>,
}

pub struct CrabCompDb;

impl Default for CrabCompDb {
    fn default() -> Self {
        Self::new()
    }
}

impl CrabCompDb {
    pub fn new() -> Self {
        CrabCompDb
    }

    // Чтение каталогов сторонних заголовков (-I) из данных детекта
    fn third_party_includes(&self) -> std::io::Result<Vec<String>> {
        let path = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.include_file);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let reader = BufReader::new(File::open(path)?);
        let mut flags = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                flags.push(format!("-I{}", line.trim()));
            }
        }

        Ok(flags)
    }

    // Генерация compile_commands.json в корне проекта
    pub fn generate(&self, profile: BuildProfile) -> std::io::Result<()> {
        crab_log!("INFO", "COMPDB", "Generating compile_commands.json");

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let compiler = config.settings.compiler;
        let lang = config.settings.lang;
        let source_dir = config.settings.source_dir;
        let header_dir = config.settings.header_dir;

        let cbf = CrabBuildFunc::new();

        // исходники проекта
        let ext = if lang == "c" { "c" } else { "cpp" };
        let mut sources: Vec<String> = Vec::new();
        CrabBuildFunc::collect_file_with_extension(Path::new(&source_dir), ext, &mut sources)?;

        if sources.is_empty() {
            crab_err!(ErrorKind::NotFound, "There are no source files");
        }
        sources.sort();

        // освежаем детект сторонних библиотек, чтобы -I были актуальны
        CrabFind::new(".").parsing_include()?;

        // общие для всех файлов флаги: -I заголовков проекта + сторонних + флаги профиля + пользовательские
        let mut common: Vec<String> = Vec::new();
        if cbf.is_header()? {
            common.push(format!("-I{}", header_dir));
        }
        common.extend(self.third_party_includes()?);
        common.extend(profile.compile_flags().iter().map(|s| s.to_string()));
        common.extend(config.build.compile_args());

        let directory = std::env::current_dir()?.display().to_string();
        let obj_dir = format!("{}/{}/{}", CONFIG.build_dir, profile.dir(), CONFIG.object_dir);

        let entries: Vec<Entry> = sources
            .iter()
            .map(|src| {
                let stem = Path::new(src)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| src.clone());
                let obj = format!("{}/{}.o", obj_dir, stem);

                let mut arguments = vec![
                    compiler.clone(),
                    "-c".to_string(),
                    src.clone(),
                    "-o".to_string(),
                    obj,
                ];
                arguments.extend(common.iter().cloned());

                Entry {
                    directory: directory.clone(),
                    file: src.clone(),
                    arguments,
                }
            })
            .collect();

        let json = serde_json::to_string_pretty(&entries)
            .map_err(std::io::Error::other)?;

        let mut file = File::create("compile_commands.json")?;
        file.write_all(json.as_bytes())?;

        crab_status!("Generated", "compile_commands.json ({} files)", entries.len());
        crab_log!("INFO", "COMPDB", "compile_commands.json written: {} entries", entries.len());

        Ok(())
    }
}
