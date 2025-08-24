use std::{env, fs::{self, File, OpenOptions}, io::{BufRead, BufReader, Write}, path::Path, path::PathBuf};
use crate::{crab_err, crab_log, crab_print, func::{crab_config::CONFIG, crab_ini::{load_config, CrabConfig}}};
use std::io::ErrorKind;

pub struct CrabFind {
    path: String,
}

impl CrabFind {
    pub fn new(path: &str) -> Self {
        CrabFind { path: path.to_string()}
    }

    // Проверка указаны ли путь к библиотеки вручную
    fn is_manually(&self) -> std::io::Result<bool> {
        crab_log!("INFO", "FIND", "Checking for specified third-party libraries");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let is_lib = config.libraries.path;

        if is_lib.is_empty() {
            crab_log!("INFO", "FIND", "There are no third-party libraries specified");
            return Ok(false);
        }

        crab_log!("INFO", "FIND", "There are specified third-party libraries");
        Ok(true)
    }

    // Сборка путей к библиотекам
    fn collect_manual_libs(&self, includes: &Vec<String>) -> std::io::Result<()> {
        crab_log!("INFO", "FIND", "The beginning of collecting to the specified libraries");
        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let paths_from_config: Vec<String> = config.libraries.path;

        if paths_from_config.is_empty() {
            crab_log!("ERROR", "FIND", "No manual library paths provided: {:?}", paths_from_config);
            crab_err!(ErrorKind::NotFound, "No manual library paths provided!");
        }

        fn get_parent_include_dir(dir: &Path, name: &str) -> std::io::Result<String> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        if let Ok(found) = get_parent_include_dir(&path, name) {
                            if !found.is_empty() {
                                return Ok(found);
                            }
                        }
                    } else if let Some(filename) = path.file_name() {
                        if filename == name {
                            let parent = path.parent().unwrap().to_string_lossy().to_string();
                            return Ok(parent);
                        }
                    }
                }
            }
            Ok(String::new())
        }

        fn get_list_lib(dir: &Path, lib_name: &str, libs: &mut Vec<String>) -> std::io::Result<()> {
            let pref = ["lib", ""];

            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        get_list_lib(&path, lib_name, libs)?;
                    } else if let Some(ext) = path.extension() {
                        if ext == "a" || ext == "so" {
                            for &p in &pref {
                                if let Some(filename) = path.file_name() {
                                    if filename.to_string_lossy().starts_with(&format!("{}{}", &p, lib_name)) {
                                        let path_str = path.display().to_string();
                                        libs.push(path_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        let mut include_path: Vec<String> = Vec::new();
        let mut libs_vec: Vec<String> = Vec::new();
        let include_name = &includes[0].split("/").nth(1).unwrap().replace(['<', '>', ' '], "");
        let lib_name = &includes[0].split("/").next().unwrap().replace(['<', '>', ' '], "").to_lowercase();

        for path_str in paths_from_config {
            let path_to_lib = Path::new(&path_str);

            if !path_to_lib.exists() {
                crab_log!("WARRNIG", "FIND", "Manual lib path not found: {}", path_str);
                crab_print!("⚠️ Warning: manual lib path not found: {}", path_str);
                continue;
            }

            if let Ok(ip) = get_parent_include_dir(path_to_lib, &include_name) {
                if !ip.is_empty() {
                    include_path.push(ip);
                }
            }

            get_list_lib(path_to_lib, lib_name, &mut libs_vec)?;
        }

        if include_path.is_empty() && libs_vec.is_empty() {
            crab_log!("ERROR", "FIND", "Couldn't find includes or libs in provided paths: {:?}", include_path);
            crab_err!(ErrorKind::NotFound, "Couldn't find includes or libs in provided paths!");
        }

        let path_to_write = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir);

        crab_log!("INFO", "FIND", "Checking the existence of a file for writing specified third-party libraries");
        if !path_to_write.exists() {
            crab_log!("INFO", "FIND", "The file does not exist. Create: {}", path_to_write.display());
            fs::create_dir_all(&path_to_write)?;
        }

        self.write_include_path(&include_path)?;        
        self.write_libs_path(&libs_vec)?;

        Ok(())
    }

    // Проверка существуют ли файлы с путями и названиями библиотек и не пустые ли они
    fn is_empty_include_files(&self) -> std::io::Result<bool> {
        let path_to_include_path = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.include_file);
        let path_to_include_lib = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir).join(CONFIG.lib_file);
        
        let is_include_path = if let Ok(metadata) = fs::metadata(path_to_include_path) {
            metadata.is_file() && metadata.len() > 0
        } else {
            false
        };

        let is_include_lib = if let Ok(metadata) = fs::metadata(path_to_include_lib) {
            metadata.is_file() && metadata.len() > 0
        } else {
            false
        };

        Ok(is_include_path && is_include_lib)
    }

    // Рекурсивный сбор файлов по указаному расширению
    pub fn collect_file_with_extension(dir: &Path, extension: &str, files: &mut Vec<String>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    Self::collect_file_with_extension(&path, extension, files)?;

                } else if path.extension().map_or(false, |ext| ext == extension) {
                    let path_str = format!("{}", path.display());
                    files.push(path_str);

                }
            }
        }

        Ok(())
    }

    // Получение системных и стороних include в hpp файлах
    fn get_include(&self, path: &str, include_vector: &mut Vec<String>) -> std::io::Result<()> {
        let file = File::open(path)?;

        let reader = BufReader::new(file);
        let mut wait = 0;

        for line in reader.lines() {
            let line = line?;

            if line.trim().starts_with("#include <") {
                let without_include = line.split("<").nth(1).filter(|s| !s.is_empty()).unwrap();
                let incl = without_include.replace([' ', '<', '>'], "");

                if !include_vector.contains(&incl) {
                    crab_log!("INFO", "FIND", "System or third-party libraries: {}", incl);
                    include_vector.push(incl);
                }
                
            } else if line.trim().starts_with("#include") {
                wait = 0;
            }

            if wait > 10 {
                break;
            }

            wait += 1;

        }
        

        Ok(())
    }

    // Удаление системных библиотек из вектора
    fn delete_sys_include(&self, include_vector: &mut Vec<String>) -> std::io::Result<()> {
        crab_log!("INFO", "FIND", "Deleting system libraries: {:?}", include_vector);
        let system_headers: Vec<String> = vec![
            "algorithm".to_string(),
            "array".to_string(),
            "atomic".to_string(),
            "bit".to_string(),
            "bitset".to_string(),
            "charconv".to_string(),
            "chrono".to_string(),
            "codecvt".to_string(), // Устаревший в C++17
            "complex".to_string(),
            "condition_variable".to_string(),
            "coroutine".to_string(), // C++20
            "deque".to_string(),
            "exception".to_string(),
            "execution".to_string(), // C++17
            "filesystem".to_string(), // C++17
            "forward_list".to_string(),
            "fstream".to_string(),
            "functional".to_string(),
            "future".to_string(),
            "initializer_list".to_string(),
            "iomanip".to_string(),
            "ios".to_string(),
            "iosfwd".to_string(),
            "iostream".to_string(),
            "istream".to_string(),
            "iterator".to_string(),
            "limits".to_string(),
            "list".to_string(),
            "locale".to_string(),
            "map".to_string(),
            "memory".to_string(),
            "memory_resource".to_string(), // C++17
            "mutex".to_string(),
            "new".to_string(),
            "numeric".to_string(),
            "optional".to_string(), // C++17
            "ostream".to_string(),
            "queue".to_string(),
            "random".to_string(),
            "ranges".to_string(), // C++20
            "ratio".to_string(),
            "regex".to_string(),
            "set".to_string(),
            "shared_mutex".to_string(),
            "span".to_string(), // C++20
            "sstream".to_string(),
            "stack".to_string(),
            "stdexcept".to_string(),
            "streambuf".to_string(),
            "string".to_string(),
            "string_view".to_string(), // C++17
            "syncstream".to_string(), // C++20
            "thread".to_string(),
            "tuple".to_string(),
            "type_traits".to_string(),
            "typeindex".to_string(),
            "typeinfo".to_string(),
            "unordered_map".to_string(),
            "unordered_set".to_string(),
            "utility".to_string(),
            "valarray".to_string(),
            "variant".to_string(), // C++17
            "vector".to_string(),
            // Заголовки C, используемые в C++
            "cassert".to_string(),
            "cctype".to_string(),
            "cerrno".to_string(),
            "cfenv".to_string(),
            "cfloat".to_string(),
            "cinttypes".to_string(),
            "climits".to_string(),
            "clocale".to_string(),
            "cmath".to_string(),
            "csetjmp".to_string(),
            "csignal".to_string(),
            "cstdarg".to_string(),
            "cstddef".to_string(),
            "cstdint".to_string(),
            "cstdio".to_string(),
            "cstdlib".to_string(),
            "cstring".to_string(),
            "ctime".to_string(),
            "cuchar".to_string(),
            "cwchar".to_string(),
            "cwctype".to_string(),
            // Унаследованные заголовки C с .h
            "assert.h".to_string(),
            "ctype.h".to_string(),
            "errno.h".to_string(),
            "fenv.h".to_string(),
            "float.h".to_string(),
            "inttypes.h".to_string(),
            "limits.h".to_string(),
            "locale.h".to_string(),
            "math.h".to_string(),
            "setjmp.h".to_string(),
            "signal.h".to_string(),
            "stdarg.h".to_string(),
            "stddef.h".to_string(),
            "stdint.h".to_string(),
            "stdio.h".to_string(),
            "stdlib.h".to_string(),
            "string.h".to_string(),
            "time.h".to_string(),
            "uchar.h".to_string(),
            "wchar.h".to_string(),
            "wctype.h".to_string(),
        ];

        let set: std::collections::HashSet<_> = system_headers.iter().collect();

        include_vector.retain(|x| !set.contains(x));

        crab_log!("INFO", "FIND", "Third-party libraries: {:?}", include_vector);

        Ok(())
    }

    // Для флага -I
    fn find_header(&self, header: &str) -> std::io::Result<String> {
        let search_paths = ["./include", "/usr/include", "/usr/local/include", ];

        for &path in &search_paths {
            let full_path = Path::new(path).join(header);

            if full_path.exists() {
                return Ok(full_path.parent().unwrap().display().to_string());
            }
        }

        if let Ok(cpath) = env::var("CPATH") {
            for path in cpath.split(":") {
                let full_path = Path::new(path).join(header);
                
                if full_path.exists() {
                    return Ok(full_path.parent().unwrap().display().to_string());
                }
            }
        }

        Ok("None".to_string())
    }

    // Для флага -l
    fn find_library(&self, header: &str) -> std::io::Result<String> {
        let name_lib = header.split("/").next().unwrap().to_lowercase();
        let mut txt=  String::new();
        
        let lib_prefixes = ["lib", ""];
        let search_path: [&'static str; 4] = ["/usr/lib", "/usr/local/lib", "./lib", "/usr/lib64"];
        let lib_extensions = [".a", ".so"];

        for &sp in &search_path {

            if fs::metadata(sp).is_err() {
                continue;
            }

            let dirs = fs::read_dir(sp)?;
            for dir in dirs {
                let dir = dir?;

                if dir.path().is_file() {

                    let file_name = dir.path().file_name().unwrap().display().to_string().to_lowercase();

                    for &pr in &lib_prefixes {
                        for &ext in &lib_extensions {
                            let target = format!("{}{}", pr, name_lib);

                            if file_name.starts_with(&target) && file_name.ends_with(ext) {
                                let fmt_name = dir.path().file_name().unwrap().display().to_string();
                                let fmt_name_2 = if fmt_name.starts_with("lib") {
                                    fmt_name.replace(".a", "").replace("lib", "").replace(".so", "")
                                } else {
                                    fmt_name.replace(".a", "").replace(".so", "")
                                };
                                
                                txt.push_str(&format!("{}\n", fmt_name_2));
                            }
                        }
                    }
                }
            }
        }

        Ok(txt)
    }

    // Запись пути к hpp библиотек
    fn write_include_path(&self, include_path: &Vec<String>) -> std::io::Result<()> {
        let path_to_write = PathBuf::from(CONFIG.build_dir)
            .join(CONFIG.data_dir)
            .join(CONFIG.include_file);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)   
            .truncate(true)
            .open(path_to_write)?;

        crab_log!("INFO", "FIND", "Writing paths to third-party libraries: {:?}", include_path);
        for ip in include_path {
            writeln!(file, "{}", ip)?;
        }

        Ok(())
    }

    // Запись путей к .a и .so для сторонних библиотек
    fn write_libs_path(&self, lib_path: &Vec<String>) -> std::io::Result<()> {
        let path_to_write = PathBuf::from(CONFIG.build_dir)
            .join(CONFIG.data_dir)
            .join(CONFIG.lib_file);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)   
            .truncate(true) 
            .open(path_to_write)?;

        crab_log!("INFO", "FIND", "Writing third-party libraries: {:?}", lib_path);
        for lp in lib_path {
            writeln!(file, "{}", lp)?;
        }

        Ok(())
    }

    // Основная функция парсига стороних библиотек
    pub fn parsing_include(&self) -> std::io::Result<bool> {
        crab_log!("INFO", "FIND", "Starting to build third-party libraries");

        if self.is_empty_include_files()? {
            return Ok(true);
        }

        let path = Path::new(&self.path);

        let config: CrabConfig = load_config(CONFIG.config_file)?;
        let lang = config.settings.lang;
        
        let mut source: Vec<String> = Vec::new();
        let mut header: Vec<String> = Vec::new();
        let mut sys_includes: Vec<String> = Vec::new();

        if lang == "c" {
            Self::collect_file_with_extension(path, "c", &mut source)?;
            Self::collect_file_with_extension(path, "h", &mut header)?;
        } else {
            Self::collect_file_with_extension(path, "cpp", &mut source)?;
            Self::collect_file_with_extension(path, "hpp", &mut header)?;
        }

        crab_log!("INFO", "FIND", "Getting system and third-party libraries");
        for c in source {
            self.get_include(c.as_str(), &mut sys_includes)?;
        }

        for h in header {
            self.get_include(h.as_str(), &mut sys_includes)?;
        }

        self.delete_sys_include(&mut sys_includes)?;

        if sys_includes.is_empty() {
            return Ok(false);
        }

        if self.is_manually()? {
            self.collect_manual_libs(&sys_includes)?;
            return Ok(true);
        }

        let mut include_vec: Vec<String> = Vec::new();
        let mut lib_vec: Vec<String> = Vec::new();

        for incl_sys in sys_includes {
            include_vec.push(self.find_header(&incl_sys)?);
            lib_vec.push(self.find_library(&incl_sys)?);
        }

        let path_to_data_dir = PathBuf::from(CONFIG.build_dir).join(CONFIG.data_dir);

        if !path_to_data_dir.exists() {
            fs::create_dir(&path_to_data_dir)?;
        }

        self.write_include_path(&include_vec)?;
        self.write_libs_path(&lib_vec)?;

        crab_log!("INFO", "FIND", "End of the build of third-party libraries");

        Ok(true)
    }

}