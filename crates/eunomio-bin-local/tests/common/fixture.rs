// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use eunomio_auth_local::LOCAL_ORG_ID;
use eunomio_server::state::AppState;

pub async fn insert_local_fixture(state: &AppState) -> (String, String) {
    state
        .datastore
        .orgs()
        .ensure_singleton_local()
        .await
        .expect("ensure org");
    let user_row = state
        .datastore
        .users()
        .insert("fixture-user")
        .await
        .expect("insert user");
    state
        .datastore
        .users()
        .ensure_membership(LOCAL_ORG_ID, &user_row.id, "Owner")
        .await
        .expect("membership");
    (LOCAL_ORG_ID.to_string(), user_row.id)
}
