use eunomio::db;
use eunomio::error::AppError;
use eunomio::repo::{node, partition, run, session, tree};
use eunomio::types::{PartitionStrategy, PhaseName, PhaseState, RunKind};
use pretty_assertions::assert_eq;
use uuid::Uuid;

mod common;
use common::{insert_local_fixture, TestApp};

const EVIL_ORG: &str = "evil";

struct Fixture {
    org_id: String,
    user_id: String,
    session_id: String,
    base_node_id: String,
    final_node_id: String,
    partition_id: String,
    run_id: String,
}

async fn seed_fixture(state: &eunomio::state::AppState) -> Fixture {
    let (org_id, user_id) = insert_local_fixture(state).await;
    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = db::unix_seconds();

    session::insert_seed_nodes(
        state,
        org_id.clone(),
        user_id.clone(),
        session_id.clone(),
        "/tmp/repo".into(),
        "/tmp/repo".into(),
        true,
        "main".into(),
        "feature".into(),
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

    let partition_id = partition::insert_pending(
        state,
        org_id.clone(),
        user_id.clone(),
        session_id.clone(),
        final_node_id.clone(),
        "/tmp/worktree".into(),
        None,
        now,
    )
    .await
    .unwrap();

    let run_id = run::start(
        state,
        org_id.clone(),
        user_id.clone(),
        partition_id.clone(),
        session_id.clone(),
        final_node_id.clone(),
        RunKind::Survey,
        None,
        "prompt".into(),
        now,
    )
    .await
    .unwrap();

    Fixture {
        org_id,
        user_id,
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
        session::exists(&app.state, EVIL_ORG, &fx.session_id)
            .await
            .unwrap(),
        false
    );
    assert_not_found(session::ensure(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_opt(session::get(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_row(session::user_id(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_row(
        session::repo_fields(&app.state, EVIL_ORG, &fx.session_id).await,
    );
    assert_not_found_row(session::git_root(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_opt(session::final_tree(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_opt(session::base_tree(&app.state, EVIL_ORG, &fx.session_id).await);
    assert_not_found_opt(
        session::seed_trees(&app.state, EVIL_ORG, &fx.session_id).await,
    );
    assert_not_found(session::delete_cascade(&app.state, EVIL_ORG, &fx.session_id).await);
    assert!(
        session::exists(&app.state, &fx.org_id, &fx.session_id)
            .await
            .unwrap()
    );
    assert!(session::list(&app.state, EVIL_ORG).await.unwrap().is_empty());
    assert!(
        session::list_partition_worktrees(&app.state, EVIL_ORG, &fx.session_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn partition_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert_not_found_row(partition::get(&app.state, EVIL_ORG, &fx.partition_id).await);
    assert!(
        partition::list(&app.state, EVIL_ORG, &fx.session_id, None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        partition::list_siblings(
            &app.state,
            EVIL_ORG,
            &fx.session_id,
            &fx.final_node_id,
            &fx.partition_id,
        )
        .await
        .unwrap()
            .is_empty()
    );
    assert_not_found(partition::delete(&app.state, EVIL_ORG, &fx.partition_id).await);
    assert_not_found(
        partition::delete_with_runs(&app.state, EVIL_ORG, &fx.partition_id).await,
    );
    assert_not_found(
        partition::delete_many_with_runs(
            &app.state,
            EVIL_ORG,
            vec![fx.partition_id.clone()],
        )
        .await,
    );
    assert_not_found(
        partition::clear_plan_and_slice(&app.state, EVIL_ORG, &fx.partition_id).await,
    );
    assert_not_found(
        partition::accept_survey(&app.state, EVIL_ORG, &fx.partition_id, "{}".into()).await,
    );
    assert_not_found(
        partition::accept_plan(
            &app.state,
            EVIL_ORG,
            &fx.partition_id,
            "{}".into(),
            PartitionStrategy::Synthetic,
        )
        .await,
    );
    assert_not_found(
        partition::set_phase_state(
            &app.state,
            EVIL_ORG,
            &fx.partition_id,
            PhaseState::Running,
        )
        .await,
    );
    assert_not_found(
        partition::set_phase_running(&app.state, EVIL_ORG, &fx.partition_id, PhaseName::Survey)
            .await,
    );
    assert_not_found(
        partition::set_worktree_path(
            &app.state,
            EVIL_ORG,
            &fx.partition_id,
            "/other".into(),
        )
        .await,
    );
    assert_not_found(
        partition::accept_constructor_ok(
            &app.state,
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
        partition::accept_constructor_blocked(
            &app.state,
            EVIL_ORG,
            &fx.partition_id,
            &fx.run_id,
            "{}".into(),
            "blocked".into(),
        )
        .await,
    );
    assert_not_found(
        partition::fail_run(
            &app.state,
            EVIL_ORG,
            &fx.partition_id,
            &fx.run_id,
            "err".into(),
            None,
        )
        .await,
    );
    assert_not_found(
        partition::cancel_run(&app.state, EVIL_ORG, &fx.partition_id, &fx.run_id).await,
    );
    assert!(
        partition::get(&app.state, &fx.org_id, &fx.partition_id)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn run_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert_not_found_row(run::get(&app.state, EVIL_ORG, &fx.run_id).await);
    assert!(
        run::list_for_partition(&app.state, EVIL_ORG, &fx.partition_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_not_found_opt(run::get_prompt(&app.state, EVIL_ORG, &fx.run_id).await);
    assert_not_found(
        run::append_transcript_text(&app.state, EVIL_ORG, &fx.run_id, "chunk").await,
    );
    assert_not_found(
        run::finish_success(&app.state, EVIL_ORG, &fx.run_id, "{}".into(), None).await,
    );
    assert_not_found(run::finish_error(&app.state, EVIL_ORG, &fx.run_id, "err".into()).await);
    assert_not_found(run::cancel(&app.state, EVIL_ORG, &fx.run_id).await);
    assert_not_found(
        run::cancel_running_for_partition(&app.state, EVIL_ORG, &fx.partition_id).await,
    );
    assert!(
        run::get(&app.state, &fx.org_id, &fx.run_id)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn node_repo_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;

    assert!(
        node::list_for_session(&app.state, EVIL_ORG, &fx.session_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_not_found_opt(
        node::get(&app.state, EVIL_ORG, &fx.session_id, &fx.final_node_id).await,
    );
    assert_not_found_row(
        node::target_and_parent(&app.state, EVIL_ORG, &fx.session_id, &fx.final_node_id).await,
    );
    assert_not_found_opt(
        node::target_tree_and_parent(&app.state, EVIL_ORG, &fx.session_id, &fx.final_node_id)
            .await,
    );
    assert_not_found_row(
        node::update_title(
            &app.state,
            EVIL_ORG,
            &fx.session_id,
            &fx.final_node_id,
            "x",
        )
        .await,
    );
    assert!(
        node::walk_to_base(&app.state, EVIL_ORG, &fx.session_id, &fx.final_node_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_not_found_row(
        node::session_for_node_id(&app.state, EVIL_ORG, &fx.final_node_id).await,
    );
}

#[tokio::test]
async fn finalize_construct_accept_wrong_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;
    let now = db::unix_seconds();
    assert_not_found(
        partition::finalize_construct_accept(
            &app.state,
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
async fn tree_known_in_session_wrong_org_returns_false() {
    let app = TestApp::spawn().await;
    let fx = seed_fixture(&app.state).await;
    let known = tree::trees_known_in_session(
        &app.state,
        EVIL_ORG,
        &fx.session_id,
        &["b".repeat(40).as_str()],
    )
    .await
    .unwrap();
    assert_eq!(known, false);
    let known = tree::trees_known_in_session(
        &app.state,
        &fx.org_id,
        &fx.session_id,
        &["b".repeat(40).as_str()],
    )
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

    session::insert_seed_nodes(
        &app.state,
        org_id.clone(),
        user_id.clone(),
        session_id.clone(),
        "/tmp/repo".into(),
        "/tmp/repo".into(),
        true,
        "main".into(),
        "feature".into(),
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

    let row: (String, String) = app
        .state
        .db
        .call({
            let session_id = session_id.clone();
            move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT org_id, user_id FROM sessions WHERE id = ?1",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
                let row = rows.next()?.unwrap();
                Ok((row.get(0)?, row.get(1)?))
            }
        })
        .await
        .unwrap();
    assert_eq!(row.0, org_id);
    assert_eq!(row.1, user_id);

    let partition_id = partition::insert_pending(
        &app.state,
        org_id.clone(),
        user_id.clone(),
        session_id.clone(),
        final_node_id.clone(),
        "/tmp/wt".into(),
        Some(2),
        now,
    )
    .await
    .unwrap();

    let part_row: (String, String) = app
        .state
        .db
        .call({
            let partition_id = partition_id.clone();
            move |conn| {
                let mut stmt =
                    conn.prepare("SELECT org_id, user_id FROM partitions WHERE id = ?1")?;
                let mut rows = stmt.query(tokio_rusqlite::params![partition_id])?;
                let row = rows.next()?.unwrap();
                Ok((row.get(0)?, row.get(1)?))
            }
        })
        .await
        .unwrap();
    assert_eq!(part_row.0, org_id);
    assert_eq!(part_row.1, user_id);

    let run_id = run::start(
        &app.state,
        org_id.clone(),
        user_id.clone(),
        partition_id,
        session_id,
        final_node_id,
        RunKind::Plan,
        None,
        "p".into(),
        now,
    )
    .await
    .unwrap();

    let run_row: (String, String) = app
        .state
        .db
        .call({
            let run_id = run_id.clone();
            move |conn| {
                let mut stmt = conn.prepare("SELECT org_id, user_id FROM runs WHERE id = ?1")?;
                let mut rows = stmt.query(tokio_rusqlite::params![run_id])?;
                let row = rows.next()?.unwrap();
                Ok((row.get(0)?, row.get(1)?))
            }
        })
        .await
        .unwrap();
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
