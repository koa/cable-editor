use crate::db::entity::{Panel, PanelPort, PanelPortType, Plan, PortSide, PortUsage};
use crate::db::schema::{panel, panel_port, port_usage};
use crate::graphql::authenticated::get_connection;
use async_graphql::{Context, Object};
use diesel::HasQuery;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::sql_types::Integer;
use diesel::{ExpressionMethods, sql_query};
use diesel_async::RunQueryDsl;
pub struct PlannedPanel {
    pub panel: Panel,
    pub plan: Plan,
}
pub struct PlannedPort {
    pub port: PanelPort,
    pub plan: Plan,
}

#[Object]
impl PlannedPanel {
    async fn parent(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<PlannedPanel>> {
        if let Some(parent_panel_id) = self.panel.parent_panel {
            let mut connection = get_connection(ctx).await?;
            Ok(Some(
                Panel::query()
                    .filter(panel::id.eq(parent_panel_id))
                    .first(&mut connection)
                    .await
                    .map(|panel| PlannedPanel {
                        panel,
                        plan: self.plan.clone(),
                    })?,
            ))
        } else {
            Ok(None)
        }
    }
    async fn children(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlannedPanel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(panel::parent_panel.eq(self.panel.id))
            .order(panel::parent_order.asc())
            .load(&mut connection)
            .await
            .map(|panels| {
                panels
                    .into_iter()
                    .map(|panel| PlannedPanel {
                        panel,
                        plan: self.plan.clone(),
                    })
                    .collect()
            })?)
    }
    async fn ports(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlannedPort>> {
        let mut connection = get_connection(ctx).await?;

        // Lade einfach alle existierenden Hardware-Ports für dieses Panel
        let ports = PanelPort::query()
            .filter(panel_port::panel_id.eq(self.panel.id))
            .order_by(panel_port::port_order.asc())
            .load::<PanelPort>(&mut connection)
            .await?;

        // Gib sie im Kontext des aktuellen Plans zurück
        Ok(ports
            .into_iter()
            .map(|port| PlannedPort {
                port,
                plan: self.plan.clone(),
            })
            .collect())
    }
    async fn all_children_recursive(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<PlannedPanel>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
        WITH RECURSIVE panel_tree AS (
            SELECT
                id, name, schacht_id, parent_panel, parent_order,
                1 as level
            FROM panel
            WHERE parent_panel = $1

            UNION ALL

            SELECT
                p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order,
                pt.level + 1 as level
            FROM panel p
            INNER JOIN panel_tree pt ON p.parent_panel = pt.id
        )
        SELECT
            id, name, schacht_id, parent_panel, parent_order
        FROM panel_tree
        ORDER BY level, parent_order;
    "#;

        Ok(sql_query(raw_sql)
            .bind::<Integer, _>(self.panel.id)
            .load::<Panel>(&mut connection)
            .await
            .map(|panels| {
                panels
                    .into_iter()
                    .map(|panel| PlannedPanel {
                        panel,
                        plan: self.plan.clone(),
                    })
                    .collect()
            })?)
    }
}
#[Object]
impl PlannedPort {
    async fn id(&self) -> i32 {
        self.port.id
    }
    async fn order_number(&self) -> i32 {
        self.port.port_order
    }
    async fn port_type(&self) -> PanelPortType {
        self.port.port_type
    }
    async fn label(&self) -> Option<&str> {
        self.port.label.as_deref()
    }

    /// Lädt die effektive Belegung für eine bestimmte Seite des Ports
    async fn usage(
        &self,
        ctx: &Context<'_>,
        side: PortSide,
    ) -> async_graphql::Result<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;

        let usage = port_usage::table
            .filter(port_usage::port_id.eq(self.port.id))
            .filter(port_usage::side.eq(side))
            // Wir betrachten nur die Baseline (0) und den aktuellen Plan
            .filter(port_usage::plan_id.eq_any([0, self.plan.id]))
            // Der höchste plan_id gewinnt (Plan überschreibt Baseline)
            .order_by(port_usage::plan_id.desc())
            .first::<PortUsage>(&mut connection)
            .await
            .optional()?;

        // Wenn ein Eintrag existiert, prüfen wir, ob es ein "Tombstone" (Löschung) ist.
        // Falls cable == None ist, wurde die Faser in diesem Plan absichtlich entfernt.
        if let Some(u) = usage {
            if u.cable.is_none() {
                return Ok(None);
            }
            return Ok(Some(u));
        }

        Ok(None)
    }
}
