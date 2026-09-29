//! Where the automatic sync to Netbox stands (`netbox::auto_sync`, docs/netbox-sync.md): for
//! everyone whether Netbox is in sync, for admins the issues and the error of the last run.

use crate::{
    db::{entity::plan::Plan, schema},
    graphql::{
        authenticated::{get_connection, mutation::sync::SyncIssue},
        authorization::{Role, RoleGuard},
        error::ApiResult,
    },
};
use async_graphql::{Context, Enum, Object, SimpleObject};
use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, dsl::exists, select};
use diesel_async::RunQueryDsl;
use log::warn;
use std::collections::HashSet;

#[derive(Enum, Copy, Clone, PartialEq, Eq, Debug)]
pub enum NetboxSyncState {
    /// Netbox shows the active plan
    Synchron,
    /// A run is due (the normal case right after a change)
    Ausstehend,
    /// The last run found issues and left Netbox as it was
    NichtSynchron,
    /// The last run failed (e.g. Netbox unreachable), it is tried again
    Fehler,
}

/// The last run of the sync (`netbox_sync`).
pub struct NetboxSync {
    pending: bool,
    last_run: Option<DateTime<Utc>>,
    retry_at: Option<DateTime<Utc>>,
    result: Option<String>,
    error: Option<serde_json::Value>,
}

impl NetboxSync {
    pub async fn load(ctx: &Context<'_>) -> ApiResult<Self> {
        let mut connection = get_connection(ctx).await?;
        let (last_run, retry_at, result, error) = schema::netbox_sync::table
            .select((
                schema::netbox_sync::letzter_lauf,
                schema::netbox_sync::naechster_versuch,
                schema::netbox_sync::ergebnis,
                schema::netbox_sync::fehler,
            ))
            .first::<(
                Option<DateTime<Utc>>,
                Option<DateTime<Utc>>,
                Option<String>,
                Option<serde_json::Value>,
            )>(&mut connection)
            .await?;
        let pending = select(exists(
            schema::netbox_sync_anstoss::table.select(schema::netbox_sync_anstoss::id),
        ))
        .get_result(&mut connection)
        .await?;
        Ok(NetboxSync {
            pending,
            last_run,
            retry_at,
            result,
            error,
        })
    }
}

/// Why the last run failed, the GraphQL error it ended with.
#[derive(SimpleObject)]
pub struct NetboxSyncError {
    pub message: String,
    /// The error's extensions as JSON: `userError` (e.g. Netbox refused a step) or `origin`
    pub extensions: Option<String>,
}

#[Object]
impl NetboxSync {
    async fn state(&self) -> NetboxSyncState {
        match self.result.as_deref() {
            Some("fehler") => NetboxSyncState::Fehler,
            Some("issues") => NetboxSyncState::NichtSynchron,
            Some("ok") if !self.pending => NetboxSyncState::Synchron,
            _ => NetboxSyncState::Ausstehend,
        }
    }
    /// Something changed that the last run didn't sync (or it failed): a run is due
    async fn pending(&self) -> bool {
        self.pending
    }
    /// After a failed run: when it is tried again at the earliest
    async fn retry_at(&self) -> Option<DateTime<Utc>> {
        self.retry_at
    }
    /// The end of the last run
    async fn last_run(&self) -> Option<DateTime<Utc>> {
        self.last_run
    }
    /// The plan whose circuits Netbox shows
    async fn active_plan(&self, ctx: &Context<'_>) -> ApiResult<Option<Plan>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Plan::query()
            .filter(schema::plan::netbox_active)
            .first(&mut connection)
            .await
            .optional()?)
    }
    /// The issues of the last run; ones whose ports were deleted since are left out (the
    /// deletion started another run)
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn issues(&self, ctx: &Context<'_>) -> ApiResult<Box<[SyncIssue]>> {
        let mut connection = get_connection(ctx).await?;
        let stored = schema::netbox_sync_issue::table
            .order(schema::netbox_sync_issue::id)
            .select(schema::netbox_sync_issue::daten)
            .load::<serde_json::Value>(&mut connection)
            .await?;
        let issues: Box<[SyncIssue]> = stored
            .into_iter()
            .filter_map(|daten| {
                serde_json::from_value(daten)
                    .inspect_err(|error| warn!("Netbox sync issue not readable: {error}"))
                    .ok()
            })
            .collect();
        let ports: HashSet<i32> = schema::panel_port::table
            .filter(schema::panel_port::id.eq_any(issues.iter().flat_map(SyncIssue::port_ids)))
            .select(schema::panel_port::id)
            .load(&mut connection)
            .await?
            .into_iter()
            .collect();
        let panels: HashSet<i32> = schema::panel::table
            .filter(schema::panel::id.eq_any(issues.iter().flat_map(SyncIssue::panel_ids)))
            .select(schema::panel::id)
            .load(&mut connection)
            .await?
            .into_iter()
            .collect();
        Ok(issues
            .into_iter()
            .filter(|issue| {
                issue.port_ids().iter().all(|id| ports.contains(id))
                    && issue.panel_ids().iter().all(|id| panels.contains(id))
            })
            .collect())
    }
    /// Why the last run failed
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn error(&self) -> Option<NetboxSyncError> {
        let error = self.error.as_ref()?;
        Some(NetboxSyncError {
            message: error["message"].as_str().unwrap_or_default().to_string(),
            extensions: error
                .get("extensions")
                .filter(|extensions| !extensions.is_null())
                .map(serde_json::Value::to_string),
        })
    }
}
