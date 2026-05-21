use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "eunomia", version, about = "Eunomia commit-review server")]
struct Cli {
    #[arg(long, default_value_t = 3001)]
    port: u16,

    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Git repository to operate on. Defaults to the current working directory.
    /// Honoured as either a CLI flag or the `EUNOMIA_REPO_ROOT` env var.
    #[arg(long, env = "EUNOMIA_REPO_ROOT")]
    repo_root: Option<PathBuf>,

    #[arg(long)]
    cursor_api_key: Option<String>,

    /// Delete the existing sqlite db before starting. Use this when the on-disk
    /// shape no longer matches the current `CREATE TABLE` definitions in db.rs.
    /// Hidden from --help.
    #[arg(long, hide = true)]
    new: bool,

    /// Point `POST /api/tunnel` at the Vite dev server on `127.0.0.1:5173`
    /// and skip the share-token gate, so HMR works over the public URL.
    /// Set by `npm run dev`'s backend invocation; never set on release builds.
    /// Hidden from --help.
    #[arg(long, hide = true)]
    dev_tunnel: bool,

    /// With `--dev-tunnel`: auto-start the cloudflared tunnel at boot and
    /// print the trycloudflare URL to stdout. Lets `npm run dev` re-share
    /// automatically on every backend rebuild without UI access.
    /// Hidden from --help.
    #[arg(long, hide = true, requires = "dev_tunnel")]
    start_tunnel: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,eunomia=info")),
        )
        .init();

    let args = Cli::parse();

    let raw_repo_root = match args.repo_root.as_ref() {
        Some(p) => p.clone(),
        None => std::env::current_dir().context("reading current_dir for REPO_ROOT")?,
    };
    let repo_root = raw_repo_root.canonicalize().with_context(|| {
        format!("canonicalising REPO_ROOT {}", raw_repo_root.display())
    })?;
    if !repo_root.is_dir() {
        bail!("REPO_ROOT {} is not a directory", repo_root.display());
    }
    eunomia::git::ensure_repo(&repo_root)
        .await
        .with_context(|| format!("REPO_ROOT {} is not a git repository", repo_root.display()))?;

    let data_dir = args
        .data_dir
        .or_else(|| dirs::home_dir().map(|h| h.join(".eunomia")))
        .context("could not determine data dir; pass --data-dir")?;

    tracing::info!(repo_root = %repo_root.display(), data_dir = %data_dir.display(), port = args.port, "starting eunomia");

    if args.new {
        let db_path = data_dir.join("eunomia.db");
        for suffix in ["", "-wal", "-shm"] {
            let p = db_path.with_file_name(format!(
                "{}{}",
                db_path.file_name().and_then(|n| n.to_str()).unwrap_or("eunomia.db"),
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

    let cursor_api_key = args
        .cursor_api_key
        .clone()
        .or_else(|| std::env::var("CURSOR_API_KEY").ok());
    std::env::remove_var("CURSOR_API_KEY");

    let state =
        eunomia::state::build_state(repo_root, data_dir, cursor_api_key, args.dev_tunnel).await?;

    if args.start_tunnel {
        let dto = state
            .tunnel
            .start(eunomia::server::router(state.clone()))
            .await
            .map_err(|e| anyhow::anyhow!("--start-tunnel: {e:?}"))?;
        let url = dto
            .url
            .context("--start-tunnel: tunnel reported running but no URL")?;
        println!("{url}");
    }

    eunomia::server::serve(state, args.port).await
}
