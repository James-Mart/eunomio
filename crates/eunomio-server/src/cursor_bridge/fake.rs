// SPDX-License-Identifier: Apache-2.0

use eunomio_core::{
    types::{CursorModel, ModelParamDef, ModelParamValue, ModelParamValueOption, ModelVariant},
    AppError,
};
use eunomio_helper_protocol::{HelperEvent, RunHandle, RunRequest, SubagentRunner};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, Mutex};

pub struct FakeSubagentRunner {
    scripts: Mutex<Vec<Vec<HelperEvent>>>,
    requests: Mutex<Vec<RunRequest>>,
    spawned: AtomicUsize,
}

impl FakeSubagentRunner {
    pub fn new(scripts: Vec<Vec<HelperEvent>>) -> Self {
        Self {
            scripts: Mutex::new(scripts),
            requests: Mutex::new(Vec::new()),
            spawned: AtomicUsize::new(0),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub async fn push_script(&self, events: Vec<HelperEvent>) {
        self.scripts.lock().await.push(events);
    }

    pub fn spawn_count(&self) -> usize {
        self.spawned.load(Ordering::SeqCst)
    }

    pub async fn requests(&self) -> Vec<RunRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl SubagentRunner for FakeSubagentRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        let run_id = request.run_id.clone();
        self.requests.lock().await.push(request.clone());
        let mut scripts = self.scripts.lock().await;
        let events = if scripts.is_empty() {
            vec![HelperEvent::Finished {
                run_id: run_id.clone(),
                result: String::new(),
                duration_ms: None,
            }]
        } else {
            scripts.remove(0)
        };
        drop(scripts);
        self.spawned.fetch_add(1, Ordering::SeqCst);
        let hang_after_events =
            events.len() == 1 && matches!(events.first(), Some(HelperEvent::SdkMessage { .. }));
        let events: Vec<HelperEvent> = events
            .into_iter()
            .map(|e| e.with_run_id(run_id.clone()))
            .collect();
        tokio::spawn(async move {
            for ev in events {
                if tx.send(ev).await.is_err() {
                    return;
                }
            }
            if hang_after_events {
                std::future::pending::<()>().await;
            }
        });
        Ok(RunHandle {
            cancel: Box::new(|| {}),
        })
    }

    async fn list_models(&self, _cursor_api_key: &str) -> Result<Vec<CursorModel>, AppError> {
        Ok(vec![CursorModel {
            id: "composer-2.5".to_string(),
            display_name: Some("Composer 2.5".to_string()),
            description: None,
            aliases: None,
            parameters: Some(vec![ModelParamDef {
                id: "fast".to_string(),
                display_name: Some("Fast".to_string()),
                values: vec![
                    ModelParamValueOption {
                        value: "false".to_string(),
                        display_name: None,
                    },
                    ModelParamValueOption {
                        value: "true".to_string(),
                        display_name: Some("Fast".to_string()),
                    },
                ],
            }]),
            variants: Some(vec![
                ModelVariant {
                    params: vec![ModelParamValue {
                        id: "fast".to_string(),
                        value: "true".to_string(),
                    }],
                    display_name: Some("Composer 2.5".to_string()),
                    description: None,
                    is_default: Some(true),
                },
                ModelVariant {
                    params: vec![ModelParamValue {
                        id: "fast".to_string(),
                        value: "false".to_string(),
                    }],
                    display_name: Some("Composer 2.5".to_string()),
                    description: None,
                    is_default: None,
                },
            ]),
        }])
    }
}
