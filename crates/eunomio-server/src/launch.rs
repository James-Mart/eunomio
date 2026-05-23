// SPDX-License-Identifier: Apache-2.0

use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct LaunchIntent {
    pull_request: Arc<Mutex<Option<String>>>,
}

impl LaunchIntent {
    pub fn new(pull_request: Option<String>) -> Self {
        Self {
            pull_request: Arc::new(Mutex::new(pull_request)),
        }
    }

    pub fn take_pull_request(&self) -> Option<String> {
        self.pull_request.lock().unwrap().take()
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchPullRequestResponse {
    pull_request_url: Option<String>,
}

pub fn public_launch_routes() -> Router<AppState> {
    Router::new().route("/api/launch/pull-request", get(get_launch_pull_request))
}

async fn get_launch_pull_request(
    State(state): State<AppState>,
) -> Json<LaunchPullRequestResponse> {
    Json(LaunchPullRequestResponse {
        pull_request_url: state.launch.take_pull_request(),
    })
}
