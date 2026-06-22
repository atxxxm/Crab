use std::path::Path;

use clap::{Parser, Subcommand, ValueEnum};

use crab::crab_err;
use crab::config::{CrabUpdateINI, CONFIG};
use crab::build::{CrabBuild, CrabLib};
use crab::project::{CrabClean, CrabProject, CrabRun, CrabTree};
use crab::module::CrabModule;
use std::io::ErrorKind;

#[derive(Parser)]
#[command(
    name = "crab",
    author = "atom",
    version = CONFIG.version,
    about = "A build tool for C and C++ projects",
    long_about = "Crab — a simple build tool for C and C++ projects.\n\n\
Create projects, build incrementally in debug or release, run the binary \
or individual modules, build static/dynamic libraries and inspect the \
#include dependency tree.",
    propagate_version = true,
    arg_required_else_help = true,
    after_help = "Examples:\n  \
crab new myapp --lang cpp --git\n  \
crab build              # debug build\n  \
crab build release\n  \
crab run -r -- --port 8080\n  \
crab module add net && crab build module net\n\n\
Project home: https://github.com/atxxxm/Crab"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project in a new directory
    #[command(after_help = "Examples:\n  crab new myapp\n  crab new mylib --lang c --git\n  crab new tool --cli")]
    New {
        /// Project name (used as the directory name)
        #[arg(value_name = "NAME")]
        name: String,

        /// Initialize a git repository and add a .gitignore
        #[arg(short, long)]
        git: bool,

        /// Project language
        #[arg(short = 'l', long, value_enum, default_value_t = Lang::Cpp, value_name = "LANG")]
        lang: Lang,

        /// Use the CLI main() template (int argc, char *argv[])
        #[arg(short, long)]
        cli: bool
    },

    /// Initialize a project in the current folder
    Init,

    /// Compile the project (debug by default)
    #[command(alias = "b", after_help = "Examples:\n  crab build\n  crab build release\n  crab build module net -r\n  crab build lib static")]
    Build {
        #[command(subcommand)]
        action: Option<BuildAction>,
    },

    /// Run the compiled binary or a module
    #[command(after_help = "Examples:\n  crab run\n  crab run -r\n  crab run -m net\n  crab run -- arg1 arg2")]
    Run {
        /// Run the release build instead of debug
        #[arg(long, short = 'r')]
        release: bool,

        /// Run the given module instead of the main binary
        #[arg(long, short = 'm', value_name = "NAME")]
        module: Option<String>,

        /// Arguments forwarded to the program (place them after --)
        #[arg(trailing_var_arg = true, value_name = "ARGS")]
        args: Vec<String>,

        /// Forward a --gdb flag to the program
        #[arg(long)]
        gdb: bool,

        /// Forward a --valgrind flag to the program
        #[arg(long)]
        valgrind: bool
    },

    /// Remove build artifacts
    #[command(alias = "c", after_help = "Examples:\n  crab clean\n  crab clean debug\n  crab clean module net\n  crab clean lib")]
    Clean {
        #[command(subcommand)]
        action: Option<CleanAction>,
    },

    /// View or change project settings
    #[command(name = "config", visible_alias = "conf")]
    Config {
        #[command(subcommand)]
        action: ConfAction,
    },

    /// Manage modules (add, remove, build)
    #[command(alias = "m", after_help = "Examples:\n  crab module add net\n  crab module remove net")]
    Module {
        #[command(subcommand)]
        action: ModuleAction,
    },

    /// Print the #include dependency tree
    Tree,
}

#[derive(Subcommand)]
enum ConfAction {
    /// Change a setting (language and/or compiler)
    #[command(after_help = "Examples:\n  crab config set --lang c\n  crab config set --compiler clang")]
    Set {
        /// Set the project language
        #[arg(long, value_name = "LANG")]
        lang: Option<Lang>,

        /// Set the compiler
        #[arg(long, value_name = "COMPILER")]
        compiler: Option<Compiler>,
    }
}

#[derive(Subcommand)]
enum ModuleAction {
    /// Add a module from a subdirectory of the source dir
    #[command(alias = "a")]
    Add {
        /// Module name (matches a subdirectory under the source dir)
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Remove a module and its build artifacts
    #[command(alias = "r")]
    Remove {
        /// Module name
        #[arg(value_name = "NAME")]
        name: String,
    }
}

#[derive(Subcommand)]
enum BuildAction {
    /// Debug build with warnings and no optimization (default)
    #[command(alias = "d")]
    Debug,

    /// Optimized release build
    #[command(alias = "r")]
    Release,

    /// Build a specific module
    #[command(alias = "m")]
    Module {
        /// Module name
        #[arg(value_name = "NAME")]
        name: String,

        /// Build the module in release mode
        #[arg(long, short = 'r')]
        release: bool,
    },

