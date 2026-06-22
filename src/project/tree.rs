use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::config::{load_config, CrabConfig, CONFIG};

pub struct CrabTree {
    deps: HashMap<String, Vec<String>>,
}

impl Default for CrabTree {
    fn default() -> Self {
        Self::new()
    }
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
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && exts.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                        out.push(path.to_string_lossy().into_owned());
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

                if line.starts_with("#include") && line.contains('"')
                    && let Some(start) = line.find('"')
                        && let Some(end) = line[start + 1..].find('"') {
                            let raw = &line[start + 1..start + 1 + end];
                            let base = Path::new(raw).file_name().unwrap().to_string_lossy().into_owned();

                            if let Some(candidates) = name_index.get(&base) {
                                includes.extend(candidates.clone());
                            } else {
                                includes.push(base);
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
