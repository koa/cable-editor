use crate::{config::CONFIG, graphql::context::UserInfo};
use async_graphql::{Context, Enum, Guard};

/// What a user may do, from the OIDC groups mapped in the config; each role includes the ones
/// before it. Every logged in user may read.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Reader,
    /// Plans and changes the master data (cables, panels, ports)
    Planner,
    /// Also implements plans, syncs them to Netbox and deletes cables
    Admin,
}

impl UserInfo {
    pub fn role(&self) -> Role {
        if self.member_of_any(CONFIG.admin_groups()) {
            Role::Admin
        } else if self.member_of_any(CONFIG.planner_groups()) {
            Role::Planner
        } else {
            Role::Reader
        }
    }

    fn member_of_any<'a>(&self, mut groups: impl Iterator<Item = &'a str>) -> bool {
        groups.any(|group| self.groups.iter().any(|g| g.as_ref() == group))
    }
}

/// Lets only users with at least the given role run a field, e.g.
/// `#[graphql(guard = "RoleGuard(Role::Planner)")]` on each mutation.
pub struct RoleGuard(pub Role);

impl Guard for RoleGuard {
    async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        if ctx.data::<UserInfo>()?.role() >= self.0 {
            Ok(())
        } else {
            Err("Keine Berechtigung für diese Änderung".into())
        }
    }
}
