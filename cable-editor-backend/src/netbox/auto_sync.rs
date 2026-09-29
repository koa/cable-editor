//! Syncs the circuits of the plan active in Netbox automatically (docs/netbox-sync.md): triggers
//! record every change that affects them in `netbox_sync_anstoss`, in the transaction making
//! it; this worker runs the sync when there are such rows or the last run is older than the
//! configured interval, and stores the result: the issues, which keep Netbox as it is, or the
//! error, which is tried again later.
//!
//! A run is one transaction: it reads the rows recording changes, syncs, stores the result and
//! deletes exactly the rows it read, so a change committed during the run stays pending. A run
//! that fails is rolled back (its changes stay pending) and the failure stored on its own.

use crate::{
    config::NETBOX_CONFIG,
    db::{DB, entity::plan::BASELINE_PLAN_ID, schema},
    graphql::{
        authenticated::mutation::sync::sync_plan_to_netbox,
        error::{ApiError, ApiResult},
    },
};
use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, dsl::sql, select, sql_types::Bool};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use log::{info, warn};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Notify, time::sleep};

/// Key of the advisory lock that lets one replica sync at a time
const LOCK_KEY: i64 = 0x4e45_5442_4f58; // "NETBOX"

/// Checks at least this often whether a sync is due (the interval, a retry, a change the worker
/// missed)
const CHECK_EVERY: Duration = Duration::from_secs(60);

/// After failed runs in a row: wait before trying again, then the last one for every further try
/// (changes don't make Netbox reachable; `syncNetbox` tries right away)
const RETRY_AFTER: [Duration; 4] = [
    Duration::from_secs(60),
    Duration::from_secs(2 * 60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(10 * 60),
];

/// What a run did
#[derive(Debug, PartialEq)]
enum Outcome {
    /// Nothing to do, or another replica is syncing
    Idle,
    Synced,
    Issues(usize),
    /// Netbox or the database failed; the changes stay pending
    Failed,
}

/// Runs the worker until the process ends; `changed` wakes it after a committed request.
pub async fn run(pool: DB, changed: Arc<Notify>) {
    loop {
        match run_once(&pool).await {
            Ok(Outcome::Idle) => {}
            Ok(outcome) => info!("Netbox sync: {outcome:?}"),
            Err(error) => warn!("Netbox sync: failure not stored: {error:?}"),
        }
        tokio::select! {
            () = changed.notified() => {}
            () = sleep(CHECK_EVERY) => {}
        }
    }
}

async fn run_once(pool: &DB) -> ApiResult<Outcome> {
    let mut connection = pool.get().await?;
    let result = connection
        .transaction::<_, ApiError, _>(async |conn| sync_if_due(conn).await)
        .await;
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            // The run's transaction may be aborted (a database error), so a new one
            store_failure(&mut connection, error).await?;
            Ok(Outcome::Failed)
        }
    }
}

/// The run, in the caller's transaction.
async fn sync_if_due(conn: &mut AsyncPgConnection) -> ApiResult<Outcome> {
    // Released with the transaction
    let locked: bool = select(sql::<Bool>(&format!(
        "pg_try_advisory_xact_lock({LOCK_KEY})"
    )))
    .get_result(conn)
    .await?;
    if !locked {
        return Ok(Outcome::Idle);
    }
    let (last_run, retry_at): (Option<DateTime<Utc>>, Option<DateTime<Utc>>) =
        schema::netbox_sync::table
            .select((
                schema::netbox_sync::letzter_lauf,
                schema::netbox_sync::naechster_versuch,
            ))
            .first(conn)
            .await?;
    if retry_at.is_some_and(|retry_at| retry_at > Utc::now()) {
        return Ok(Outcome::Idle);
    }
    let changes: Box<[i64]> = schema::netbox_sync_anstoss::table
        .select(schema::netbox_sync_anstoss::id)
        .load::<i64>(conn)
        .await?
        .into_boxed_slice();
    // A fixed interval since the last run, independent of the time of day
    let interval_over = last_run.is_none_or(|last_run| {
        (Utc::now() - last_run)
            .to_std()
            .is_ok_and(|age| age >= NETBOX_CONFIG.sync_interval())
    });
    if changes.is_empty() && !interval_over {
        return Ok(Outcome::Idle);
    }
    let plan_id = schema::plan::table
        .filter(schema::plan::netbox_active)
        .select(schema::plan::id)
        .first::<i32>(conn)
        .await
        .optional()?
        .unwrap_or(BASELINE_PLAN_ID);
    let issues = sync_plan_to_netbox(plan_id, conn).await?;

    diesel::delete(schema::netbox_sync_issue::table)
        .execute(conn)
        .await?;
    let rows = issues
        .iter()
        .map(|issue| {
            serde_json::to_value(issue).map(|daten| schema::netbox_sync_issue::daten.eq(daten))
        })
        // Diesel inserts the rows of a Vec, not of a boxed slice
        .collect::<Result<Vec<_>, _>>()?;
    diesel::insert_into(schema::netbox_sync_issue::table)
        .values(rows)
        .execute(conn)
        .await?;
    let result = if issues.is_empty() { "ok" } else { "issues" };
    diesel::update(schema::netbox_sync::table)
        .set((
            schema::netbox_sync::letzter_lauf.eq(Utc::now()),
            schema::netbox_sync::ergebnis.eq(result),
            schema::netbox_sync::fehler.eq(None::<serde_json::Value>),
            schema::netbox_sync::fehlversuche.eq(0),
            schema::netbox_sync::naechster_versuch.eq(None::<DateTime<Utc>>),
        ))
        .execute(conn)
        .await?;
    // Only the changes the run saw: those committed during it stay pending
    diesel::delete(schema::netbox_sync_anstoss::table)
        .filter(schema::netbox_sync_anstoss::id.eq_any(changes))
        .execute(conn)
        .await?;
    Ok(if issues.is_empty() {
        Outcome::Synced
    } else {
        Outcome::Issues(issues.len())
    })
}

/// Stores the error of a failed run, which was rolled back, and when to try again.
async fn store_failure(connection: &mut AsyncPgConnection, error: ApiError) -> ApiResult<()> {
    // Logged here, with the id the page shows
    let error = async_graphql::Error::from(error);
    let error = serde_json::json!({
        "message": error.message,
        "extensions": error.extensions,
    });
    connection
        .transaction::<_, ApiError, _>(async move |conn| {
            // Which issues the plan has isn't known after a failure
            diesel::delete(schema::netbox_sync_issue::table)
                .execute(conn)
                .await?;
            let failures: i32 = schema::netbox_sync::table
                .select(schema::netbox_sync::fehlversuche)
                .for_update()
                .first(conn)
                .await?;
            let wait = RETRY_AFTER[(failures as usize).min(RETRY_AFTER.len() - 1)];
            diesel::update(schema::netbox_sync::table)
                .set((
                    schema::netbox_sync::letzter_lauf.eq(Utc::now()),
                    schema::netbox_sync::ergebnis.eq("fehler"),
                    schema::netbox_sync::fehler.eq(error),
                    schema::netbox_sync::fehlversuche.eq(failures + 1),
                    schema::netbox_sync::naechster_versuch.eq(Utc::now() + wait),
                ))
                .execute(conn)
                .await?;
            Ok(())
        })
        .await
}
