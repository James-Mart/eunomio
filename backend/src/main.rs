use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "eunomia", version, about = "Eunomia commit-review server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    serve: ServeArgs,
}

#[derive(Subcommand, Debug)]
enum Command {
    Serve(ServeArgs),
}

#[derive(clap::Args, Debug, Clone)]
struct ServeArgs {
    #[arg(long, default_value_t = 3001)]
    port: u16,

    #[arg(long)]
    data_dir: Option<PathBuf>,

    #[arg(long)]
    no_open: bool,

    #[arg(long, conflicts_with = "no_open")]
    open: bool,

    #[arg(long)]
    cursor_api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,eunomia=info")),
        )
        .init();

    let cli = Cli::parse();
    let args = match cli.command {
        Some(Command::Serve(s)) => s,
        None => cli.serve,
    };

    let repo_root = std::env::current_dir()
        .context("reading current_dir for REPO_ROOT")?
        .canonicalize()
        .context("canonicalising REPO_ROOT")?;

    let data_dir = args
        .data_dir
        .or_else(|| dirs::home_dir().map(|h| h.join(".eunomia")))
        .context("could not determine data dir; pass --data-dir")?;

    tracing::info!(repo_root = %repo_root.display(), data_dir = %data_dir.display(), port = args.port, "starting eunomia");

    let should_open = !args.no_open && (args.open || std::io::stdout().is_terminal());
    if should_open {
        let url = format!("http://localhost:{}", args.port);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if let Err(e) = open_in_browser(&url) {
                tracing::debug!(error = %e, "could not open browser");
            }
        });
    }

    let cursor_api_key = args
        .cursor_api_key
        .clone()
        .or_else(|| std::env::var("CURSOR_API_KEY").ok());

    let state = eunomia::server::build_state(repo_root, data_dir, cursor_api_key).await?;
    eunomia::server::serve(state, args.port).await
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}
