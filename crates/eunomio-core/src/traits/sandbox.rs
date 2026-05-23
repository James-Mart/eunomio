// SPDX-License-Identifier: Apache-2.0

use crate::error::AppError;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SubprocessCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub stdin_json: Option<Vec<u8>>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SandboxScope {
    pub org_id: String,
    pub user_id: String,
    pub session_id: String,
    pub partition_id: String,
    pub run_id: String,
}

#[async_trait]
pub trait SandboxRuntime: Send + Sync {
    async fn wrap(
        &self,
        cmd: SubprocessCommand,
        scope: SandboxScope,
    ) -> Result<SubprocessCommand, AppError>;
}
