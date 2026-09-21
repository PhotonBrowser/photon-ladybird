mod build;
mod doctor;
mod output;
mod process;

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "photon", bin_name = "photon", version, about = "Photon developer tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build Photon with Ladybird's upstream build tooling
    Build(BuildArgs),
    /// Build Photon if needed, then launch it
    Run(RunArgs),
    /// Remove the selected Ladybird build directory
    Clean,
    /// Build and run Ladybird's test suite
    Test(TestArgs),
    /// Check the local Photon development environment
    Doctor,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// Use Ladybird's Debug preset instead of Release
    #[arg(long)]
    debug: bool,
    /// Stream complete output instead of showing the status UI
    #[arg(long, short)]
    verbose: bool,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Launch the existing executable without building it first
    #[arg(long)]
    no_build: bool,
    /// Stream complete build output instead of showing the status UI
    #[arg(long, short)]
    verbose: bool,
    /// Arguments passed to the Photon application
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    application_args: Vec<OsString>,
}

#[derive(Debug, Args)]
struct TestArgs {
    /// Only run tests whose names match this regular expression
    pattern: Option<String>,
    /// Stream complete output instead of showing the status UI
    #[arg(long, short)]
    verbose: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => exit_code(code),
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let repository = repository_root()?;

    match cli.command {
        Command::Build(args) => build::build(&repository, preset(args.debug), args.verbose, None),
        Command::Run(args) => build::run(
            &repository,
            "Release",
            args.no_build,
            args.verbose,
            &args.application_args,
        ),
        Command::Clean => build::clean(&repository, "Release"),
        Command::Test(args) => build::test(&repository, "Release", args.pattern.as_deref(), args.verbose),
        Command::Doctor => doctor::run(&repository),
    }
}

fn preset(debug: bool) -> &'static str {
    if debug { "Debug" } else { "Release" }
}

fn repository_root() -> Result<PathBuf> {
    if let Some(root) = env::var_os("PHOTON_ROOT") {
        return Ok(PathBuf::from(root));
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .context("PhotonCLI must be located at Tools/PhotonCLI")
}

fn exit_code(code: i32) -> ExitCode {
    let normalized = u8::try_from(code).unwrap_or(1);
    ExitCode::from(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_selects_ladybird_debug_preset() {
        assert_eq!(preset(true), "Debug");
    }

    #[test]
    fn release_is_the_default_preset() {
        assert_eq!(preset(false), "Release");
    }
}
