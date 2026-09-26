use crate::{
    db::{
        entity::panel::{Panel, PortUsage},
        schema,
    },
    graphql::authenticated::{get_connection, planned::PlannedPanel},
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

    async fn root_panels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlannedPanel>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
WITH RECURSIVE affected_panels AS (
    -- 1. Basisfall (Anchor):
    -- Finde alle Panels, die in dieser plan_id Belegungen (port_usage) haben
    SELECT p.id, p.parent_panel
    FROM panel p
    WHERE EXISTS (
        SELECT 1
        FROM port_usage pu
        JOIN panel_port pp ON pu.port_id = pp.id
        WHERE pp.panel_id = p.id AND pu.plan_id = $1
    )
    UNION
    -- 2. Rekursiver Schritt: Klettere nach oben
    SELECT parent.id, parent.parent_panel
    FROM panel parent
    INNER JOIN affected_panels child ON child.parent_panel = parent.id
)
-- 3. Finale Ausgabe: Root-Panels filtern
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
    async fn panel(
        &self,
        ctx: &Context<'_>,
        panel_id: i32,
    ) -> async_graphql::Result<Option<PlannedPanel>> {
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
    async fn usage(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PortUsage::query()
            .filter(schema::port_usage::plan_id.eq(self.id))
            .load(&mut connection)
            .await?)
    }
}
