//! "Datenprüfung" (admins, docs/datenpruefung.md): the port usages not fitting their cables, in
//! every plan, to delete.

use crate::components::load::Load;
use crate::{
    components::{
        dialog::{confirm_delete, confirm_delete_all},
        fiber::FiberNumber,
        links::{CableLink, PanelLink},
        page_layout::PageLayout,
        plan_link::PlanNameLink,
        port_usage_issues::notify_changed,
        table::ListModel,
    },
    error::{FrontendError, messages},
    graphql::authenticated::{
        CableRef, PanelPortInfo, PortSide,
        port_usage_issues::{
            PlanName, PortUsageIssue, PortUsageKeyInput, PortUsageProblem, delete_broken,
            fetch_issues,
        },
    },
    util::{get_credentials, toast_error, toast_success},
};
use patternfly_yew::prelude::{
    Button, ButtonVariant, Cell, CellContext, ExpansionState, MemoizedTableModel, Table,
    TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Callback, Component, Context, Html, Properties, html, html_nested, platform::spawn_local,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Plan,
    Port,
    Side,
    Fiber,
    Problem,
    Actions,
}

/// A usage with its problems (the view has a row per problem).
#[derive(Clone, PartialEq)]
struct UsageRow {
    plan: PlanName,
    port: PanelPortInfo,
    side: PortSide,
    cable: CableRef,
    bundle: i32,
    fiber: i32,
    problems: Vec<PortUsageProblem>,
    /// The click handler of its "Löschen"
    ondelete: Callback<web_sys::MouseEvent>,
}

impl TableEntryRenderer<Columns> for UsageRow {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::Plan => Cell::new(html! {
                <PlanNameLink id={self.plan.id} text={self.plan.name.clone()}/>
            }),
            // In the plan of the usage, where it can be changed too
            Columns::Port => Cell::new(html! {
                <PanelLink id={self.port.panel_id()} text={self.port.port_label()} plan_id={Some(self.plan.id)}/>
            }),
            Columns::Side => Cell::new(html!(match self.side {
                PortSide::FRONT => "Front",
                PortSide::BACK => "Back",
            })),
            // One element per cell: on phones the cell lays its children out as a grid
            Columns::Fiber => Cell::new(html! {
                <span>
                    <CableLink id={self.cable.id} text={self.cable.name.clone()}/>{" "}
                    <FiberNumber bundle={self.bundle} fiber={self.fiber}/>
                </span>
            }),
            Columns::Problem => Cell::new(html! {
                <span>
                    {self.problems.iter().map(|problem| capitalized(messages::port_usage_problem((*problem).into()))).collect::<Vec<_>>().join("; ")}
                </span>
            }),
            Columns::Actions => Cell::new(html! {
                <Button label="Löschen" variant={ButtonVariant::DangerSecondary} onclick={self.ondelete.clone()}/>
            }),
        }
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[derive(Properties, PartialEq)]
pub struct PortUsageIssuesPageProps {
    pub plan_id: i32,
}

pub struct PortUsageIssuesPage {
    rows: Load<Rc<Vec<UsageRow>>>,
    deleting: bool,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Load,
    Loaded(Result<Vec<PortUsageIssue>, FrontendError>),
    Delete(Vec<PortUsageKeyInput>),
    Deleted(Result<i32, FrontendError>),
}

