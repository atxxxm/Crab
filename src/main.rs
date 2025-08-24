use std::path::Path;

use clap::{Parser, Subcommand, ValueEnum};

mod func;
mod crab_project;
mod crab_build;

use crab::crab_module::CrabModule;
use crab::func::crab_config::CONFIG;
use crab::crab_project::{CrabProject, CrabClean, CrabRun};
use crab::crab_build::{CrabBuild};
use crate::crab_build::CrabLib;
use crate::crab_project::CrabTree;
use crate::func::crab_ini::{CrabUpdateINI};
use std::io::ErrorKind;

#[derive(Parser)]
#[command(
    name = "crab",
    author = "atom",
    version = CONFIG.version,
    about = "https://github.com/atxxxm/Crab"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project
    New {
        /// Project Name
        name: String,

        /// Initialize git (flag)
        #[arg(short, long)]
        git: bool,

        /// Project language (c or c++)
        #[arg(short = 'l', long, value_enum, default_value_t = Lang::Cpp)]
        lang: Lang,

        // CLI template (flag)
        #[arg(short, long)]
        cli: bool
    },

    /// Initialize the project in the current folder
    Init,

    /// Build the project
    #[command(alias = "b")]
    Build {
        #[command(subcommand)]
        action: Option<BuildAction>,
    },

    /// Run binary or module
    Run {
        /// Run in release mode
        #[arg(long, short = 'r')]
        release: bool,

        /// Run a specific module instead of the main binary
        #[arg(long)]
        module: Option<String>,

        /// Arguments to be passed to the binary/module
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,

        /// Enable gdb (Flag)
        #[arg(long)]
        gdb: bool,

        /// Enable valgrind (Flag)
        #[arg(long)]
        valgrind: bool
    },


    /// Clear the assembly
    #[command(alias = "c")]
    Clean {
        #[command(subcommand)]
        action: Option<CleanAction>,

    },

    /// Work with the configuration file
    Conf {

        #[command(subcommand)]
        action: ConfAction,
    },

    /// Work with modules
    #[command(alias = "m")]
    Module {
        #[command(subcommand)]
        action: ModuleAction,
    },

    /// Dependency tree
    Tree,
}

#[derive(Subcommand)]
enum ConfAction {
    /// Change the parameter
    Set {
        /// Specify the language
        #[arg(long)]
        lang: Option<Lang>,

        /// Specify the compiler
        #[arg(long)]
        compiler: Option<Compiler>,
    }
}

#[derive(Subcommand)]
enum ModuleAction {
    /// Add new module
    #[command(alias = "a")]
    Add {
        /// Module name
        name: String,
    },

    /// Remove existing module
    #[command(alias = "r")]
    Remove {
        /// Module name
        name: String,
    }
}

#[derive(Subcommand)]
enum BuildAction {
    /// Build project in debug mode (default if no subcommand is given)
    #[command(alias = "d")]
    Debug,

    /// Build project in release mode
    #[command(alias = "r")]
    Release,

    /// Build specific module
    #[command(alias = "m")]
    Module {
        /// Module name
        name: String,

        /// Release flag (short: -r)
        #[arg(long, short = 'r')]
        release: bool,
    },

     /// Build library (static or dynamic)
    #[command(alias = "l")]
    Lib {
        #[arg(value_enum)]
        mode: LibMode,
    },
}

#[derive(Subcommand)]
enum CleanAction {
    /// Clear build dir
    #[command(alias = "a")]
    All,

    /// Clear the debug directory
    #[command(alias = "d")]
    Debug,

    /// Clear the release directory
    #[command(alias = "r")]
    Release,

    /// Clear specific module
    #[command(alias = "m")]
    Module {
        /// Module name
        name: String,
    },

     /// Clear library (static or dynamic)
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
    Gcui,
    Gpp,
    Clang,
}

fn main() -> std::io::Result<()> {
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

        Commands::Conf { action } => match action {
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
                        Compiler::Gcui => "gcui",
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
