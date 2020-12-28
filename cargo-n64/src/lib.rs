#![deny(clippy::all)]
#![feature(backtrace)]
#![forbid(unsafe_code)]

mod cargo;
mod cli;
mod elf;
mod fs;
mod header;
mod ipl3;

use crate::cargo::SubcommandError;
use crate::cli::{parse_args, ArgParseError, BuildArgs, Subcommand};
use crate::elf::ElfError;
use crate::fs::FSError;
use crate::header::{N64Header, HEADER_SIZE};
use crate::ipl3::{IPL_SIZE, PROGRAM_SIZE};
use colored::Colorize;
use error_iter::ErrorIter;
use std::cmp;
use std::path::PathBuf;
use std::process;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunError {
    #[error("Argument parsing error")]
    ArgParseError(#[from] ArgParseError),

    #[error("Build error")]
    BuildError(#[from] BuildError),
}

impl ErrorIter for RunError {}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("Subcommand failed")]
    SubcommandError(#[from] SubcommandError),

    #[error("Elf parsing error")]
    ElfError(#[from] ElfError),

    #[error("Error while creating filesystem")]
    FSError(#[from] FSError),

    #[error("Elf program is larger than 1MB")]
    ProgramTooBigError,

    #[error("Empty filename")]
    EmptyFilenameError,

    #[error("Filename encoding error")]
    FilenameEncodingError,

    #[error("Could not create file `{0}`")]
    CreateFileError(String),
}

fn print_backtrace(error: &dyn std::error::Error) {
    if let Some(backtrace) = error.backtrace() {
        let backtrace = backtrace.to_string();
        if !backtrace.is_empty() {
            eprintln!("{}", backtrace);
        }
    }
}

pub fn handle_errors<E, R, T>(run: R, args: &[T])
where
    E: std::error::Error + ErrorIter,
    R: Fn(&[T]) -> Result<bool, E>,
    T: AsRef<str>,
{
    let start = Instant::now();

    match run(args) {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);
            print_backtrace(&e);

            for cause in e.chain().skip(1) {
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
pub fn run<T: AsRef<str>>(args: &[T]) -> Result<bool, RunError> {
    let args = parse_args(args)?;

    match args.subcommand.unwrap() {
        Subcommand::Build(build_args) => build(build_args, args.verbose)?,
    }

    Ok(true)
}

/// The build subcommand. Parses cli args specific to build, executes
/// `cargo build-std`, and transforms the ELF to a ROM file.
fn build(mut args: BuildArgs, verbose: usize) -> Result<(), BuildError> {
    use self::BuildError::*;

    eprintln!("{:>12} with cargo build-std", "Building".green().bold());
    let artifact = cargo::run(&args, verbose)?;

    // Set default program name
    args.name.get_or_insert(artifact.target.name);
    let args = args;

    eprintln!("{:>12} ELF to binary", "Dumping".green().bold());
    let filename = artifact.executable;
    let (entry_point, program) = elf::dump(&filename)?;

    // XXX: See https://github.com/rust-console/cargo-n64/issues/40
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

    let name = args.name.as_ref().unwrap();
    let ipl3 = args.ipl3.as_ref().unwrap();
    let mut rom = [
        &N64Header::new(entry_point, name, &program, &fs, &ipl3).to_vec()[..],
        ipl3.get_ipl(),
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
