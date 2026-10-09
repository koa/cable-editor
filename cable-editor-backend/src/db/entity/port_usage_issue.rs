//! Port usages not fitting their cables (the view `port_usage_issue`), and the check refusing
//! changes that would add ones.

use crate::db::{
    entity::{
        cable::Cable,
        panel::{PanelPort, PortSide},
        plan::Plan,
    },
    schema,
};
use crate::graphql::{
    error::ApiResult,
    loader::{CableId, PanelPortId, PlanId, load_one},
};
use async_graphql::{Context, Enum, Object};
use cable_editor_common::{UserError, error::BrokenPortUsage};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use diesel_derive_enum::DbEnum;
use std::collections::HashSet;

/// Why a port usage doesn't fit its cable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DbEnum, Enum)]
#[ExistingTypePath = "crate::db::schema::sql_types::PortUsageProblemEnum"]
pub enum PortUsageProblem {
    /// The cable doesn't end in the Schacht of the port's panel
    #[db_rename = "CableNotEnding"]
    CableNotEnding,
    /// The cable has no such bundle or fiber
    #[db_rename = "FiberOutOfRange"]
    FiberOutOfRange,
    /// The same fiber end is at another port side too
    #[db_rename = "FiberTwice"]
    FiberTwice,
}

impl From<PortUsageProblem> for cable_editor_common::error::PortUsageProblem {
    fn from(problem: PortUsageProblem) -> Self {
        match problem {
            PortUsageProblem::CableNotEnding => Self::CableNotEnding,
            PortUsageProblem::FiberOutOfRange => Self::FiberOutOfRange,
            PortUsageProblem::FiberTwice => Self::FiberTwice,
        }
    }
}

/// The port usages a change can affect.
pub enum IssueScope<'a> {
    /// These ports in a plan
    Ports { plan_id: i32, port_ids: &'a [i32] },
    /// Every usage of a cable, in all plans
    Cable(i32),
}

/// A problem of a port usage: port, plan, side and problem.
type IssueKey = (i32, i32, PortSide, PortUsageProblem);

/// Refuses a change adding problems to the usages of its scope: `before` takes the problems
/// there are, `after` (once the change is written) fails with the new ones. Existing problems
/// don't block a change, so they can be repaired.
pub struct IssueCheck<'a> {
    scope: IssueScope<'a>,
    before: HashSet<IssueKey>,
}

impl<'a> IssueCheck<'a> {
    pub async fn before(
        connection: &mut AsyncPgConnection,
        scope: IssueScope<'a>,
    ) -> ApiResult<Self> {
        let before = load(connection, &scope)
            .await?
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        Ok(Self { scope, before })
    }

    pub async fn after(self, connection: &mut AsyncPgConnection) -> ApiResult<()> {
        let usages: Box<[BrokenPortUsage]> = load(connection, &self.scope)
            .await?
            .into_iter()
            .filter(|(key, _)| !self.before.contains(key))
            .map(|(_, usage)| usage)
            .collect();
        if usages.is_empty() {
            Ok(())
        } else {
            Err(UserError::PortUsagesBroken { usages }.into())
        }
    }
}

async fn load(
    connection: &mut AsyncPgConnection,
    scope: &IssueScope<'_>,
) -> ApiResult<Box<[(IssueKey, BrokenPortUsage)]>> {
    use schema::port_usage_issue as issue;
    let mut query = PortUsageIssue::query()
        .order_by((issue::plan_id, issue::port_id, issue::side))
        .into_boxed();
    query = match scope {
        IssueScope::Ports { plan_id, port_ids } => query
            .filter(issue::plan_id.eq(*plan_id))
            .filter(issue::port_id.eq_any(port_ids.iter().copied())),
        IssueScope::Cable(cable_id) => query.filter(issue::cable.eq(*cable_id)),
    };
    Ok(query
        .load(connection)
        .await?
        .into_iter()
        .map(|issue: PortUsageIssue| {
            (
                (issue.port_id, issue.plan_id, issue.side, issue.problem),
                BrokenPortUsage {
                    plan: issue.plan_id,
                    port: issue.port_id,
                    cable: issue.cable,
                    bundle: issue.bundle,
                    fiber: issue.fiber,
                    problem: issue.problem.into(),
                },
            )
        })
        .collect())
}

/// A row of the view `port_usage_issue`.
#[derive(HasQuery, Debug, Clone, Copy, PartialEq)]
#[diesel(table_name = schema::port_usage_issue)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PortUsageIssue {
    pub port_id: i32,
    pub plan_id: i32,
    pub side: PortSide,
    pub cable: i32,
    pub bundle: i32,
    pub fiber: i32,
    pub problem: PortUsageProblem,
}

#[Object]
impl PortUsageIssue {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
    async fn plan(&self, ctx: &Context<'_>) -> ApiResult<Plan> {
        load_one(ctx, PlanId(self.plan_id)).await
    }
    async fn side(&self) -> PortSide {
        self.side
    }
    async fn cable(&self, ctx: &Context<'_>) -> ApiResult<Cable> {
        load_one(ctx, CableId(self.cable)).await
    }
    async fn bundle(&self) -> i32 {
        self.bundle
    }
    async fn fiber(&self) -> i32 {
        self.fiber
    }
    async fn problem(&self) -> PortUsageProblem {
        self.problem
    }
}

/// All port usages not fitting their cables, by plan, port and side.
pub async fn all(connection: &mut AsyncPgConnection) -> ApiResult<Box<[PortUsageIssue]>> {
    use schema::port_usage_issue as issue;
    Ok(PortUsageIssue::query()
        .order_by((issue::plan_id, issue::port_id, issue::side, issue::problem))
        .load(connection)
        .await?
        .into_boxed_slice())
}
