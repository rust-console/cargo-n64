use failure::Fail;
use serde_derive::Deserialize;
use serde_json::Error as JsonError;
use std::io;
use std::process::{Command, Output, Stdio};
use std::string::FromUtf8Error;

use crate::cli;

#[derive(Debug, Fail)]
pub enum SubcommandError {
    #[fail(display = "Command failed with I/O error")]
    IoError(#[cause] io::Error),

    #[fail(display = "Command failed with exit code: {:?}", _0)]
    CommandError(Option<i32>),

    #[fail(display = "Command failed with I/O error")]
    Utf8Error(#[cause] FromUtf8Error),

    #[fail(display = "JSON error: {}", _1)]
    JsonError(#[cause] JsonError, String),
}

impl From<io::Error> for SubcommandError {
    fn from(e: io::Error) -> Self {
        SubcommandError::IoError(e)
    }
}

impl From<FromUtf8Error> for SubcommandError {
    fn from(e: FromUtf8Error) -> Self {
        SubcommandError::Utf8Error(e)
    }
}

trait Runner {
    fn run(&mut self, verbose: bool) -> io::Result<Output>;
}

impl Runner for Command {
    fn run(&mut self, verbose: bool) -> io::Result<Output> {
        if verbose {
            eprintln!("+ {:?}", self);
        }

        self.output()
    }
}

#[derive(Deserialize, Debug)]
crate struct CargoArtifact {
    crate filenames: Vec<String>,
    crate target: CargoArtifactTarget,
}

#[derive(Deserialize, Debug)]
crate struct CargoArtifactTarget {
    crate name: String,
}

#[derive(Deserialize, Debug)]
struct CargoMessage {
    message: CargoMessageMessage,
}

#[derive(Deserialize, Debug)]
struct CargoMessageMessage {
    rendered: String,
}

crate fn run(profile: &str, args: &cli::Args) -> Result<CargoArtifact, SubcommandError> {
    let verbose = args.verbose();
    let output = Command::new("cargo")
        .arg("xbuild")
        .arg("--message-format=json")
        .arg(format!("--{}", profile))
        .arg(format!("--target={}", args.target))
        .args(args.rest.clone())
        .stderr(Stdio::inherit())
        .run(verbose)?;

    let json = String::from_utf8(output.stdout)?;
    if output.status.success() {
        // Successful build
        parse_artifact(&json)
    } else {
        // Failed build
        let (_artifacts, errors): (Vec<&str>, Vec<&str>) = json
            .trim()
            .split("\n")
            .partition(|x| x.find(r#""reason":"compiler-artifact""#).is_some());
        print_messages(errors)?;

        Err(SubcommandError::CommandError(output.status.code()))
    }
}

fn parse_artifact<'a>(json: &'a str) -> Result<CargoArtifact, SubcommandError> {
    // Warnings need to be handled separately
    let (artifacts, warnings): (Vec<&str>, Vec<&str>) = json
        .trim()
        .split("\n")
        .partition(|x| x.find(r#""reason":"compiler-artifact""#).is_some());
    print_messages(warnings)?;

    // Return build artifact
    let json = *artifacts.last().expect("Expected artifact JSON");
    serde_json::from_str(json).map_err(|e| SubcommandError::JsonError(e, json.into()))
}

fn print_messages<'a, T>(messages: T) -> Result<(), SubcommandError>
where
    T: IntoIterator<Item = &'a str>,
{
    for s in messages {
        let message: CargoMessage =
            serde_json::from_str(s).map_err(|e| SubcommandError::JsonError(e, s.into()))?;

        // TODO: Add highlighting
        eprintln!("{}", message.message.rendered);
    }

    Ok(())
}
