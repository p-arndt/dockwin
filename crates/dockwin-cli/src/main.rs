//! `dockwin` — native Rust provisioning CLI for the dockwin engine.
//!
//! A permissively licensed (MIT OR Apache-2.0), dependency-light replacement for
//! the original PowerShell provisioning scripts. All the actual work lives in the
//! shared [`dockwin_core`] crate, which the Tauri GUI uses too — so the CLI and
//! the GUI's Setup panel run the exact same install / uninstall / start / stop
//! implementation.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use dockwin_core::ops::{self, InstallOpts};

#[derive(Parser)]
#[command(
    name = "dockwin",
    about = "Native provisioner for the dockwin WSL2 Docker engine",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show whether the dockwin distro is registered / running and dockerd reachable.
    Status,

    /// Provision the dedicated 'dockwin' WSL2 distro and install dockerd.
    Install {
        /// Path to an Ubuntu 24.04 WSL rootfs (.tar or .tar.gz).
        /// If omitted, downloads the official cloud-images noble rootfs.
        #[arg(long)]
        rootfs: Option<PathBuf>,

        /// Directory to hold the distro's ext4.vhdx
        /// (default: %LOCALAPPDATA%\dockwin\distro).
        #[arg(long)]
        install_dir: Option<PathBuf>,

        /// Override the wsl.conf placed at /etc/wsl.conf
        /// (default: distro/wsl.conf, else the embedded copy).
        #[arg(long)]
        wsl_conf: Option<PathBuf>,

        /// Override the in-distro provisioning script
        /// (default: distro/provision-inside.sh, else the embedded copy).
        #[arg(long)]
        provision_script: Option<PathBuf>,

        /// Also enable the INSECURE loopback TCP (127.0.0.1:2375) fallback.
        #[arg(long)]
        enable_tcp: bool,
    },

    /// Boot the distro and bring dockerd up.
    Start {
        /// Seconds to wait for dockerd to become reachable.
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },

    /// Stop dockerd (and optionally terminate the distro VM).
    Stop {
        /// Also `wsl --terminate` the distro to release its RAM.
        #[arg(long)]
        terminate: bool,
    },

    /// Reset a broken / dangling engine registration (`wsl --unregister` a distro
    /// whose disk image is gone) so it can be cleanly reprovisioned with `install`.
    Repair,

    /// Upgrade the in-distro Docker Engine packages to the latest in the pinned
    /// apt repo and restart dockerd (in-place; does not re-import the distro).
    Update,

    /// Tear down the distro (`wsl --unregister`) and remove docker context(s).
    Uninstall {
        /// Export the distro to a .tar before unregistering.
        #[arg(long)]
        backup: bool,
        /// Where to write the backup tar.
        #[arg(long)]
        backup_path: Option<PathBuf>,
        /// Skip the interactive confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// `docker compose up` against the dockwin engine (use this instead of the
    /// Windows `docker compose`, which targets Docker Desktop's pipe).
    Up {
        /// Path to the compose file (default: ./docker-compose.yml or ./compose.yml).
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
        /// Run in the foreground instead of detaching.
        #[arg(long)]
        foreground: bool,
    },

    /// `docker compose down` against the dockwin engine.
    Down {
        /// Path to the compose file (default: ./docker-compose.yml or ./compose.yml).
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
    },
}

/// Resolve a compose file: an explicit path, else the conventional names in cwd.
fn resolve_compose_file(file: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(f) = file {
        let abs = if f.is_absolute() {
            f
        } else {
            std::env::current_dir()?.join(f)
        };
        if !abs.is_file() {
            anyhow::bail!("compose file not found: {}", abs.display());
        }
        return Ok(abs);
    }
    let cwd = std::env::current_dir()?;
    for name in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        let p = cwd.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    anyhow::bail!(
        "no compose file found in {} (looked for docker-compose.yml / compose.yml); pass -f <file>",
        cwd.display()
    )
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Status => ops::status(),
        Commands::Install {
            rootfs,
            install_dir,
            wsl_conf,
            provision_script,
            enable_tcp,
        } => {
            let opts = InstallOpts {
                rootfs,
                install_dir,
                wsl_conf,
                provision_script,
                enable_tcp,
            };
            dockwin_core::backend::detect()
                .install(opts, &|p| dockwin_core::ops::print_progress(&p))
        }
        Commands::Repair => dockwin_core::backend::detect().repair(),
        Commands::Update => dockwin_core::backend::detect()
            .update(&|p| dockwin_core::ops::print_progress(&p)),
        Commands::Start { timeout } => dockwin_core::backend::detect().start(timeout),
        Commands::Stop { terminate } => dockwin_core::backend::detect().stop(terminate),
        Commands::Uninstall {
            backup,
            backup_path,
            yes,
        } => dockwin_core::backend::detect().uninstall(backup, backup_path, yes),
        Commands::Up { file, foreground } => resolve_compose_file(file).and_then(|f| {
            println!("==> docker compose up{} ({})", if foreground { "" } else { " -d" }, f.display());
            let mut emit = |line: &str| println!("{line}");
            dockwin_core::backend::detect().compose_up(&f, !foreground, &mut emit).and_then(|ok| {
                if ok {
                    println!("==> compose up complete.");
                    Ok(())
                } else {
                    anyhow::bail!("docker compose up reported an error")
                }
            })
        }),
        Commands::Down { file } => resolve_compose_file(file).and_then(|f| {
            println!("==> docker compose down ({})", f.display());
            let mut emit = |line: &str| println!("{line}");
            dockwin_core::backend::detect().compose_down(&f, &mut emit).and_then(|ok| {
                if ok {
                    println!("==> compose down complete.");
                    Ok(())
                } else {
                    anyhow::bail!("docker compose down reported an error")
                }
            })
        }),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e:#}");
            ExitCode::FAILURE
        }
    }
}
