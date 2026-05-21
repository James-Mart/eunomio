use super::runner::{HelperEvent, RunHandle, RunRequest, SubagentRunner};
use crate::error::AppError;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, Mutex};

pub struct FakeSubagentRunner {
    scripts: Mutex<Vec<Vec<HelperEvent>>>,
    spawned: AtomicUsize,
}

impl FakeSubagentRunner {
    pub fn new(scripts: Vec<Vec<HelperEvent>>) -> Self {
        Self {
            scripts: Mutex::new(scripts),
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
}

#[async_trait::async_trait]
impl SubagentRunner for FakeSubagentRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        let mut scripts = self.scripts.lock().await;
        let events = if scripts.is_empty() {
            vec![HelperEvent::Finished {
                run_id: request.run_id,
                result: String::new(),
                duration_ms: None,
            }]
        } else {
            scripts.remove(0)
        };
        drop(scripts);
        self.spawned.fetch_add(1, Ordering::SeqCst);
        let run_id = request.run_id;
        let events: Vec<HelperEvent> = events
            .into_iter()
            .map(|e| e.with_run_id(run_id))
            .collect();
        tokio::spawn(async move {
            for ev in events {
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
        });
        Ok(RunHandle {
            cancel: Box::new(|| {}),
        })
    }
}
