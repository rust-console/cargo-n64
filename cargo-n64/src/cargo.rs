use crate::cli;
use serde::Deserialize;
use serde_json::Error as JsonError;
use std::env;
use std::io;
use std::process::{Command, Output, Stdio};
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubcommandError {
    #[error("Command failed with I/O error")]
    IoError(#[from] io::Error),

    #[error("Command failed with exit code: {0:?}")]
    CommandError(Option<i32>),

    #[error("Command failed with UTF-8 error")]
    Utf8Error(#[from] FromUtf8Error),

    #[error("Command failed with environment error")]
    VarError(#[from] env::VarError),

    #[error("JSON error: {1}")]
    JsonError(#[source] JsonError, String),

    #[error("Couldn't get cargo-n64 executable path")]
    ExePath(#[source] io::Error),
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
    crate executable: String,
    crate target: CargoArtifactTarget,
}

#[derive(Deserialize, Debug)]
crate struct CargoArtifactTarget {
    crate name: String,
}

#[derive(Deserialize, Debug)]
struct CargoMessage {
    message: Option<CargoMessageMessage>,
}

#[derive(Deserialize, Debug)]
struct CargoMessageMessage {
    rendered: String,
}

crate fn run(args: &cli::BuildArgs) -> Result<CargoArtifact, SubcommandError> {
    let verbose = args.verbose();

    // Add -Clinker-plugin-lto if necessary
    let rustflags = env::var("RUSTFLAGS")
        .and_then(|mut var| {
            var.push_str(" -Clinker-plugin-lto");
            Ok(var)
        })
        .or_else(|e| match e {
            env::VarError::NotPresent => Ok(String::from("-Clinker-plugin-lto")),
            e => Err(e),
        })?;
    env::set_var("RUSTFLAGS", rustflags);

    // Add --release flag if necessary
    let build_args = {
        let release_flag = "--release".to_owned();

        let mut args = args.rest.clone();
        if !args.contains(&release_flag) {
            args.push(release_flag);
        }
        args
    };

    let output = Command::new(env::current_exe().map_err(SubcommandError::ExePath)?)
        .arg("xbuild")
        .arg("--message-format=json")
        .arg(format!("--target={}", args.target))
        .args(build_args)
        .stderr(Stdio::inherit())
        .run(verbose)?;

    let json = String::from_utf8(output.stdout)?;
    if output.status.success() {
        // Successful build
        parse_artifact(&json)
    } else {
        // Failed build
        let (_artifacts, errors) = split_output(&json);
        print_messages(errors)?;

        Err(SubcommandError::CommandError(output.status.code()))
    }
}

fn split_output(json: &str) -> (Vec<&str>, Vec<&str>) {
    json.trim()
        .split('\n')
        .filter(|x| {
            !x.is_empty()
                && !x.starts_with('#')
                && x.find("] cargo:").is_none()
                && x.find(r#""reason":"build-script-executed""#).is_none()
        })
        .partition(|x| x.find(r#""reason":"compiler-artifact""#).is_some())
}

fn parse_artifact(json: &str) -> Result<CargoArtifact, SubcommandError> {
    // Warnings need to be handled separately
    let (artifacts, warnings) = split_output(json);
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

        if let Some(message) = message.message {
            // TODO: Add highlighting
            eprintln!("{}", message.rendered);
        }
    }

    Ok(())
}
