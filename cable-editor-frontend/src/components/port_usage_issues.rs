//! Port usages not fitting their cables (docs/datenpruefung.md): the list in the toast of a
//! refused change, which refers to them by id (their names loaded when shown), and the hint in
//! the breadcrumb bar while there are such usages.

use crate::{
    components::{
        fiber::FiberNumber,
        load::Load,
        menu::popup::{MenuGroup, MenuLinkItem, PopupMenu},
    },
    error::{FrontendError, messages},
    graphql::authenticated::{
        current_user::Role,
        list_plans::BASELINE_PLAN_ID,
        port_usage_issues::{PortUsageNames, fetch_broken_count},
    },
    pages::router::{AppRoute, PlanView},
    util::{get_credentials, get_role},
};
use cable_editor_common::error::BrokenPortUsage;
use gloo_events::EventListener;
use gloo_timers::callback::Interval;
use patternfly_yew::prelude::{Icon, MenuToggleVariant, Spinner, SpinnerSize};
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
        .map_or_else(
            || format!("Planung {}", usage.plan),
            |plan| plan.name.clone(),
        );
    let port = names
        .ports
        .iter()
        .find(|port| port.id == usage.port)
        .map_or_else(|| format!("Port {}", usage.port), |port| port.port_label());
    let cable = names
        .list_cable
        .iter()
        .find(|cable| cable.id == usage.cable)
        .map_or_else(
            || format!("Kabel {}", usage.cable),
            |cable| cable.name.clone(),
        );
    html! {
        <li>
            {format!("{plan} · {port} · {cable} ")}
            <FiberNumber bundle={usage.bundle} fiber={usage.fiber}/>
            {" · "}{messages::port_usage_problem(usage.problem)}
        </li>
    }
}

/// How often the hint asks again, in milliseconds (besides on every navigation)
const REFRESH_MS: u32 = 60_000;
/// The event on the window telling the hint to ask again
const CHANGED_EVENT: &str = "port-usages-changed";

/// Tells the hint that usages not fitting were deleted, e.g. by the page "Datenprüfung".
pub fn notify_changed() {
    if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::Event::new(CHANGED_EVENT)) {
        let _ = window.dispatch_event(&event);
    }
}

#[derive(Properties, PartialEq)]
pub struct BrokenPortUsageHintProps {
    /// The page shown: the hint asks again when it changes
    pub route: AppRoute,
}

/// In the breadcrumb bar, only while port usages don't fit their cables or that couldn't be
/// loaded: a menu explaining it, for admins with the way to the page "Datenprüfung".
pub struct BrokenPortUsageHint {
    count: Load<i32>,
    _refresh: Interval,
    /// `notify_changed`
    _changed: Option<EventListener>,
}

pub enum HintMsg {
    Fetch,
    Loaded(Result<i32, FrontendError>),
}

impl Component for BrokenPortUsageHint {
    type Message = HintMsg;
    type Properties = BrokenPortUsageHintProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(HintMsg::Fetch);
        let link = ctx.link().clone();
        let changed = web_sys::window().map(|window| {
            let link = ctx.link().clone();
            EventListener::new(&window, CHANGED_EVENT, move |_| {
                link.send_message(HintMsg::Fetch)
            })
        });
        Self {
            count: Load::Pending,
            _refresh: Interval::new(REFRESH_MS, move || link.send_message(HintMsg::Fetch)),
            _changed: changed,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            HintMsg::Fetch => {
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(HintMsg::Loaded(
                        fetch_broken_count(credentials.as_ref()).await,
                    ));
                });
                false
            }
            HintMsg::Loaded(count) => {
                self.count = Load::from(count);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().route != old_props.route {
            ctx.link().send_message(HintMsg::Fetch);
        }
        false
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let (text, message, details): (String, String, Box<[String]>) = match &self.count {
            Load::Pending | Load::Loaded(0) => return Html::default(),
            Load::Loaded(count) => (
                match count {
                    1 => "1 Belegung passt nicht".into(),
                    count => format!("{count} Belegungen passen nicht"),
                },
                "Ports sind mit Fasern von Kabeln belegt, die nicht im Schacht enden oder diese Faser nicht haben, oder eine Faser liegt an zwei Ports. Bis sie korrigiert sind, zeigen die Editoren diese Kabel womöglich nicht an.".into(),
                Box::default(),
            ),
            Load::Failed(error) => (
                "Datenprüfung unbekannt".into(),
                "Ob die Port-Belegungen zu ihren Kabeln passen, konnte nicht geladen werden:".into(),
                [error.title()].into_iter().chain(error.details()).collect(),
            ),
        };
        let admin = get_role(ctx.link()) >= Role::Admin;
        let plan_id = match ctx.props().route {
            AppRoute::Plan { plan_id, .. } => plan_id,
            AppRoute::ListOfPlans => BASELINE_PLAN_ID,
        };
        html! {
            <PopupMenu
                variant={MenuToggleVariant::Plain}
                icon={html!(<span class="bar-hint__icon bar-hint__icon--warning">{Icon::ExclamationTriangle}</span>)}
                text={html!(<span class="bar-hint__text">{text.clone()}</span>)}
                aria_label={text}
                align_end=true
            >
                <div class="bar-hint__message">
                    <p>{message}</p>
                    if !details.is_empty() {
                        <ul>{for details.iter().map(|detail| html!(<li>{detail}</li>))}</ul>
                    }
                    if !admin {
                        <p>{"Admins sehen und löschen sie auf der Seite „Datenprüfung“."}</p>
                    }
                </div>
                if admin {
                    <MenuGroup divider=true>
                        <MenuLinkItem to={AppRoute::Plan { plan_id, view: PlanView::Datenpruefung }}>
                            {"Datenprüfung anzeigen"}
                        </MenuLinkItem>
                    </MenuGroup>
                }
            </PopupMenu>
        }
    }
}
