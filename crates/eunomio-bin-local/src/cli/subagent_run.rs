// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Context, Result};
use clap::Args;
use eunomio_core::types::{RunKind, Transcript};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct SubagentRunArgs {
    #[arg(long)]
    pub base_url: String,

    #[arg(long)]
    pub partition_id: String,

    #[arg(long, value_parser = parse_run_kind)]
    pub kind: RunKind,

    #[arg(long)]
    pub prompt_file: Option<PathBuf>,

    /// Session cookie value (`eunomio_local_session=…`) for authenticated API calls.
    #[arg(long, env = "EUNOMIO_SESSION_COOKIE")]
    pub session_cookie: Option<String>,
}

fn parse_run_kind(s: &str) -> Result<RunKind, String> {
    RunKind::parse(s).ok_or_else(|| format!("unknown run kind: {s}"))
}

pub async fn run(args: SubagentRunArgs) -> Result<()> {
    let base = args.base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .context("building HTTP client")?;

    let prompt_override = match args.prompt_file {
        Some(path) => Some(
            tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("reading prompt file {}", path.display()))?,
        ),
        None => None,
    };

    let mut body = serde_json::json!({ "kind": args.kind });
    if let Some(text) = prompt_override {
        body["promptOverride"] = serde_json::Value::String(text);
    }

    let cookie = args
        .session_cookie
        .as_deref()
        .context("session cookie required; pass --session-cookie or set EUNOMIO_SESSION_COOKIE")?;

    let run_resp: serde_json::Value = client
        .post(format!("{base}/api/partitions/{}/runs", args.partition_id))
        .header("X-Eunomio-Request", "1")
        .header("Cookie", cookie)
        .json(&body)
        .send()
        .await
        .context("starting run")?
        .error_for_status()
        .context("starting run returned error status")?
        .json()
        .await
        .context("decoding start run response")?;

    let run_id = run_resp["id"]
        .as_str()
        .context("start run response missing id")?
        .to_string();

    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let runs: Vec<serde_json::Value> = client
            .get(format!("{base}/api/partitions/{}/runs", args.partition_id))
            .header("Cookie", cookie)
            .send()
            .await
            .context("listing runs")?
            .error_for_status()
            .context("listing runs returned error status")?
            .json()
            .await
            .context("decoding runs list")?;

        let Some(run) = runs
            .iter()
            .find(|r| r["id"].as_str() == Some(run_id.as_str()))
        else {
            bail!(
                "run {run_id} disappeared from partition {}",
                args.partition_id
            );
        };
        let status = run["status"].as_str().context("run missing status")?;
        match status {
            "running" => continue,
            "finished" | "error" | "cancelled" => break,
            other => bail!("unexpected run status: {other}"),
        }
    }

    let transcript: Transcript = client
        .get(format!(
            "{base}/api/partitions/{}/runs/{run_id}/transcript",
            args.partition_id
        ))
        .header("Cookie", cookie)
        .send()
        .await
        .context("fetching transcript")?
        .error_for_status()
        .context("fetching transcript returned error status")?
        .json()
        .await
        .context("decoding transcript")?;

    println!("{}", serde_json::to_string_pretty(&transcript)?);
    Ok(())
}
