use crate::ipl3::IPL3;
use gumdrop::Options;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgParseError {
    #[error("Must be invoked as cargo subcommand: `cargo n64`")]
    CargoSubcommand,

    #[error("Argument parsing error")]
    Gumdrop(#[from] gumdrop::Error),

    #[error("One of `--ipl3` or `--ipl3-from-rom` are required")]
    MissingIPL3Value,

    #[error("`--ipl3` and `--ipl3-from-rom` are mutually exclusive")]
    AmbiguousIPL3Value,

    #[error("Error creating target or linker script: {0}")]
    TargetCreationError(String),

    #[error("Error writing target or linker script: {0}")]
    TargetWriteError(String),
}

#[derive(Debug, Options)]
pub(crate) struct Args {
    /// Print help info and exit
    #[options()]
    pub(crate) help: bool,

    /// Print version info and exit
    #[options(short = "V")]
    pub(crate) version: bool,

    /// Set verbosity, can be used multiple times
    #[options(short = "v", count)]
    pub(crate) verbose: usize,

    /// Available subcommands
    #[options(command)]
    pub(crate) subcommand: Option<Subcommand>,
}

#[derive(Debug, Options)]
pub(crate) enum Subcommand {
    /// Build an executable ROM for Nintendo 64
    #[options()]
    Build(BuildArgs),

    /// Build the Rust sysroot for the Nintendo 64 target
    #[options()]
    Xbuild(XBuildArgs),
}

#[derive(Debug, Options)]
pub(crate) struct BuildArgs {
    /// Build target triple
    #[options()]
    pub(crate) target: Option<String>,

    /// Program name (Default: Crate name)
    #[options()]
    pub(crate) name: Option<String>,

    /// Path to a directory for creating the embedded file system
    #[options()]
    pub(crate) fs: Option<String>,

    /// Path to IPL3 (bootcode)
    #[options(meta = "PATH", parse(try_from_str = "IPL3::read"))]
    pub(crate) ipl3: Option<IPL3>,

    /// Path to ROM where IPL3 (bootcode) will be extracted
    #[options(meta = "PATH", parse(try_from_str = "IPL3::read_from_rom"))]
    pub(crate) ipl3_from_rom: Option<IPL3>,

    /// All remaining arguments will be passed directly to cargo-xbuild
    #[options(free)]
    pub(crate) rest: Vec<String>,
}

fn print_usage(args: Args) {
    println!("{}", env!("CARGO_PKG_NAME"));
    println!("Nintendo 64 build tool");
    println!();
    println!("Usage:");

    let command = match args.subcommand {
        Some(Subcommand::Build(_)) => "build",
        Some(Subcommand::Xbuild(_)) => "xbuild",
        None => "<COMMAND>",
    };
    println!("  cargo n64 {} [OPTIONS]", command);
    println!();
    println!("{}", args.self_usage());
    println!();

    let commands = args.self_command_list();
    if let Some(commands) = commands {
        println!("Commands:");
        println!("{}", commands);
    }
}

#[derive(Debug, Options)]
pub(crate) struct XBuildArgs {
    /// All arguments will be passed directly to cargo-xbuild
    #[options(free)]
    pub(crate) rest: Vec<String>,
}

pub(crate) fn parse_args<T: AsRef<str>>(args: &[T]) -> Result<Args, ArgParseError> {
    use self::ArgParseError::*;

    let mut args = args.iter();
    if args.next().map(|x| x.as_ref()) != Some("n64") {
        return Err(CargoSubcommand);
    }

    let args: Vec<_> = args.collect();
    let mut args = Args::parse_args_default(&args)?;

    // Print usage info
    if args.help {
        print_usage(args);
        process::exit(0);
    }

    if let Some(ref mut subcommand) = args.subcommand {
        if let Subcommand::Build(ref mut build_args) = subcommand {
            // IPL3 args are required and mutually exclusive
            if build_args.ipl3.is_none() && build_args.ipl3_from_rom.is_none() {
                return Err(MissingIPL3Value);
            }
            if build_args.ipl3.is_some() && build_args.ipl3_from_rom.is_some() {
                return Err(AmbiguousIPL3Value);
            }

            // Set default target
            build_args.target.get_or_insert(create_target()?);
        }
    }

    Ok(args)
}

/// Create a target triple JSON file and linker script in a temporary directory.
/// This is necessary because we don't want users to have to specify the
/// `--target` option on every build, and we have practically no chance to get
/// it into the compiler as a default target. Just being realistic. :P
///
/// Both files are compiled into the executable, the JSON is a template because
/// it needs a path reference to the linker script.
fn create_target() -> Result<String, ArgParseError> {
    // Sad, but this little helper function really simplifies the error handling
    fn path_to_string(path: &std::path::Path) -> String {
        path.to_string_lossy().to_string().replace("\\", "/")
    }

    use self::ArgParseError::*;

    let mut path = env::temp_dir();
    path.push("n64-build");

    // Create our temporary sub-directory for storing the target files
    fs::create_dir_all(&path).map_err(|_| TargetCreationError(path_to_string(&path)))?;

    // Create the linker script first
    let mut linker_script = path.clone();
    linker_script.push("linker.ld");
    let mut file = File::create(&linker_script)
        .map_err(|_| TargetCreationError(path_to_string(&linker_script)))?;
    file.write_all(include_bytes!("templates/linker.ld"))
        .map_err(|_| TargetWriteError(path_to_string(&linker_script)))?;

    // Create the target spec next
    path.push("mips-nintendo64-none.json");
    let mut file = File::create(&path).map_err(|_| TargetCreationError(path_to_string(&path)))?;
    let data = format!(
        include_str!("templates/mips-nintendo64-none.fmt"),
        path_to_string(&linker_script)
    );
    file.write_all(data.as_bytes())
        .map_err(|_| TargetWriteError(path_to_string(&path)))?;

    Ok(path_to_string(&path))
}
