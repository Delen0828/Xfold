use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use xfold::{api, engine::Engine, jobs, model::*, repo, store::*};

#[derive(Parser)]
#[command(
    version,
    about = "Explore committed code as semantic graphs",
    subcommand_precedence_over_arg = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(default_value = ".")]
    repository: PathBuf,
    #[arg(long = "ref", default_value = "HEAD")]
    revision: String,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long)]
    no_open: bool,
    #[arg(long)]
    include_excluded: bool,
    #[arg(long, value_delimiter = ',')]
    extensions: Vec<String>,
}
#[derive(Subcommand)]
enum Command {
    Build(BuildArgs),
    /// Emit the OpenAPI contract derived from backend DTOs.
    Openapi,
    #[command(hide = true)]
    Worker(WorkerArgs),
}
#[derive(Args)]
struct BuildArgs {
    #[arg(default_value = ".")]
    repository: PathBuf,
    #[arg(long = "ref", default_value = "HEAD")]
    revision: String,
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    include_excluded: bool,
    #[arg(long, value_delimiter = ',')]
    extensions: Vec<String>,
}
#[derive(Args)]
struct WorkerArgs {
    #[arg(long)]
    repository: PathBuf,
    #[arg(long)]
    cache_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Xfold: {e:#}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Openapi) => {
            println!("{}", serde_json::to_string_pretty(&api::openapi())?);
            return Ok(());
        }
        Some(Command::Worker(w)) => return jobs::worker_main(w.repository, w.cache_dir),
        Some(Command::Build(b)) => {
            let git = repo::open(&b.repository)?;
            let store =
                Store::open(cache_root(b.cache_dir.as_deref())?.join(repo::identity(&git)))?;
            let mut engine = Engine::new(&b.repository, store.clone())?;
            let (s, r) = engine.build(
                &BuildRequest {
                    revision: b.revision,
                    profile: Profile {
                        include_excluded: b.include_excluded,
                        extensions: b.extensions,
                    },
                    force: b.force,
                },
                |phase, n| eprintln!("{phase}: {n}"),
            )?;
            if r.stats.mode != "cache-hit" {
                store.publish(&s, &r)?;
            }
            if b.json {
                println!("{}", serde_json::to_string(&r)?);
            } else {
                println!(
                    "{} · {} modules · {} dependencies · {} ms ({})",
                    s.commit,
                    s.modules.len(),
                    s.edges.len(),
                    r.stats.duration_ms,
                    r.stats.mode
                );
            }
            return Ok(());
        }
        None => {}
    }
    let git = repo::open(&cli.repository)?;
    let default = repo::resolve(&git, cli.to.as_deref().unwrap_or(&cli.revision))?;
    let store = Store::open(cache_root(cli.cache_dir.as_deref())?.join(repo::identity(&git)))?;
    let profile = Profile {
        include_excluded: cli.include_excluded,
        extensions: cli.extensions,
    };
    let coordinator = jobs::Coordinator::new(repo::repository_path(&cli.repository)?, store);
    let ui_profile = profile.clone();
    let initial = if let Some(base) = cli.from {
        coordinator
            .comparison(CompareRequest {
                base,
                target: cli.to.unwrap_or(cli.revision),
                profile,
            })
            .await?
    } else {
        coordinator
            .snapshot(BuildRequest {
                revision: cli.to.unwrap_or(cli.revision),
                profile,
                force: false,
            })
            .await?
    };
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, cli.port))
        .await
        .context("Cannot bind the loopback server")?;
    let host = listener.local_addr()?.to_string();
    let url = format!("http://{host}");
    let state = api::AppState::new(coordinator, initial, default, host, ui_profile);
    println!("Xfold is serving {url}");
    if !cli.no_open {
        open_browser(&url);
    }
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result: std::io::Result<std::process::Child> =
        Err(std::io::Error::other("Unsupported browser opener"));
    if let Err(e) = result {
        eprintln!("Open {url} in your browser ({e})");
    }
}
