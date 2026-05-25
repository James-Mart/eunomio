// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use eunomio_server::state::AppState;

pub async fn open_db(state: &AppState) -> tokio_rusqlite::Connection {
    eunomio_sqlite::db::open(&state.data_dir.join("eunomio.db"))
        .await
        .expect("open test db")
}

pub async fn query_i64(state: &AppState, sql: &str, param: &str) -> i64 {
    let conn = open_db(state).await;
    conn.call({
        let param = param.to_string();
        let sql = sql.to_string();
        move |c| {
            let n: i64 = c.query_row(&sql, tokio_rusqlite::params![param], |r| r.get(0))?;
            Ok(n)
        }
    })
    .await
    .unwrap()
}

pub async fn query_i64_no_params(state: &AppState, sql: &str) -> i64 {
    let conn = open_db(state).await;
    conn.call({
        let sql = sql.to_string();
        move |c| {
            let n: i64 = c.query_row(&sql, [], |r| r.get(0))?;
            Ok(n)
        }
    })
    .await
    .unwrap()
}

pub async fn query_two_strings(state: &AppState, sql: &str, id_param: &str) -> (String, String) {
    let conn = open_db(state).await;
    conn.call({
        let id_param = id_param.to_string();
        let sql = sql.to_string();
        move |c| {
            let mut stmt = c.prepare(&sql)?;
            let mut rows = stmt.query(tokio_rusqlite::params![id_param])?;
            let row = rows.next()?.unwrap();
            Ok((row.get(0)?, row.get(1)?))
        }
    })
    .await
    .unwrap()
}

pub async fn exec_sql(state: &AppState, sql: &str, param: i64, id: &str) {
    let conn = open_db(state).await;
    conn.call({
        let id = id.to_string();
        let sql = sql.to_string();
        move |c| {
            c.execute(&sql, tokio_rusqlite::params![param, id])?;
            Ok(())
        }
    })
    .await
    .unwrap();
}
