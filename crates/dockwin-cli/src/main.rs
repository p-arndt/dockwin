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
use dockwin_core::ops::{self, InstallOpts, LogsOpts};

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

        /// HTTP(S) proxy for the in-distro apt/curl/dockerd, e.g.
        /// http://user:pass@host:port or a local no-auth forwarder
        /// http://127.0.0.1:3128 (the reliable path for Negotiate/Kerberos
        /// proxies). Use "direct" (or "none") to force proxy-less egress.
        /// Omit to auto-detect: the WSL-injected Windows proxy is used only if
        /// it's actually reachable, otherwise provisioning goes direct.
        #[arg(long)]
        proxy: Option<String>,

        /// Comma-separated no-proxy hosts for the in-distro apt/docker
        /// (default: localhost,127.0.0.1,::1).
        #[arg(long)]
        no_proxy: Option<String>,
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

    /// Fetch (and optionally follow) logs from the dockwin engine — either a
    /// single container, or the whole compose stack when no container is given.
    ///
    /// With a `<CONTAINER>` argument this streams that one container's logs
    /// (like `docker logs`). With no container it aggregates the logs of an
    /// entire compose stack (like `docker compose logs`); point at a specific
    /// stack with `--file`.
    Logs {
        /// Container name or ID. Omit to tail the whole compose stack instead.
        container: Option<String>,
        /// Compose file for stack logs (default: ./docker-compose.yml or
        /// ./compose.yml). Only valid when no `<CONTAINER>` is given.
        #[arg(long = "file")]
        file: Option<PathBuf>,
        /// Keep the stream open and follow new output (like `docker logs -f`).
        #[arg(short = 'f', long)]
        follow: bool,
        /// Show only the last N lines first (default: all of the buffered log).
        #[arg(short = 'n', long)]
        tail: Option<u32>,
        /// Prefix every line with its timestamp.
        #[arg(short = 't', long)]
        timestamps: bool,
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
            proxy,
            no_proxy,
        } => {
            let opts = InstallOpts {
                rootfs,
                install_dir,
                wsl_conf,
                provision_script,
                enable_tcp,
                proxy,
                no_proxy,
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
        Commands::Logs {
            container,
            file,
            follow,
            tail,
            timestamps,
        } => {
            let mut emit = |line: &str| println!("{line}");
            match container {
                Some(container) => {
                    // `--file` only makes sense for whole-stack (compose) logs.
                    if file.is_some() {
                        Err(anyhow::anyhow!(
                            "--file only applies to compose-stack logs; drop the <CONTAINER> argument to tail a stack"
                        ))
                    } else {
                        let opts = LogsOpts {
                            follow,
                            timestamps,
                            tail,
                        };
                        dockwin_core::backend::detect()
                            .container_logs(&container, &opts, &mut emit)
                            .and_then(|ok| {
                                // A `--follow` stream cancelled with Ctrl-C exits
                                // non-zero, which is a normal stop — don't surface
                                // it as a CLI error. Without --follow a non-zero
                                // exit is a real failure (e.g. "No such
                                // container"), so propagate it.
                                if ok || follow {
                                    Ok(())
                                } else {
                                    anyhow::bail!(
                                        "docker logs reported an error (see output above)"
                                    )
                                }
                            })
                    }
                }
                None => resolve_compose_file(file).and_then(|f| {
                    // Build `docker compose logs [...]` honoring the flags.
                    let mut args: Vec<String> = vec!["logs".to_string()];
                    if follow {
                        args.push("--follow".to_string());
                    }
                    if timestamps {
                        args.push("--timestamps".to_string());
                    }
                    if let Some(n) = tail {
                        args.push("--tail".to_string());
                        args.push(n.to_string());
                    }
                    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    dockwin_core::backend::detect()
                        .compose_run(&f, &arg_refs, &mut emit)
                        .and_then(|ok| {
                            // Same Ctrl-C semantics as the single-container path:
                            // a cancelled `--follow` stream exits non-zero but is
                            // a clean stop; a non-following failure is real.
                            if ok || follow {
                                Ok(())
                            } else {
                                anyhow::bail!(
                                    "docker compose logs reported an error (see output above)"
                                )
                            }
                        })
                }),
            }
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e:#}");
            ExitCode::FAILURE
        }
    }
}
