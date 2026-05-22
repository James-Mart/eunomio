#![allow(dead_code)]

use eunomio::repo::{org, user};
use eunomio::state::AppState;

pub async fn insert_local_fixture(state: &AppState) -> (String, String) {
    org::ensure_singleton_local(state).await.expect("ensure org");
    let user_row = user::insert(state, "fixture-user")
        .await
        .expect("insert user");
    user::ensure_membership(state, org::LOCAL_ORG_ID, &user_row.id, "Owner")
        .await
        .expect("membership");
    (org::LOCAL_ORG_ID.to_string(), user_row.id)
}
