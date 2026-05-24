// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use eunomio_auth_local::LocalAuthProvider;
use eunomio_core::traits::{
    AuthProvider, Datastore, KeyStore, NoopQuotaEnforcer, QuotaEnforcer, SandboxRuntime,
};
use eunomio_keystore_file::FileKeyStore;
use eunomio_sandbox_linux::LinuxSandboxRuntime;
use eunomio_server::{build_state, BuildStateOptions};
use eunomio_server::cursor_bridge::CursorHelperRunner;
use eunomio_helper_protocol::SubagentRunner;
use eunomio_sqlite::SqliteDatastore;
use std::path::PathBuf;
use std::sync::Arc;

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
    SubagentRun(eunomio_bin_local::cli::subagent_run::SubagentRunArgs),
}

#[derive(Parser, Debug)]
struct ServeArgs {
    #[arg(long, default_value_t = 3001)]
    port: u16,

    #[arg(long)]
    data_dir: Option<PathBuf>,

    #[arg(long, hide = true)]
    new: bool,

    #[arg(
        long,
        help = "Enable in-app Cloudflare quick tunnel: share token, mobile UI, auto-start at boot. For Vite dev with a stable URL, use npm run dev instead."
    )]
    enable_tunnel: bool,

    #[arg(
        long,
        help = "Allow API requests from *.trycloudflare.com origins (npm run dev + external tunnel). Does not start cloudflared or enable the tunnel API."
    )]
    allow_dev_url: bool,

    #[arg(long)]
    pr: Option<String>,
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
        Some(Commands::SubagentRun(args)) => eunomio_bin_local::cli::subagent_run::run(args).await,
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

    if let Some(ref url) = args.pr {
        eunomio_server::github::parse_github_pull_url(url).map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!(pull_request_url = %url, "launch pull request configured");
    }

    let tunnel_enabled = args.enable_tunnel;

    let datastore: Arc<dyn Datastore> =
        Arc::new(SqliteDatastore::open(&data_dir.join("eunomio.db")).await?);
    let keystore: Arc<dyn KeyStore> = Arc::new(FileKeyStore::new(data_dir.clone(), launch_key_hint));
    let auth: Arc<dyn AuthProvider> = Arc::new(LocalAuthProvider::new(
        datastore.clone(),
        keystore.clone(),
        data_dir.clone(),
    ));
    let sandbox: Arc<dyn SandboxRuntime> = Arc::new(LinuxSandboxRuntime::new());
    let quota: Arc<dyn QuotaEnforcer> = Arc::new(NoopQuotaEnforcer::new());
    let runner: Arc<dyn SubagentRunner> =
        Arc::new(CursorHelperRunner::new(data_dir.clone(), sandbox.clone()));

    let state = build_state(BuildStateOptions {
        data_dir,
        datastore,
        keystore,
        auth,
        runner,
        quota,
        launch_pull_request: args.pr,
        tunnel_enabled,
        allow_dev_url: args.allow_dev_url,
    })
    .await?;

    if tunnel_enabled {
        match state
            .tunnel
            .start(eunomio_server::server::router(state.clone()))
            .await
        {
            Ok(dto) => {
                if let Some(url) = dto.url {
                    println!("{url}");
                }
            }
            Err(e) => {
                tracing::warn!(error = ?e, "tunnel auto-start failed; continuing without public URL");
            }
        }
    }

    eunomio_server::server::serve(state, args.port).await
}
