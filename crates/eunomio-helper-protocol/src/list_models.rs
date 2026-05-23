// SPDX-License-Identifier: Apache-2.0

use eunomio_core::types::CursorModel;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ListModelsRequest {
    #[serde(rename = "cursorApiKey")]
    pub cursor_api_key: String,
}

#[derive(Deserialize)]
pub struct ListModelsResponse {
    pub models: Vec<CursorModel>,
}
