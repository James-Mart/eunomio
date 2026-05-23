// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;

#[async_trait]
pub trait KeyStore: Send + Sync {
    async fn get(&self, user_id: &str) -> anyhow::Result<Option<String>>;
    async fn set(&self, user_id: &str, key: &str) -> anyhow::Result<()>;
    fn has_launch_key_hint(&self) -> bool;
    fn take_launch_key_hint(&self) -> Option<String>;
}
