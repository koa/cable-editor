use crate::db::entity::plan::BASELINE_PLAN_ID;
use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{
            cable::Fiber,
            panel::{Panel, PanelPort, PanelPortType, PortSide, PortUsage},
            plan::Plan,
        },
        schema::{panel, panel_port, port_usage},
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Object};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;

pub struct PlannedPanel {
    pub panel: Panel,
    pub plan: Plan,
}
pub struct PlannedPort {
    pub port: PanelPort,
    pub plan: Plan,
}

/// A port whose fibers a plan changes: what it holds in the current state and in the plan
/// (`Plan.changedPorts`, the plan's work order).
pub struct PortChange {
    pub port: PanelPort,
    pub current_front: Option<Fiber>,
    pub current_back: Option<Fiber>,
    pub planned_front: Option<Fiber>,
    pub planned_back: Option<Fiber>,
}

impl PortChange {
    /// The ports `planned` (the rows of a plan) changes, compared with `current` (the rows of
    /// the baseline for the same ports), by panel and position; a row of the plan without a
    /// fiber removes the fiber, one equal to the current state changes nothing.
    pub fn of(
        ports: impl IntoIterator<Item = PanelPort>,
        current: &[PortUsage],
        planned: &[PortUsage],
    ) -> Box<[PortChange]> {
        let fiber_of = |rows: &[PortUsage], port_id: i32, side: PortSide| {
            rows.iter()
                .find(|row| row.port_id == port_id && row.side == side)
                .map(PortUsage::used_fiber)
        };
        let mut changes = ports
            .into_iter()
            .filter_map(|port| {
                let current_front = fiber_of(current, port.id, PortSide::Front).flatten();
                let current_back = fiber_of(current, port.id, PortSide::Back).flatten();
                let planned_front =
                    fiber_of(planned, port.id, PortSide::Front).unwrap_or(current_front);
                let planned_back =
                    fiber_of(planned, port.id, PortSide::Back).unwrap_or(current_back);
                (current_front != planned_front || current_back != planned_back).then_some(
                    PortChange {
                        port,
                        current_front,
                        current_back,
                        planned_front,
                        planned_back,
                    },
                )
            })
            .collect::<Box<[_]>>();
        changes.sort_by_key(|change| (change.port.panel_id, change.port.port_order));
        changes
    }
}

#[Object]
impl PortChange {
    async fn port(&self) -> &PanelPort {
        &self.port
    }
    /// The fiber on the front in the current state
    async fn current_front(&self) -> Option<Fiber> {
        self.current_front
    }
    async fn current_back(&self) -> Option<Fiber> {
        self.current_back
    }
    /// The fiber on the front once the plan is implemented
    async fn planned_front(&self) -> Option<Fiber> {
        self.planned_front
    }
    async fn planned_back(&self) -> Option<Fiber> {
        self.planned_back
    }
}

#[Object]
impl PlannedPanel {
    async fn panel(&self) -> &Panel {
        &self.panel
    }
    async fn plan(&self) -> &Plan {
        &self.plan
    }
    async fn parent(&self, ctx: &Context<'_>) -> ApiResult<Option<PlannedPanel>> {
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
    async fn children(&self, ctx: &Context<'_>) -> ApiResult<Vec<PlannedPanel>> {
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
    async fn ports(&self, ctx: &Context<'_>) -> ApiResult<Vec<PlannedPort>> {
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
    async fn all_children_recursive(&self, ctx: &Context<'_>) -> ApiResult<Vec<PlannedPanel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(
            Panel::load_all_children_recursive(self.panel.id, &mut connection)
                .await?
                .into_iter()
                .map(|panel| PlannedPanel {
                    panel,
                    plan: self.plan.clone(),
                })
                .collect(),
        )
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
    async fn usage(&self, ctx: &Context<'_>, side: PortSide) -> ApiResult<Option<PortUsage>> {
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
        Ok(usage.filter(|u| u.fiber.is_some()))
    }
    async fn current_usage(
        &self,
        ctx: &Context<'_>,
        side: PortSide,
    ) -> ApiResult<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;

        let usage = port_usage::table
            .filter(port_usage::port_id.eq(self.port.id))
            .filter(port_usage::side.eq(side))
            .filter(port_usage::plan_id.eq(BASELINE_PLAN_ID))
            .first::<PortUsage>(&mut connection)
            .await
            .optional()?;
        Ok(usage.filter(|u| u.fiber.is_some()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(id: i32, panel_id: i32, port_order: i32) -> PanelPort {
        PanelPort {
            id,
            panel_id,
            port_order,
            label: None,
            port_type: PanelPortType::Splice,
            netbox_port_id: None,
        }
    }

    fn row(port_id: i32, plan_id: i32, side: PortSide, fiber: Option<i32>) -> PortUsage {
        PortUsage {
            port_id,
            plan_id,
            side,
            cable: fiber.map(|_| 7),
            bundle: fiber.map(|_| 1),
            fiber,
        }
    }

    fn fiber(fiber: i32) -> Option<Fiber> {
        Some(Fiber {
            cable: 7,
            bundle: 1,
            fiber,
        })
    }

    #[test]
    fn compares_the_plan_with_the_current_state() {
        let current = [
            row(1, 0, PortSide::Front, Some(1)),
            row(1, 0, PortSide::Back, Some(2)),
            row(2, 0, PortSide::Front, Some(3)),
            row(3, 0, PortSide::Front, Some(4)),
        ];
        let planned = [
            // the back changes, the front stays
            row(1, 1, PortSide::Back, Some(5)),
            // removed
            row(2, 1, PortSide::Front, None),
            // the same as now
            row(3, 1, PortSide::Front, Some(4)),
            // new
            row(4, 1, PortSide::Front, Some(6)),
        ];
        let ports = [
            port(4, 10, 1),
            port(3, 10, 3),
            port(2, 11, 1),
            port(1, 10, 2),
        ];
        let changes = PortChange::of(ports, &current, &planned);
        let summary = changes
            .iter()
            .map(|c| {
                (
                    c.port.id,
                    c.current_front.map(|f| f.fiber),
                    c.current_back.map(|f| f.fiber),
                    c.planned_front.map(|f| f.fiber),
                    c.planned_back.map(|f| f.fiber),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            [
                (4, None, None, Some(6), None),
                (1, Some(1), Some(2), Some(1), Some(5)),
                (2, Some(3), None, None, None),
            ]
        );
        assert_eq!(fiber(6), changes[0].planned_front);
    }
}
