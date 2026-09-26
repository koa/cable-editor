//! The mutations, one object per kind of thing they change; `Mutation` merges them into one
//! GraphQL type. Every mutation needs a `RoleGuard` (see `graphql/authorization.rs`).

mod cable;
mod duct;
pub mod implement;
mod panel;
mod plan;
mod schacht;
pub mod sync;

use async_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct Mutation(
    cable::CableMutation,
    schacht::SchachtMutation,
    duct::DuctMutation,
    panel::PanelMutation,
    plan::PlanMutation,
);
