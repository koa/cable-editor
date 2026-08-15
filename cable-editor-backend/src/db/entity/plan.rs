use crate::db::entity::cable::Cable;
use crate::db::entity::panel::{Panel, PanelPort, PortSide};
use crate::db::schema;
use crate::graphql::authenticated::get_connection;
use crate::graphql::authenticated::planned::PlannedPanel;
use async_graphql::{Context, Enum, Object, SimpleObject};
use diesel::sql_types::Integer;
use diesel::{HasQuery, Identifiable, Insertable, QueryableByName, sql_query};
use diesel_async::RunQueryDsl;
use diesel_derive_enum::DbEnum;

#[derive(QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::plan)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Plan {
    pub id: i32,
    pub name: String,
    pub status: PlanStatusType,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq, DbEnum, Enum, Hash, PartialOrd, Ord)]
#[ExistingTypePath = "crate::db::schema::sql_types::PlanStatusEnum"]
pub enum PlanStatusType {
    #[db_rename = "Open"]
    Open,

    #[db_rename = "Implemented"]
    Implemented,

    #[db_rename = "Rejected"]
    Rejected,
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
    async fn status(&self) -> PlanStatusType {
        self.status
    }

    async fn root_panels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlannedPanel>> {
        {
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
SELECT p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order
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
    }
}
