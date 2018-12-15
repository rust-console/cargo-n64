#![feature(crate_visibility_modifier)]
#![feature(transpose_result)]
#![feature(try_trait)]
#![feature(wrapping_int_impl)]
#![warn(rust_2018_idioms)]

mod cargo;
mod ipl3;
mod cli;
mod elf;
mod fs;
mod header;

use colored::Colorize;
use failure::Fail;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use crate::cargo::SubcommandError;
use crate::cli::{ArgParseError, Args};
use crate::elf::ElfError;
use crate::fs::FSError;
use crate::header::{N64Header, HEADER_SIZE};
use crate::ipl3::{IPL_SIZE, PROGRAM_SIZE};

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

pub fn handle_errors<F, R>(run: R)
where
    F: Fail,
    R: Fn() -> Result<(), F>,
{
    let start = Instant::now();

    match run() {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);

            for cause in Fail::iter_causes(&e) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
            }

            if let Some(backtrace) = e.backtrace() {
                eprintln!("{:?}", backtrace);
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

pub fn build() -> Result<(), BuildError> {
    use self::BuildError::*;

    let mut args = cli::parse_args()?;

    eprintln!("{:>12} with cargo xbuild", "Building".green().bold());
    let artifact = cargo::run("release", &args)?;

    // Set default program name
    if args.name.len() == 0 {
        args.name = artifact.target.name;
    }
    let args = args;

    eprintln!("{:>12} ELF to binary", "Dumping".green().bold());
    let filename = artifact
        .filenames
        .first()
        .expect("Cargo build message is missing build artifacts");
    let (entry_point, program) = elf::dump(filename)?;

    // XXX: See https://github.com/parasyte/technek/issues/1
    if program.len() > 1024 * 1024 {
        Err(ProgramTooBigError)?;
    }

    let path = get_output_filename(filename)?;
    let fs = args
        .fs
        .as_ref()
        .map(|fs_path| {
            eprintln!(
                "{:>12} file system at `{}` to the ROM image",
                "Appending".green().bold(),
                fs_path,
            );

            fs::create_filesystem(&fs_path)
        })
        .transpose()?;

    eprintln!("{:>12} final ROM image", "Building".green().bold());
    create_rom_image(path, &args, entry_point, program, fs)
}

/// Creates a ROM image, generating the header and IPL3 from `args`. An optional
/// file system (FAT image) is appended to the ROM image if provided.
fn create_rom_image(
    path: PathBuf,
    args: &Args,
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
