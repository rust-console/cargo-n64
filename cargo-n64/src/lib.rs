#![deny(clippy::all)]
#![feature(crate_visibility_modifier)]
#![feature(try_trait)]
#![feature(wrapping_int_impl)]
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

mod cargo;
mod cli;
mod elf;
mod fs;
mod header;
mod ipl3;

use colored::Colorize;
use failure::Fail;
use std::cmp;
use std::env;
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

    #[fail(display = "xbuild argument parsing error: {}", _0)]
    XbuildArgParseError(String),

    #[fail(display = "xbuild error: {}", _0)]
    XbuildError(String), // `String` because `xargo_lib::Error` is private

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
    R: Fn() -> Result<bool, F>,
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
        Ok(print_status) => {
            if print_status {
                eprintln!(
                    "{:>12} nintendo64 target(s) in {}",
                    "Finished".green().bold(),
                    get_runtime(start)
                );
            }
        }
    };
}

/// This is the entrypoint. It is responsible for parsing the cli args common to
/// all subcommands, and ultimately executing the requested subcommand.
pub fn run() -> Result<bool, RunError> {
    use self::{BuildError::*, RunError::*};

    let args = env::args().collect::<Vec<_>>();

    // So users won't have to install an extra cargo command and worry about its version being
    // up to date, we have cargo-xbuild as a dep, and just transfer control to it when we're being
    // invoked as such.
    if args.get(1).map(|a| a == "xbuild") == Some(true) {
        let args = args.iter().skip(2);
        let args =
            xargo_lib::Args::from_raw(args).map_err(|s| RunError::from(XbuildArgParseError(s)))?;

        xargo_lib::build(args, "build", None)
            .map_err(|e| RunError::from(XbuildError(e.to_string())))?;

        return Ok(false);
    }

    let args = cli::parse_args()?;
    match args.subcommand {
        cli::Subcommand::Build => build(args)?,
        _ => return Err(UnknownSubcommand),
    }

    Ok(true)
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

const PAD_BYTE: u8 = 0xFF;
const MULTIPLE: usize = 4 * 1024 * 1024;

/// Pads the program to its minimum required size for CRC calculation
fn pad_program(program: &mut Vec<u8>) {
    program.resize(cmp::max(PROGRAM_SIZE, program.len()), PAD_BYTE);
}

/// Pads the ROM to a power of 2, or a multiple of 4 MiB. Whichever is smallest.
fn pad_rom(rom: &mut Vec<u8>) {
    let size = cmp::max(HEADER_SIZE + IPL_SIZE + PROGRAM_SIZE, rom.len()) as f64;

    let by_power_of_2 = 2.0f64.powf(size.log2().ceil());
    let by_multiple = (size / MULTIPLE as f64).ceil() * MULTIPLE as f64;

    rom.resize(
        cmp::min(by_power_of_2 as usize, by_multiple as usize),
        PAD_BYTE,
    );
}

/// Creates a ROM image, generating the header and IPL3 from `args`. An optional
/// file system (FAT image) is appended to the ROM image if provided.
fn create_rom_image(
    path: PathBuf,
    args: &BuildArgs,
    entry_point: u32,
    mut program: Vec<u8>,
    fs: Option<Vec<u8>>,
) -> Result<(), BuildError> {
    use self::BuildError::*;

    let fs = fs.unwrap_or_default();

    pad_program(&mut program);

    let mut rom = [
        &N64Header::new(entry_point, &args.name, &program, &fs, &args.ipl3).to_vec()[..],
        args.ipl3.get_ipl(),
        &program,
        &fs,
    ]
    .iter()
    .fold(Vec::new(), |mut acc, cur| {
        acc.extend_from_slice(cur);

        acc
    });

    pad_rom(&mut rom);

    std::fs::write(&path, &rom).map_err(|_| CreateFileError(path.to_string_lossy().to_string()))?;

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

#[cfg(test)]
mod tests {
    use crate::ipl3::PROGRAM_SIZE;
    use crate::{pad_program, pad_rom, PAD_BYTE};

    #[test]
    fn test_program_pad() {
        let mut program = Vec::new();

        pad_program(&mut program);

        assert_eq!(vec![PAD_BYTE; PROGRAM_SIZE], program);
    }

    #[test]
    fn test_rom_pad_power_of_two() {
        let mut rom = Vec::new();

        pad_rom(&mut rom);

        assert_eq!(vec![PAD_BYTE; 2 * 1024 * 1024], rom);
    }

    #[test]
    fn test_rom_pad_multiple_of() {
        let mut rom = vec![0; 9 * 1024 * 1024];
        let expected_size = 12 * 1024 * 1024;
        let expected_padding = expected_size - rom.len();

        pad_rom(&mut rom);

        assert_eq!(rom.len(), expected_size);
        assert_eq!(
            &vec![PAD_BYTE; expected_padding][..],
            &rom[(rom.len() - expected_padding)..]
        );
    }

    #[test]
    fn test_rom_pad_already_power_of_2() {
        let mut rom = vec![0; 2 * 1024 * 1024];

        pad_rom(&mut rom);

        assert_eq!(vec![0; 2 * 1024 * 1024], rom);
    }

    #[test]
    fn test_rom_already_multiple_of() {
        let mut rom = vec![0; 12 * 1024 * 1024];

        pad_rom(&mut rom);

        assert_eq!(vec![0; 12 * 1024 * 1024], rom);
    }
}
