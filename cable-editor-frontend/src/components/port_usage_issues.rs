//! Port usages not fitting their cables, as the backend refers to them (by id), with their names
//! loaded when shown.

use crate::{
    components::{fiber::FiberNumber, load::Load},
    error::{FrontendError, messages},
    graphql::authenticated::port_usage_issues::PortUsageNames,
    util::get_credentials,
};
use cable_editor_common::error::BrokenPortUsage;
use patternfly_yew::prelude::{Spinner, SpinnerSize};
use std::rc::Rc;
use yew::{Component, Context, Html, Properties, html, platform::spawn_local};

/// How many usages the list shows, the rest only counted (it is in a toast)
const SHOWN: usize = 8;

#[derive(Properties, PartialEq)]
pub struct BrokenPortUsageListProps {
    pub usages: Rc<[BrokenPortUsage]>,
}

/// A line per usage: plan, port, cable and fiber, problem.
pub struct BrokenPortUsageList {
    names: Load<PortUsageNames>,
}

pub enum Msg {
    Loaded(Result<PortUsageNames, FrontendError>),
}

impl Component for BrokenPortUsageList {
    type Message = Msg;
    type Properties = BrokenPortUsageListProps;

    fn create(ctx: &Context<Self>) -> Self {
        let port_ids = ctx
            .props()
            .usages
            .iter()
            .take(SHOWN)
            .map(|usage| usage.port)
            .collect();
        let scope = ctx.link().clone();
        spawn_local(async move {
            let credentials = get_credentials(&scope);
            scope.send_message(Msg::Loaded(
                PortUsageNames::fetch(credentials.as_ref(), port_ids).await,
            ));
        });
        Self {
            names: Load::Pending,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(result) => self.names = Load::from(result),
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let usages = &ctx.props().usages;
        let more = usages.len().saturating_sub(SHOWN);
        // Not the alert of `Load::view`: the list is in a toast, its error a line of it
        let lines = match &self.names {
            Load::Pending => html!(<Spinner size={SpinnerSize::Md} />),
            Load::Failed(error) => html!(<p>{error.title()}</p>),
            Load::Loaded(names) => html! {
                <ul class="pf-v6-c-list">
                    { for usages.iter().take(SHOWN).map(|usage| line(names, usage)) }
                </ul>
            },
        };
        html! {
            <>
                {lines}
                if more > 0 {
                    <p>{format!("und {more} weitere")}</p>
                }
            </>
        }
    }
}

fn line(names: &PortUsageNames, usage: &BrokenPortUsage) -> Html {
    let plan = names
        .list_plan
        .iter()
        .find(|plan| plan.id == usage.plan)
        .map_or_else(|| format!("Planung {}", usage.plan), |plan| plan.name.clone());
    let port = names
        .ports
        .iter()
        .find(|port| port.id == usage.port)
        .map_or_else(|| format!("Port {}", usage.port), |port| port.port_label());
    let cable = names
        .list_cable
        .iter()
        .find(|cable| cable.id == usage.cable)
        .map_or_else(|| format!("Kabel {}", usage.cable), |cable| cable.name.clone());
    html! {
        <li>
            {format!("{plan} · {port} · {cable} ")}
            <FiberNumber bundle={usage.bundle} fiber={usage.fiber}/>
            {" · "}{messages::port_usage_problem(usage.problem)}
        </li>
    }
}
