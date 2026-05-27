// SPDX-License-Identifier: Apache-2.0

use eunomio_core::{
    AppError, NewPartitionInsert, NewRunInsert, PartitionStrategy, PhaseName, PhaseState, RunKind,
};
use eunomio_server::repo_store;
use eunomio_sqlite::db;
use pretty_assertions::assert_eq;
use uuid::Uuid;

mod common;
use common::{app::TestApp, db as test_db, fixture::insert_local_fixture};

const EVIL_ORG: &str = "evil";

struct Fixture {
    org_id: String,
    session_id: String,
    base_node_id: String,
    final_node_id: String,
    partition_id: String,
    run_id: String,
}

async fn seed_fixture(state: &eunomio_server::state::AppState) -> Fixture {
    let (org_id, user_id) = insert_local_fixture(state).await;
    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = db::unix_seconds();

    state
        .datastore
        .sessions()
        .insert_seed_nodes(
            org_id.clone(),
            user_id.clone(),
            session_id.clone(),
            "local:/tmp/repo".into(),
            "/tmp/repo".into(),
            "main".into(),
            "feature".into(),
            "e".repeat(40),
            "f".repeat(40),
            "a".repeat(40),
            "b".repeat(40),
            base_node_id.clone(),
            final_node_id.clone(),
            "c".repeat(40),
            "d".repeat(40),
            now,
        )
        .await
        .unwrap();

    let partition_id = state
        .datastore
        .partitions()
        .insert_pending(NewPartitionInsert {
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            session_id: session_id.clone(),
            target_node_id: final_node_id.clone(),
            worktree_path: "/tmp/worktree".into(),
            initial_phase: PhaseName::Plan,
            remaining_depth: None,
            now,
        })
        .await
        .unwrap();

    let run_id = state
        .datastore
        .runs()
        .start(NewRunInsert {
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            partition_id: partition_id.clone(),
            session_id: session_id.clone(),
            target_node_id: final_node_id.clone(),
            kind: RunKind::Plan,
            parent_run_id: None,
            prompt_text: "prompt".into(),
            started_at: now,
        })
        .await
        .unwrap();

    Fixture {
        org_id,
        session_id,
        base_node_id,
        final_node_id,
        partition_id,
        run_id,
    }
}

fn assert_not_found(result: Result<(), AppError>) {
    assert!(matches!(result, Err(AppError::NotFound)));
}

fn assert_not_found_opt<T>(result: Result<Option<T>, AppError>) {
    match result {
        Ok(None) => {}
        Ok(Some(_)) => panic!("expected None for wrong org"),
        Err(AppError::NotFound) => {}
        Err(e) => panic!("unexpected error: {e:?}"),
    }
}

