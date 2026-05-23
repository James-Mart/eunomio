// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use eunomio_core::{
    traits::sandbox::{SandboxRuntime, SandboxScope, SubprocessCommand},
    AppError,
};

/// No-op today; matches ARCHITECTURE.md "Subagents are unsandboxed".
/// Future work hardens this with kernel namespaces + seccomp per HOSTED_DEPLOYMENT.md §Subagent process isolation.
pub struct LinuxSandboxRuntime;

impl LinuxSandboxRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxSandboxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxRuntime for LinuxSandboxRuntime {
    async fn wrap(
        &self,
        cmd: SubprocessCommand,
        _scope: SandboxScope,
    ) -> Result<SubprocessCommand, AppError> {
        Ok(cmd)
    }
}
