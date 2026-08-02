use crate::db::entity::{Panel, PanelPort, Plan};
use crate::db::schema::{panel, panel_port};
use crate::graphql::authenticated::get_connection;
use async_graphql::{Context, Object};
use diesel::HasQuery;
use diesel::QueryDsl;
use diesel::sql_types::Integer;
use diesel::{ExpressionMethods, sql_query};
use diesel_async::RunQueryDsl;
pub struct PlannedPanel {
    pub panel: Panel,
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
    async fn ports(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PanelPort>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PanelPort::query()
            .filter(panel_port::panel_id.eq(self.panel.id))
            .filter(panel_port::plan_id.eq_any([0, self.plan.id]))
            .distinct_on(panel_port::port_number)
            .order_by((panel_port::port_number.asc(), panel_port::plan_id.desc()))
            .load(&mut connection)
            .await?)
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
