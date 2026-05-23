// SPDX-License-Identifier: Apache-2.0

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CurrentPrincipal {
    pub user_id: String,
    pub org_id: String,
    pub role: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalResponse {
    pub user_id: String,
    pub org_id: String,
    pub role: String,
    pub username: String,
}

impl From<CurrentPrincipal> for PrincipalResponse {
    fn from(p: CurrentPrincipal) -> Self {
        Self {
            user_id: p.user_id,
            org_id: p.org_id,
            role: p.role,
            username: p.username,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthSessionRow {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub expires_at: i64,
}
