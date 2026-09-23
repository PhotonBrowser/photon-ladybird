// SPDX-License-Identifier: GPL-3.0-only
mod build;
mod doctor;
mod engine;
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
    /// Sync upstream, record its base, and refresh generated engine trees
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
    /// Inspect and capture the Photon patch series
    Patches {
        #[command(subcommand)]
        command: PatchCommand,
    },
    /// Manage generated Ladybird source trees
    Engine {
        #[command(subcommand)]
        command: EngineCommand,
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
    /// Show the registered patches and generated engine tree state
    Status,
    /// Check that the complete series applies to the recorded base
    Check,
    /// Capture edits from .photon/worktree into a numbered, registered patch
    Capture(ApplyPatchArgs),
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// Create or refresh Build/Source with upstream plus the registered series
    Materialize,
    /// Create .photon/worktree for Ladybird source development
    Edit,
    /// Remove generated source trees while preserving build artifacts
    Clean,
}

#[derive(Debug, Args)]
struct ApplyPatchArgs {
    /// Short description used for the patch filename and series entry
    name: String,
    /// Area label recorded in the patch series
    #[arg(long, default_value = "General")]
    area: String,
}

#[derive(Debug, Args)]
struct SyncArgs {
    /// Only fetch upstream and report the divergence without merging
    #[arg(long)]
    fetch_only: bool,
    /// Merge the already-fetched upstream tracking ref without fetching
    #[arg(long, conflicts_with = "fetch_only")]
    no_fetch: bool,
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
    /// Run Photon with Vite and rebuild/restart after native or build configuration changes.
    /// `photon run dev` remains accepted as a shorthand for this flag.
    #[arg(long)]
    dev: bool,
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
        Command::Run(args) => {
            let (dev_mode, forwarded_args) = split_dev_args(args.dev, &args.application_args);
            if dev_mode {
                return build::run_dev(&repository, args.no_build, args.verbose, forwarded_args);
            }

            build::run(
                &repository,
                "Release",
                args.no_build,
                args.verbose,
                &args.application_args,
            )
        }
        Command::Clean => build::clean(&repository, "Release"),
        Command::Test(args) => build::test(&repository, "Release", args.pattern.as_deref(), args.verbose),
        Command::Doctor => doctor::run(&repository),
        Command::Sync(args) => sync::run(&repository, args.fetch_only, args.no_fetch),
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
        Command::Patches {
            command: PatchCommand::Capture(args),
        } => patches::capture(&repository, &args.name, &args.area),
        Command::Engine {
            command: EngineCommand::Materialize,
        } => engine::materialize(&repository),
        Command::Engine {
            command: EngineCommand::Edit,
        } => engine::edit(&repository),
        Command::Engine {
            command: EngineCommand::Clean,
        } => engine::clean(&repository),
    }
}

fn preset(debug: bool) -> &'static str {
    if debug { "Debug" } else { "Release" }
}

/// Resolve dev mode from `--dev` or the `dev` shorthand (`photon run dev`).
/// Returns whether dev mode is active and the remaining application arguments.
fn split_dev_args(dev_flag: bool, application_args: &[OsString]) -> (bool, &[OsString]) {
    if dev_flag {
        return (true, application_args);
    }
    if application_args.first().is_some_and(|argument| argument == "dev") {
        return (true, &application_args[1..]);
    }
    (false, application_args)
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

    #[test]
    fn dev_flag_enables_dev_mode_without_consuming_arguments() {
        let args = vec![OsString::from("--help")];
        let (dev_mode, forwarded) = split_dev_args(true, &args);
        assert!(dev_mode);
        assert_eq!(forwarded, args.as_slice());
    }

    #[test]
    fn positional_dev_shorthand_is_stripped_from_application_args() {
        let args = vec![OsString::from("dev"), OsString::from("https://example.com")];
        let (dev_mode, forwarded) = split_dev_args(false, &args);
        assert!(dev_mode);
        assert_eq!(forwarded, &[OsString::from("https://example.com")]);
    }

    #[test]
    fn ordinary_application_args_do_not_enable_dev_mode() {
        let args = vec![OsString::from("https://example.com")];
        let (dev_mode, forwarded) = split_dev_args(false, &args);
        assert!(!dev_mode);
        assert_eq!(forwarded, args.as_slice());
    }
}
