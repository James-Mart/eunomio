// SPDX-License-Identifier: Apache-2.0

use crate::error::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

#[async_trait]
pub trait QuotaEnforcer: Send + Sync {
    async fn check_can_start_run(&self, org_id: &str) -> Result<(), AppError>;
    async fn record_usage(&self, org_id: &str, usage: TokenUsage) -> Result<(), AppError>;
}

pub struct NoopQuotaEnforcer;

impl NoopQuotaEnforcer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopQuotaEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QuotaEnforcer for NoopQuotaEnforcer {
    async fn check_can_start_run(&self, _org_id: &str) -> Result<(), AppError> {
        Ok(())
    }

    async fn record_usage(&self, _org_id: &str, _usage: TokenUsage) -> Result<(), AppError> {
        Ok(())
    }
}