    /// Build a static or dynamic library
    #[command(alias = "l")]
    Lib {
        #[arg(value_enum, value_name = "MODE")]
        mode: LibMode,
    },
}

#[derive(Subcommand)]
enum CleanAction {
    /// Remove the entire build directory (default)
    #[command(alias = "a")]
    All,

    /// Remove only the debug build
    #[command(alias = "d")]
    Debug,

    /// Remove only the release build
    #[command(alias = "r")]
    Release,

    /// Remove a specific module's build
    #[command(alias = "m")]
    Module {
        /// Module name
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Remove built libraries
    #[command(alias = "l")]
    Lib,
}


#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum LibMode {
    Static,
    Dynamic,
}


#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Lang {
    C,
    Cpp,
}


#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Compiler {
    Gcc,
    Gpp,
    Clang,
}

pub fn run() -> std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, git, lang , cli} => {
            if !is_valid_project_name(&name) {
                crab_err!(ErrorKind::InvalidFilename, "Invalid project name: {}", name);
            }

            if CrabProject::new(&name).is_exists()? {
                return Ok(());
            }

            let lang_str = match lang {
                Lang::C => "c",
                Lang::Cpp => "c++"
            };

            CrabProject::new(&name).create(git, lang_str, cli)?;
        }

        Commands::Init => {
            CrabProject::new("None").init()?;
        }

        Commands::Build { action } => {
            if !Path::new(CONFIG.config_file).exists() {
                crab_err!(ErrorKind::Other, "The current directory is not a project");
            }

            let build_mode = CrabBuild::new();

            match action.unwrap_or(BuildAction::Debug) {
                BuildAction::Debug => {
                    build_mode.debug_building(None, None)?;
                }

                BuildAction::Release => {
                    build_mode.release_building(None, None)?;
                }

                BuildAction::Module { name, release } => {
                    if release {
                        CrabModule::new().build_module(&name, "release")?;
                    } else {
                        CrabModule::new().build_module(&name, "debug")?;
                    }
                }

                BuildAction::Lib { mode } => {
                    match mode {
                        LibMode::Static => CrabLib::new().static_lib_build()?,
                        LibMode::Dynamic => CrabLib::new().dynamic_lib_build()?,
                    }
                }
            }
        }

        Commands::Run { release, module, mut args , gdb, valgrind} => {
            if !Path::new(CONFIG.config_file).exists() {
                crab_err!(ErrorKind::Other, "The current directory is not a project");
            }

            let mode = if release { "release" } else { "debug" };
            let runner = CrabRun::new();

            if let Some(module_name) = module {
                runner.run_module(&module_name, mode, &mut args, gdb, valgrind)?;
            } else {
                runner.run(mode, &mut args, gdb, valgrind)?;
            }
        }

        Commands::Clean { action } => {
            if !Path::new(CONFIG.config_file).exists() {
                crab_err!(ErrorKind::Other, "The current directory is not a project");
            }

            let clean = CrabClean;

            match action.unwrap_or(CleanAction::All) {
                CleanAction::All => clean.clean("all")?,
                CleanAction::Debug => clean.clean("debug")?,
                CleanAction::Release => clean.clean("release")?,

                CleanAction::Module { name } => {
                    clean.clean_module(&name)?;
                }

                CleanAction::Lib => clean.clean_lib()?,
            }
        }

        Commands::Config { action } => match action {
            ConfAction::Set { lang, compiler} => {
                if !Path::new(CONFIG.config_file).exists() {
                    crab_err!(ErrorKind::Other, "The current directory is not a project");
                }

                let cui = CrabUpdateINI::new(CONFIG.config_file);

                if let Some(lang) = lang {
                    let lang_str = match lang {
                        Lang::C => "c",
                        Lang::Cpp => "c++"
                    };
                    cui.update_lang(lang_str)?;
                }

                if let Some(compiler) = compiler {
                    let compiler_str = match compiler {
                        Compiler::Gcc => "gcc",
                        Compiler::Gpp => "g++",
                        Compiler::Clang => "clang"
                    };
                    cui.update_compiler(compiler_str)?;
                }

            }
        },

        Commands::Module { action } => {
            if !Path::new(CONFIG.config_file).exists() {
                crab_err!(ErrorKind::Other, "The current directory is not a project");
            }

            match action {
                ModuleAction::Add { name } => {
                    CrabModule::new().create(&name)?;
                }

                ModuleAction::Remove { name } => {
                    CrabModule::new().remove(&name)?;
                }
            }
        }

        Commands::Tree => {
            CrabTree::new().tree()?;
        }
    }

    Ok(())
}

fn is_valid_project_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        && !name.starts_with('-')
        && name.len() <= 50
}
