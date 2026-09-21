mod build;
mod doctor;
mod metadata;
mod output;
mod patches;
mod process;
mod remote;
mod sync;
mod ui;
mod upstream;

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand};
use console::style;

fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::White.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::White.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Yellow.on_default())
}

#[derive(Debug, Parser)]
#[command(name = "photon", bin_name = "photon", version, about = "Photon developer tools", long_about = "Photon developer tools for building, running, and syncing the Photon browser.", styles = cli_styles())]
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
    /// Fetch Ladybird upstream and merge it into the current branch
    Sync(SyncArgs),
    /// Inspect or update the Photon origin remote
    Remote {
        #[command(subcommand)]
        command: RemoteCommand,
    },
    /// Inspect Ladybird upstream tracking
    Upstream {
        #[command(subcommand)]
        command: UpstreamCommand,
    },
    /// Inspect the Photon patch series
    Patches {
        #[command(subcommand)]
        command: PatchCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RemoteCommand {
    /// Show the canonical origin and the configured git remote
    Status,
    /// Point origin at a new repository and update managed files
    SetOrigin(SetOriginArgs),
}

#[derive(Debug, Args)]
struct SetOriginArgs {
    /// Git URL or owner/repo shorthand (e.g. PhotonBrowser/photon-ladybird).
    /// Omit with --ssh/--https to convert the current origin in place.
    repository: Option<String>,
    /// Use the git@github.com: SSH form for the git remote
    #[arg(long, conflicts_with = "https")]
    ssh: bool,
    /// Use the https://github.com/ form for the git remote
    #[arg(long, conflicts_with = "ssh")]
    https: bool,
}

#[derive(Debug, Subcommand)]
enum UpstreamCommand {
    /// Show the recorded base and its relationship to upstream
    Status,
}

#[derive(Debug, Subcommand)]
enum PatchCommand {
    /// Show whether each recorded patch is present
    Status,
    /// Check that the complete series applies to the recorded base
    Check,
}

#[derive(Debug, Args)]
struct SyncArgs {
    /// Only fetch upstream and report the divergence without merging
    #[arg(long)]
    fetch_only: bool,
    /// Merge the already-fetched upstream tracking ref without fetching
    #[arg(long, conflicts_with = "fetch_only")]
    no_fetch: bool,
    /// After a clean merge, verify the patch series against the new base
    /// and record it in Meta/Photon/upstream.toml
    #[arg(long, conflicts_with = "fetch_only")]
    record: bool,
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
            eprintln!("{} {error:#}", style("Error:").red().bold());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let repository = repository_root()?;

    match cli.command {
        Command::Build(args) => build::build(&repository, preset(args.debug), args.verbose, Some("Photon")),
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
        Command::Sync(args) => sync::run(&repository, args.fetch_only, args.record, args.no_fetch),
        Command::Remote {
            command: RemoteCommand::Status,
        } => remote::status(&repository),
        Command::Remote {
            command: RemoteCommand::SetOrigin(args),
        } => remote::set_origin(&repository, args.repository.as_deref(), args.ssh, args.https),
        Command::Upstream {
            command: UpstreamCommand::Status,
        } => upstream::status(&repository),
        Command::Patches {
            command: PatchCommand::Status,
        } => patches::status(&repository),
        Command::Patches {
            command: PatchCommand::Check,
        } => patches::check(&repository),
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
