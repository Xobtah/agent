use std::sync::{Arc, Mutex};

use common::model::Mission;
use rusqlite::OptionalExtension;
use tracing::debug;

use crate::{error::C2Result, ThreadSafeConnection};

use super::RusqliteResult;

fn row_to_mission(row: &rusqlite::Row) -> Result<Mission, rusqlite::Error> {
    Ok(Mission {
        id: row.get("id")?,
        agent_id: row.get("agent_id")?,
        task: serde_json::from_str(&row.get::<_, String>("task")?).unwrap(),
        result: row.get("result")?,
        issued_at: row.get("issued_at")?,
        completed_at: row.get("completed_at")?,
    })
}

pub fn create(
    conn: ThreadSafeConnection,
    agent_id: i32,
    task: common::model::Task,
) -> C2Result<Mission> {
    debug!("Creating mission for agent {}", agent_id);
    let conn = conn.lock().unwrap();

    conn.execute(
        "INSERT INTO missions (agent_id, task) VALUES (?1, ?2)",
        rusqlite::params![agent_id, serde_json::to_string(&task)?],
    )?;

    Ok(conn.query_row(
            "SELECT id, agent_id, task, result, issued_at, completed_at FROM missions WHERE id = last_insert_rowid()",
            [],
            row_to_mission,
        )?)
}

pub fn get_next(
    conn: Arc<Mutex<rusqlite::Connection>>,
    agent_id: i32,
) -> RusqliteResult<Option<Mission>> {
    debug!("Getting next mission for agent {}", agent_id);
    conn.lock().unwrap().query_row(
            "SELECT id, agent_id, task, result, issued_at, completed_at FROM missions WHERE agent_id = ?1 AND completed_at IS NULL ORDER BY issued_at ASC LIMIT 1",
            [agent_id],
            row_to_mission,
        )
        .optional()
}

pub async fn poll_next(
    conn: Arc<Mutex<rusqlite::Connection>>,
    agent_id: i32,
) -> RusqliteResult<Option<Mission>> {
    debug!("Polling next mission for agent {}", agent_id);
    for _ in 0..5 {
        if let Some(mission) = get_next(conn.clone(), agent_id)? {
            return Ok(Some(mission));
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    Ok(None)
}

pub fn get_by_id(conn: ThreadSafeConnection, id: i32) -> RusqliteResult<Option<Mission>> {
    debug!("Getting mission {}", id);
    let conn = conn.lock().unwrap();
    conn.query_row(
            "SELECT id, agent_id, task, result, issued_at, completed_at FROM missions WHERE id = ?1 LIMIT 1",
            [id],
            row_to_mission,
        ).optional()
}

pub fn complete(conn: ThreadSafeConnection, id: i32, result: &str) -> RusqliteResult<Mission> {
    debug!("Completing mission {}", id);
    let conn = conn.lock().unwrap();

    conn.execute(
        "UPDATE missions SET result = ?1, completed_at = CURRENT_TIMESTAMP WHERE id = ?2",
        rusqlite::params![result, id],
    )?;

    conn.query_row(
        "SELECT id, agent_id, task, result, issued_at, completed_at FROM missions WHERE id = ?1",
        [id],
        row_to_mission,
    )
}
