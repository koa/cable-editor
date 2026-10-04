use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::panel::{Panel, PanelPort, PortUsage},
        schema,
    },
    graphql::authenticated::{
        get_connection,
        planned::{PlannedPanel, PortChange},
    },
};
use async_graphql::{Context, Object};
use diesel::{
    AsChangeset, ExpressionMethods, HasQuery, Identifiable, Insertable, OptionalExtension,
    QueryDsl, QueryableByName, sql_query, sql_types::Integer,
};
use diesel_async::RunQueryDsl;

#[derive(
    QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq, AsChangeset,
)]
#[diesel(table_name = schema::plan)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Plan {
    pub id: i32,
    pub name: String,
    /// Netbox shows the circuits of this plan (at most one, see docs/netbox-sync.md)
    pub netbox_active: bool,
}

/// The plan holding the current state. Every other plan holds the changes it plans on top of
/// it and is deleted once implemented (merged into the baseline). SQL (the port_usage trigger,
/// raw queries) uses the literal 0.
pub const BASELINE_PLAN_ID: i32 = 0;

impl Plan {
    pub fn is_baseline(&self) -> bool {
        self.id == BASELINE_PLAN_ID
    }
}

#[derive(Insertable)]
#[diesel(table_name = schema::plan)]
pub struct InsertPlan {
    pub name: String,
}

#[Object]
impl Plan {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Whether this is the current state rather than a planned change of it
    #[graphql(name = "isBaseline")]
    async fn graphql_is_baseline(&self) -> bool {
        self.is_baseline()
    }
    /// Whether Netbox shows the circuits of this plan (synced automatically)
    async fn netbox_active(&self) -> bool {
        self.netbox_active
    }

    async fn root_panels(&self, ctx: &Context<'_>) -> ApiResult<Box<[PlannedPanel]>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
WITH RECURSIVE affected_panels AS (
    -- The panels with ports the plan changes
    SELECT p.id, p.parent_panel
    FROM panel p
    WHERE EXISTS (
        SELECT 1
        FROM port_usage pu
        JOIN panel_port pp ON pu.port_id = pp.id
        WHERE pp.panel_id = p.id AND pu.plan_id = $1
    )
    UNION
    -- and the panels above them
    SELECT parent.id, parent.parent_panel
    FROM panel parent
    INNER JOIN affected_panels child ON child.parent_panel = parent.id
)
-- of which the root panels
SELECT p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order, p.netbox_device_id
FROM affected_panels a
JOIN panel p ON a.id = p.id
WHERE a.parent_panel IS NULL;
            "#;
        Ok(sql_query(raw_sql)
            .bind::<Integer, _>(self.id)
            .load::<Panel>(&mut connection)
            .await
            .map(|panels| {
                panels
                    .into_iter()
                    .map(|panel| PlannedPanel {
                        panel,
                        plan: self.clone(),
                    })
                    .collect()
            })?)
    }
    async fn panel(&self, ctx: &Context<'_>, panel_id: i32) -> ApiResult<Option<PlannedPanel>> {
        {
            let mut connection = get_connection(ctx).await?;
            Ok(Panel::query()
                .filter(schema::panel::id.eq(panel_id))
                .first(&mut connection)
                .await
                .optional()?
                .map(|panel| PlannedPanel {
                    panel,
                    plan: self.clone(),
                }))
        }
    }
    /// The ports whose fibers this plan changes, with their fibers in the current state and in
    /// the plan, by panel and position: what is to be done to implement it (the work order).
    /// None for the baseline.
    async fn changed_ports(&self, ctx: &Context<'_>) -> ApiResult<Box<[PortChange]>> {
        if self.is_baseline() {
            return Ok(Box::default());
        }
        let mut connection = get_connection(ctx).await?;
        let planned = PortUsage::query()
            .filter(schema::port_usage::plan_id.eq(self.id))
            .load(&mut connection)
            .await?;
        let port_ids = planned.iter().map(|row| row.port_id).collect::<Box<[_]>>();
        let current = PortUsage::query()
            .filter(schema::port_usage::plan_id.eq(BASELINE_PLAN_ID))
            .filter(schema::port_usage::port_id.eq_any(&port_ids))
            .load(&mut connection)
            .await?;
        let ports = PanelPort::query()
            .filter(schema::panel_port::id.eq_any(&port_ids))
            .load(&mut connection)
            .await?;
        Ok(PortChange::of(ports, &current, &planned))
    }
    async fn usage(&self, ctx: &Context<'_>) -> ApiResult<Vec<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PortUsage::query()
            .filter(schema::port_usage::plan_id.eq(self.id))
            .load(&mut connection)
            .await?)
    }
}