impl Component for PortUsageIssuesPage {
    type Message = Msg;
    type Properties = PortUsageIssuesPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Load);
        Self {
            rows: Load::Pending,
            deleting: false,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Load => {
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(Msg::Loaded(fetch_issues(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(issues) => {
                self.rows = Load::from(issues.map(|issues| Rc::new(self.rows_of(ctx, issues))));
                true
            }
            Msg::Delete(usages) => {
                self.deleting = true;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(Msg::Deleted(
                        delete_broken(credentials.as_ref(), usages).await,
                    ));
                });
                true
            }
            Msg::Deleted(result) => {
                self.deleting = false;
                match result {
                    Ok(count) => {
                        toast_success(
                            ctx.link(),
                            match count {
                                1 => "Belegung gelöscht".to_string(),
                                count => format!("{count} Belegungen gelöscht"),
                            },
                        );
                        ctx.link().send_message(Msg::Load);
                        notify_changed();
                    }
                    Err(error) => toast_error(
                        ctx.link(),
                        "Belegungen konnten nicht gelöscht werden",
                        error,
                    ),
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = self.rows.view(|rows| self.view_rows(ctx, rows));
        html!(<PageLayout title="Datenprüfung">{content}</PageLayout>)
    }
}

impl PortUsageIssuesPage {
    /// Groups the problems by usage, each with its "Löschen".
    fn rows_of(&self, ctx: &Context<Self>, issues: Vec<PortUsageIssue>) -> Vec<UsageRow> {
        let mut rows: Vec<UsageRow> = Vec::new();
        for issue in issues {
            let key = issue.key();
            if let Some(row) = rows.iter_mut().find(|row| {
                row.plan.id == key.plan_id && row.port.id == key.port_id && row.side == key.side
            }) {
                row.problems.push(issue.problem);
                continue;
            }
            let name = format!(
                "{} {}-{} an {} ({})",
                issue.cable.name,
                issue.bundle,
                issue.fiber,
                issue.port.port_label(),
                issue.plan.name
            );
            let on_confirm = ctx.link().callback(move |()| Msg::Delete(vec![key]));
            rows.push(UsageRow {
                ondelete: confirm_delete(ctx.link(), "Belegung", &name, on_confirm),
                plan: issue.plan,
                port: issue.port,
                side: issue.side,
                cable: issue.cable,
                bundle: issue.bundle,
                fiber: issue.fiber,
                problems: vec![issue.problem],
            });
        }
        rows
    }

    fn view_rows(&self, ctx: &Context<Self>, rows: &Rc<Vec<UsageRow>>) -> Html {
        if rows.is_empty() {
            return html!(<p>{"Alle Port-Belegungen passen zu ihren Kabeln."}</p>);
        }
        let header = html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Planung" index={Columns::Plan}/>
                <TableColumn<Columns> label="Port" index={Columns::Port}/>
                <TableColumn<Columns> label="Seite" index={Columns::Side}/>
                <TableColumn<Columns> label="Faser" index={Columns::Fiber}/>
                <TableColumn<Columns> label="Problem" index={Columns::Problem}/>
                <TableColumn<Columns> index={Columns::Actions}/>
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(rows.clone()),
            self.table_state.clone(),
        );
        let all: Vec<PortUsageKeyInput> = rows
            .iter()
            .map(|row| PortUsageKeyInput {
                port_id: row.port.id,
                plan_id: row.plan.id,
                side: row.side,
            })
            .collect();
        let delete_all = confirm_delete_all(
            ctx.link(),
            "Belegungen",
            all.len(),
            ctx.link().callback(move |()| Msg::Delete(all.clone())),
        );
        html! {
            <>
                <p class="pf-v6-u-mb-md">
                    {"Diese Ports sind mit Fasern belegt, die nicht zu ihrem Kabel passen, zum Beispiel weil der Weg des Kabels geändert wurde. Löschen macht die Seite des Ports frei: im Ist-Zustand sofort, in einer Planung als Änderung der Planung. Wer die Faser anders auflegen will, tut das im Panel selbst. Liegt eine Faser an zwei Ports, bleibt sie am zweiten, sobald der erste gelöscht ist."}
                </p>
                <Table<Columns, ListModel<Columns, MemoizedTableModel<UsageRow>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
                <div class="pf-v6-u-mt-md">
                    <Button
                        label="Alle löschen"
                        variant={ButtonVariant::DangerSecondary}
                        disabled={self.deleting}
                        onclick={delete_all}
                    />
                </div>
            </>
        }
    }
}
