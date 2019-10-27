#![deny(clippy::all)]
#![feature(crate_visibility_modifier)]
#![feature(try_trait)]
#![feature(wrapping_int_impl)]
#![warn(rust_2018_idioms)]

mod cargo;
mod cli;
mod elf;
mod fs;
mod header;
mod ipl3;

use colored::Colorize;
use failure::Fail;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use crate::cargo::SubcommandError;
use crate::cli::{ArgParseError, Args, BuildArgs};
use crate::elf::ElfError;
use crate::fs::FSError;
use crate::header::{N64Header, HEADER_SIZE};
use crate::ipl3::{IPL_SIZE, PROGRAM_SIZE};

#[derive(Debug, Fail)]
pub enum RunError {
    #[fail(display = "Argument parsing error")]
    ArgParseError(#[cause] ArgParseError),

    #[fail(display = "Error running subcommand")]
    UnknownSubcommand,

    #[fail(display = "Build error")]
    BuildError(#[cause] BuildError),
}

impl From<ArgParseError> for RunError {
    fn from(e: ArgParseError) -> Self {
        RunError::ArgParseError(e)
    }
}

impl From<BuildError> for RunError {
    fn from(e: BuildError) -> Self {
        RunError::BuildError(e)
    }
}

#[derive(Debug, Fail)]
pub enum BuildError {
    #[fail(display = "Argument parsing error")]
    ArgParseError(#[cause] ArgParseError),

    #[fail(display = "Subcommand failed")]
    SubcommandError(#[cause] SubcommandError),

    #[fail(display = "Elf parsing error")]
    ElfError(#[cause] ElfError),

    #[fail(display = "Error while creating filesystem")]
    FSError(#[cause] FSError),

    #[fail(display = "Elf program is larger than 1MB")]
    ProgramTooBigError,

    #[fail(display = "Empty filename")]
    EmptyFilenameError,

    #[fail(display = "Filename encoding error")]
    FilenameEncodingError,

    #[fail(display = "Could not create file `{}`", _0)]
    CreateFileError(String),

    #[fail(display = "Could not write file `{}`", _0)]
    WriteFileError(String),
}

impl From<ArgParseError> for BuildError {
    fn from(e: ArgParseError) -> Self {
        BuildError::ArgParseError(e)
    }
}

impl From<SubcommandError> for BuildError {
    fn from(e: SubcommandError) -> Self {
        BuildError::SubcommandError(e)
    }
}

impl From<ElfError> for BuildError {
    fn from(e: ElfError) -> Self {
        BuildError::ElfError(e)
    }
}

impl From<FSError> for BuildError {
    fn from(e: FSError) -> Self {
        BuildError::FSError(e)
    }
}

fn print_backtrace(error: &dyn Fail) {
    if let Some(backtrace) = error.backtrace() {
        let backtrace = backtrace.to_string();
        if backtrace != "" {
            eprintln!("{}", backtrace);
        }
    }
}

pub fn handle_errors<F, R>(run: R)
where
    F: Fail,
    R: Fn() -> Result<(), F>,
{
    let start = Instant::now();

    match run() {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);
            print_backtrace(&e);

            for cause in Fail::iter_causes(&e) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
                print_backtrace(cause);
            }

            process::exit(1);
        }
        Ok(()) => {
            eprintln!(
                "{:>12} nintendo64 target(s) in {}",
                "Finished".green().bold(),
                get_runtime(start)
            );
        }
    };
}

/// This is the entrypoint. It is responsible for parsing the cli args common to
/// all subcommands, and ultimately executing the requested subcommand.
pub fn run() -> Result<(), RunError> {
    use self::RunError::*;

    let args = cli::parse_args()?;
    match args.subcommand {
        cli::Subcommand::Build => build(args)?,
        cli::Subcommand::Test => test(args)?,
        _ => return Err(UnknownSubcommand),
    }

    Ok(())
}

/// The build subcommand. Parses cli args specific to build, executes
/// `cargo xbuild`, and transforms the ELF to a ROM file.
fn build(args: Args) -> Result<(), BuildError> {
    use self::BuildError::*;

    let mut args = cli::parse_build_args(args)?;

    eprintln!("{:>12} with cargo xbuild", "Building".green().bold());
    let artifact = cargo::run(&args)?;

    // Set default program name
    if args.name.is_empty() {
        args.name = artifact.target.name;
    }
    let args = args;

    eprintln!("{:>12} ELF to binary", "Dumping".green().bold());
    let filename = artifact.executable;
    let (entry_point, program) = elf::dump(&filename)?;

    // XXX: See https://github.com/parasyte/technek/issues/1
    if program.len() > 1024 * 1024 {
        return Err(ProgramTooBigError);
    }

    let path = get_output_filename(&filename)?;
    let fs = args
        .fs
        .as_ref()
        .map(|fs_path| {
            eprintln!(
                "{:>12} file system at `{}` to the ROM image",
                "Appending".green().bold(),
                fs_path,
            );

            fs::create_filesystem(fs_path)
        })
        .transpose()?;

    eprintln!("{:>12} final ROM image", "Building".green().bold());
    create_rom_image(path, &args, entry_point, program, fs)
}

fn test(args: Args) -> Result<(), RunError> {
    build(args)?;
    eprintln!("{:>12} Build test finished.", "Testing".green().bold());
    eprintln!("{:>12} Unit test finished.", "Testing".green().bold());
    Ok(())
}

/// Creates a ROM image, generating the header and IPL3 from `args`. An optional
/// file system (FAT image) is appended to the ROM image if provided.
fn create_rom_image(
    path: PathBuf,
    args: &BuildArgs,
    entry_point: u32,
    program: Vec<u8>,
    fs: Option<Vec<u8>>,
) -> Result<(), BuildError> {
    use self::BuildError::*;

    let fs = if let Some(fs) = fs { fs } else { Vec::new() };

    let mut file =
        File::create(&path).map_err(|_| CreateFileError(path.to_string_lossy().to_string()))?;

    let header = N64Header::new(entry_point, &args.name, &program, &fs, &args.ipl3).to_vec();
    file.write_all(&header)
        .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;

    let ipl = args.ipl3.get_ipl();
    file.write_all(ipl)
        .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;

    file.write_all(&program)
        .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;

    let padding_length = (2 - (program.len() & 1)) & 1;
    let padding = [0; 1];
    file.write_all(&padding[0..padding_length])
        .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;

    file.write_all(&fs)
        .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;

    let rom_length = HEADER_SIZE + IPL_SIZE + program.len() + padding_length + fs.len();
    const ROM_SIZE: usize = PROGRAM_SIZE + HEADER_SIZE + IPL_SIZE;
    if rom_length < ROM_SIZE {
        let padding = std::iter::repeat(0)
            .take(ROM_SIZE - rom_length)
            .collect::<Vec<u8>>();
        file.write_all(&padding)
            .map_err(|_| WriteFileError(path.to_string_lossy().to_string()))?;
    }

    // TODO: Padding up to nearest 4 KB?

    Ok(())
}

fn get_output_filename(filename: &str) -> Result<PathBuf, BuildError> {
    use self::BuildError::*;

    let mut path = PathBuf::from(filename);
    let stem = path
        .file_stem()
        .ok_or(EmptyFilenameError)?
        .to_str()
        .ok_or(FilenameEncodingError)?
        .to_owned();

    path.pop();
    path.push(format!("{}.n64", stem));

    Ok(path)
}

fn get_runtime(start: Instant) -> String {
    let total = start.elapsed();
    format!("{}.{}s", total.as_secs(), total.subsec_millis())
}
