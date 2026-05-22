use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "eunomio", version, about = "Eunomio commit-review server", args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    serve: ServeArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run one subagent against a partition and print transcript JSON
    SubagentRun(eunomio::cli::subagent_run::SubagentRunArgs),
}

#[derive(Parser, Debug)]
struct ServeArgs {
    #[arg(long, default_value_t = 3001)]
    port: u16,

    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Delete the existing sqlite db before starting. Use this when the on-disk
    /// shape no longer matches the current `CREATE TABLE` definitions in db.rs.
    /// Hidden from --help.
    #[arg(long, hide = true)]
    new: bool,

    /// Enable Cloudflare quick-tunnel sharing. Auto-starts cloudflared at boot
    /// and prints the trycloudflare URL to stdout.
    #[arg(long)]
    enable_tunnel: bool,

    /// Point `POST /api/tunnel` at the Vite dev server on `127.0.0.1:5173`
    /// and skip the share-token gate, so HMR works over the public URL.
    /// Set by `npm run dev`'s backend invocation; never set on release builds.
    /// Hidden from --help.
    #[arg(long, hide = true)]
    dev_tunnel: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,eunomio=info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Some(Commands::SubagentRun(args)) => eunomio::cli::subagent_run::run(args).await,
        None => serve(cli.serve).await,
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let data_dir = args
        .data_dir
        .or_else(|| dirs::home_dir().map(|h| h.join(".eunomio")))
        .context("could not determine data dir; pass --data-dir")?;

    tracing::info!(data_dir = %data_dir.display(), port = args.port, "starting eunomio");

    if args.new {
        let db_path = data_dir.join("eunomio.db");
        for suffix in ["", "-wal", "-shm"] {
            let p = db_path.with_file_name(format!(
                "{}{}",
                db_path.file_name().and_then(|n| n.to_str()).unwrap_or("eunomio.db"),
                suffix
            ));
            match tokio::fs::remove_file(&p).await {
                Ok(()) => tracing::warn!(path = %p.display(), "--new: deleted db file"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(anyhow::Error::new(e)
                        .context(format!("--new: failed to delete {}", p.display())));
                }
            }
        }
    }

    let launch_key_hint = std::env::var("CURSOR_API_KEY").ok();
    std::env::remove_var("CURSOR_API_KEY");

    let tunnel_enabled = args.enable_tunnel || args.dev_tunnel;

    let state = eunomio::state::build_state(
        data_dir,
        launch_key_hint,
        tunnel_enabled,
        args.dev_tunnel,
    )
    .await?;

    if tunnel_enabled {
        match state
            .tunnel
            .start(eunomio::server::router(state.clone()))
            .await
        {
            Ok(dto) => {
                if let Some(url) = dto.url {
                    println!("{url}");
                }
            }
            Err(e) if args.dev_tunnel => {
                return Err(anyhow::anyhow!("tunnel auto-start: {e:?}"));
            }
            Err(e) => {
                tracing::warn!(error = ?e, "tunnel auto-start failed; continuing without public URL");
            }
        }
    }

    eunomio::server::serve(state, args.port).await
}