fn assert_not_found_row<T>(result: Result<T, AppError>) {
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[tokio::test]
async fn session_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert_eq!(
        app.state
            .datastore
            .sessions()
            .exists(EVIL_ORG, &fx.session_id)
            .await
            .unwrap(),
        false
    );
    assert_not_found(
        app.state
            .datastore
            .sessions()
            .ensure(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_opt(
        app.state
            .datastore
            .sessions()
            .get(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_row(
        app.state
            .datastore
            .sessions()
            .user_id(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_row(
        app.state
            .datastore
            .sessions()
            .repo_fields(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_row(repo_store::session_git_root(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_opt(
        app.state
            .datastore
            .sessions()
            .final_tree(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_opt(
        app.state
            .datastore
            .sessions()
            .base_tree(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found_row(
        app.state
            .datastore
            .sessions()
            .seed_trees(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .sessions()
            .delete_cascade(EVIL_ORG, &fx.session_id)
            .await,
    );
    assert!(app
        .state
        .datastore
        .sessions()
        .exists(&fx.org_id, &fx.session_id)
        .await
        .unwrap());
    assert!(app
        .state
        .datastore
        .sessions()
        .list(EVIL_ORG)
        .await
        .unwrap()
        .is_empty());
    assert!(app
        .state
        .datastore
        .sessions()
        .list_partition_worktrees(EVIL_ORG, &fx.session_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn partition_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert_not_found_row(
        app.state
            .datastore
            .partitions()
            .get(EVIL_ORG, &fx.partition_id)
            .await,
    );
    assert!(app
        .state
        .datastore
        .partitions()
        .list(EVIL_ORG, &fx.session_id, None)
        .await
        .unwrap()
        .is_empty());
    assert!(app
        .state
        .datastore
        .partitions()
        .list_siblings(
            EVIL_ORG,
            &fx.session_id,
            &fx.final_node_id,
            &fx.partition_id,
        )
        .await
        .unwrap()
        .is_empty());
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .delete(EVIL_ORG, &fx.partition_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .delete_with_runs(EVIL_ORG, &fx.partition_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .delete_many_with_runs(EVIL_ORG, vec![fx.partition_id.clone()])
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .clear_plan_and_slice(EVIL_ORG, &fx.partition_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .accept_plan(
                EVIL_ORG,
                &fx.partition_id,
                "{}".into(),
                PartitionStrategy::Synthetic,
            )
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .set_phase_state(EVIL_ORG, &fx.partition_id, PhaseState::Running)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .set_phase_running(EVIL_ORG, &fx.partition_id, PhaseName::Plan)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .set_worktree_path(EVIL_ORG, &fx.partition_id, "/other".into())
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .accept_constructor_ok(
                EVIL_ORG,
                &fx.partition_id,
                "tree".into(),
                "commit".into(),
                &fx.run_id,
                "{}".into(),
                "ok".into(),
            )
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .accept_constructor_blocked(
                EVIL_ORG,
                &fx.partition_id,
                &fx.run_id,
                "{}".into(),
                "blocked".into(),
            )
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .fail_run(EVIL_ORG, &fx.partition_id, &fx.run_id, "err".into(), None)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .cancel_run(EVIL_ORG, &fx.partition_id, &fx.run_id)
            .await,
    );
    assert!(app
        .state
        .datastore
        .partitions()
        .get(&fx.org_id, &fx.partition_id)
        .await
        .is_ok());
}

#[tokio::test]
async fn run_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert_not_found_row(app.state.datastore.runs().get(EVIL_ORG, &fx.run_id).await);
    assert!(app
        .state
        .datastore
        .runs()
        .list_for_partition(EVIL_ORG, &fx.partition_id)
        .await
        .unwrap()
        .is_empty());
    assert_not_found_opt(
        app.state
            .datastore
            .runs()
            .get_prompt(EVIL_ORG, &fx.run_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .runs()
            .append_transcript_text(EVIL_ORG, &fx.run_id, "chunk")
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .runs()
            .finish_success(EVIL_ORG, &fx.run_id, "{}".into(), None)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .runs()
            .finish_error(EVIL_ORG, &fx.run_id, "err".into())
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .runs()
            .cancel(EVIL_ORG, &fx.run_id)
            .await,
    );
    assert_not_found(
        app.state
            .datastore
            .runs()
            .cancel_running_for_partition(EVIL_ORG, &fx.partition_id)
            .await,
    );
    assert!(app
        .state
        .datastore
        .runs()
        .get(&fx.org_id, &fx.run_id)
        .await
        .is_ok());
}

#[tokio::test]
async fn node_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert!(app
        .state
        .datastore
        .nodes()
        .list_for_session(EVIL_ORG, &fx.session_id)
        .await
        .unwrap()
        .is_empty());
    assert_not_found_opt(
        app.state
            .datastore
            .nodes()
            .get(EVIL_ORG, &fx.session_id, &fx.final_node_id)
            .await,
    );
    assert_not_found_row(
        app.state
            .datastore
            .nodes()
            .target_and_parent(EVIL_ORG, &fx.session_id, &fx.final_node_id)
            .await,
    );
    assert_not_found_opt(
        app.state
            .datastore
            .nodes()
            .target_tree_and_parent(EVIL_ORG, &fx.session_id, &fx.final_node_id)
            .await,
    );
    assert_not_found_row(
        app.state
            .datastore
            .nodes()
            .session_for_node_id(EVIL_ORG, &fx.final_node_id)
            .await,
    );
}

#[tokio::test]
async fn finalize_construct_accept_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;
    let now = db::unix_seconds();
    assert_not_found(
        app.state
            .datastore
            .partitions()
            .finalize_construct_accept(
                EVIL_ORG.to_string(),
                fx.session_id.clone(),
                fx.partition_id.clone(),
                fx.final_node_id.clone(),
                Uuid::new_v4().to_string(),
                fx.base_node_id.clone(),
                "tree".into(),
                "commit".into(),
                "slice".into(),
                "desc".into(),
                None,
                "leftover".into(),
                "desc".into(),
                vec![],
                now,
            )
            .await,
    );
}

#[tokio::test]
async fn distinct_trees_in_session_wrong_org_returns_false() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;
    let known = app
        .state
        .datastore
        .nodes()
        .distinct_trees_in_session(EVIL_ORG, &fx.session_id, &["b".repeat(40).as_str()])
        .await
        .unwrap();
    assert_eq!(known, false);
    let known = app
        .state
        .datastore
        .nodes()
        .distinct_trees_in_session(&fx.org_id, &fx.session_id, &["b".repeat(40).as_str()])
        .await
        .unwrap();
    assert_eq!(known, true);
}

#[tokio::test]
async fn insert_carries_org_and_user_ids() {
    let app = TestApp::spawn().await;
    let (org_id, user_id) = insert_local_fixture(&app.state).await;
    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = db::unix_seconds();

    app.state
        .datastore
        .sessions()
        .insert_seed_nodes(
            org_id.clone(),
            user_id.clone(),
            session_id.clone(),
            "local:/tmp/repo".into(),
            "/tmp/repo".into(),
            "main".into(),
            "feature".into(),
            "e".repeat(40),
            "f".repeat(40),
            "a".repeat(40),
            "b".repeat(40),
            base_node_id,
            final_node_id.clone(),
            "c".repeat(40),
            "d".repeat(40),
            now,
        )
        .await
        .unwrap();

    let row = test_db::query_two_strings(
        &app.state,
        "SELECT org_id, user_id FROM sessions WHERE id = ?1",
        &session_id,
    )
    .await;
    assert_eq!(row.0, org_id);
    assert_eq!(row.1, user_id);

    let partition_id = app
        .state
        .datastore
        .partitions()
        .insert_pending(NewPartitionInsert {
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            session_id: session_id.clone(),
            target_node_id: final_node_id.clone(),
            worktree_path: "/tmp/wt".into(),
            initial_phase: PhaseName::Plan,
            remaining_depth: Some(2),
            now,
        })
        .await
        .unwrap();

    let part_row = test_db::query_two_strings(
        &app.state,
        "SELECT org_id, user_id FROM partitions WHERE id = ?1",
        &partition_id,
    )
    .await;
    assert_eq!(part_row.0, org_id);
    assert_eq!(part_row.1, user_id);

    let run_id = app
        .state
        .datastore
        .runs()
        .start(NewRunInsert {
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            partition_id,
            session_id,
            target_node_id: final_node_id,
            kind: RunKind::Plan,
            parent_run_id: None,
            prompt_text: "p".into(),
            started_at: now,
        })
        .await
        .unwrap();

    let run_row = test_db::query_two_strings(
        &app.state,
        "SELECT org_id, user_id FROM runs WHERE id = ?1",
        &run_id,
    )
    .await;
    assert_eq!(run_row.0, org_id);
    assert_eq!(run_row.1, user_id);
    assert_eq!(run_id.len(), 36);
}

#[tokio::test]
async fn partition_and_run_ids_are_uuid_strings() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;
    assert_eq!(fx.partition_id.len(), 36);
    assert_eq!(fx.run_id.len(), 36);
    assert!(Uuid::parse_str(&fx.partition_id).is_ok());
    assert!(Uuid::parse_str(&fx.run_id).is_ok());
}
