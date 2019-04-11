use failure::Fail;
use std::fs::{self, File};
use std::io::Write;
use std::{env, process};

use crate::ipl3::{IPL3Error, IPL3};

#[derive(Debug, Fail)]
pub enum ArgParseError {
    #[fail(display = "Must be invoked as cargo subcommand: `cargo n64`")]
    CargoSubcommand,

    #[fail(display = "Missing subcommand, e.g. `cargo n64 build`")]
    MissingSubcommand,

    #[fail(display = "Error creating target or linker script: {}", _0)]
    TargetCreationError(String),

    #[fail(display = "Error writing target or linker script: {}", _0)]
    TargetWriteError(String),

    #[fail(display = "Missing target value")]
    MissingTargetValue,

    #[fail(display = "Missing IPL3 value")]
    MissingIPL3Value,

    #[fail(display = "Missing name value")]
    MissingNameValue,

    #[fail(display = "Missing FS value")]
    MissingFSValue,

    #[fail(display = "FS value must be a path to a readable directory")]
    InvalidFSValue,

    #[fail(display = "IPL3 error")]
    IPL3Error(#[cause] IPL3Error),
}

impl From<IPL3Error> for ArgParseError {
    fn from(e: IPL3Error) -> Self {
        ArgParseError::IPL3Error(e)
    }
}

#[derive(Debug)]
crate enum Subcommand {
    None,
    Build,
}

#[derive(Debug)]
crate struct Args {
    crate subcommand: Subcommand,
    crate target: String,
    crate rest: Vec<String>,
}

#[derive(Debug)]
crate struct BuildArgs {
    crate target: String,
    crate name: String,
    crate fs: Option<String>,
    crate ipl3: IPL3,
    crate rest: Vec<String>,
}

impl BuildArgs {
    crate fn verbose(&self) -> bool {
        self.rest
            .iter()
            .any(|a| a == "--verbose" || a == "-v" || a == "-vv")
    }
}

crate fn parse_args() -> Result<Args, ArgParseError> {
    use self::ArgParseError::*;

    let mut args = env::args().skip(1);
    if args.next() != Some("n64".to_owned()) {
        Err(CargoSubcommand)?;
    }

    let target = create_target()?;
    let mut rest: Vec<String> = Vec::new();

    // Peek at the first arg to select the command
    let mut args = args.peekable();
    let subcommand = match args.peek().map(String::as_str) {
        Some("build") => {
            args.next();
            Subcommand::Build
        }
        _ => Subcommand::None,
    };

    // Process common arguments
    for arg in args {
        if arg == "--help" || arg == "-h" {
            eprintln!(
                include_str!("templates/help.fmt"),
                env!("CARGO_PKG_NAME"),
                target
            );
            process::exit(0);
        } else if arg == "--version" || arg == "-V" {
            eprintln!(
                "{}\nVersion {}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            );
            process::exit(0);
        } else {
            rest.push(arg.to_owned());
        }
    }

    // Check subcommand after handling --help and --version
    if let Subcommand::None = subcommand {
        Err(MissingSubcommand)?
    }

    Ok(Args {
        subcommand,
        target,
        rest,
    })
}

crate fn parse_build_args(args: Args) -> Result<BuildArgs, ArgParseError> {
    use self::ArgParseError::*;

    let mut target = args.target;
    let mut name = String::new();
    let mut fs = None;
    let mut ipl3 = None;
    let mut rest: Vec<String> = Vec::new();

    let mut args = args.rest.iter();

    // Process build subcommand arguments
    while let Some(arg) = args.next() {
        if arg.starts_with("--target") {
            if let Some("=") = arg.get(8..9) {
                target = arg[9..].to_owned();
            } else if let Some(arg) = args.next() {
                target = arg.to_owned();
            } else {
                Err(MissingTargetValue)?;
            }
        } else if arg.starts_with("--name") {
            if let Some("=") = arg.get(6..7) {
                name = arg[7..].to_owned();
            } else if let Some(arg) = args.next() {
                name = arg.to_owned();
            } else {
                Err(MissingNameValue)?;
            }
        } else if arg.starts_with("--fs") {
            let path = if let Some("=") = arg.get(4..5) {
                arg[5..].to_owned()
            } else if let Some(arg) = args.next() {
                arg.to_owned()
            } else {
                return Err(MissingFSValue);
            };

            let stat = fs::metadata(&path).map_err(|_| InvalidFSValue)?;
            if !stat.is_dir() {
                Err(InvalidFSValue)?;
            }

            fs = Some(path);
        } else if arg.starts_with("--ipl3") {
            ipl3 = Some(if let Some("=") = arg.get(5..6) {
                IPL3::read(&arg[6..])?
            } else if let Some(arg) = args.next() {
                IPL3::read(&arg)?
            } else {
                return Err(MissingIPL3Value);
            });
        } else {
            rest.push(arg.to_owned());
        }
    }

    let ipl3 = ipl3.ok_or(MissingIPL3Value)?;

    Ok(BuildArgs {
        target,
        name,
        fs,
        ipl3,
        rest,
    })
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
        path.to_string_lossy().to_string()
    }

    use self::ArgParseError::*;

    let mut path = env::temp_dir();
    path.push("n64-build");

    // Create our temporary sub-directory for storing the target files
    fs::create_dir_all(&path).map_err(|_| TargetCreationError(path_to_string(&path)))?;

    // Create the linker script first
    let mut linker_script = path.to_path_buf();
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
