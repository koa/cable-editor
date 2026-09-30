use crate::components::menu::list_cabinet::ListCabinet;
use crate::components::menu::list_cabinet_type::ListCabinetType;
use crate::components::menu::list_cable::ListCable;
use crate::components::menu::list_duct::ListDuct;
use crate::components::menu::list_panel::ListPanel;
use crate::components::menu::list_plan::ListPlan;
use crate::components::menu::{MenuDropdown, MenuEntry};
use crate::{
    components::{
        netbox::NetboxHint,
        panel::{attach_fiber::AttachFiber, loop_editor::LoopPortEditor, show::ShowPanel},
        recovery::RetryScope,
        user::{RequireRole, UserMenu},
    },
    graphql::authenticated::{IdOrNew, current_user::Role},
    pages::{
        cabinet::{
            edit::EditCabinetPanels, list::ListOfCabinets, overview::CabinetOverview,
            properties::CabinetProperties,
        },
        cabinet_type::{list::ListOfCabinetTypes, properties::CabinetTypeProperties},
        cable::edit::EditCable,
        duct::{list::ListOfDucts, properties::EditDuctProperties, show::ShowDuct},
        list_of_cables::ListOfCables,
        lkmap::Leitungskataster,
        map::Map,
        netbox::NetboxPage,
        owner::list::ListOfOwners,
        panel::EditPanel,
        planning::{edit::EditPlan, list::ListOfPlannings},
    },
};
use patternfly_yew::prelude::{Breadcrumb, BreadcrumbItem, PageSection, PageSectionType};
use std::borrow::Cow;
use uuid::Uuid;
use yew::virtual_dom::VNode;
use yew::{Callback, Html, Properties, function_component, html, html_nested, use_effect_with};
use yew_nested_router::prelude::{Target, use_router};

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct SwitchProps<T>
where
    T: Target,
{
    /// The function rendering based on the active target.
    pub render: Callback<T, Html>,

    /// The default, in case no route is active (not found).
    #[prop_or_default]
    pub default: Html,
}

/// A component two switch rendering between the different targets.
#[function_component(Switch)]
pub fn switch<T>(props: &SwitchProps<T>) -> Html
where
    T: Target + 'static,
{
    // A missing router is a programming error, like a missing Yew context
    #[allow(clippy::expect_used)]
    let router = use_router::<T>().expect("Must be a child of a Router or Nested component");

    match router.active_target {
        Some(target) => props.render.emit(target),
        None => props.default.clone(),
    }
}

/// Rendered for URLs that match no route: replaces them with the list of plans.
#[function_component(RedirectToPlans)]
pub fn redirect_to_plans() -> Html {
    let router = use_router::<AppRoute>();
    use_effect_with((), move |_| {
        if let Some(router) = router {
            router.replace(AppRoute::ListOfPlans);
        }
    });
    Html::default()
}

#[derive(Debug, Clone, PartialEq, Eq, Target, Default)]
pub enum AppRoute {
    #[default]
    ListOfPlans,
    Plan {
        plan_id: i32,
        #[target(nested)]
        view: PlanView,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum CableView {
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum DuctView {
    Show,
    Properties,
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum CabinetView {
    Overview,
    Properties,
    Edit,
}
#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum PanelView {
    Edit,
    Loop,
    Attach,
    Show,
}

impl PanelView {
    pub fn title(&self) -> &'static str {
        match self {
            PanelView::Show => "Übersicht",
            PanelView::Edit => "Ports bearbeiten",
            PanelView::Attach => "Fasern auflegen",
            PanelView::Loop => "Loops verbinden",
        }
    }

    pub fn content(&self, plan_id: i32, panel_id: i32) -> Html {
        match self {
            PanelView::Edit => {
                html!(<EditPanel {plan_id} {panel_id}/>)
            }
            PanelView::Loop => {
                html!(<LoopPortEditor  {plan_id} {panel_id}/>)
            }
            PanelView::Attach => {
                html!(<AttachFiber {plan_id} {panel_id}/>)
            }
            PanelView::Show => {
                html!(<ShowPanel {plan_id} {panel_id}/>)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum PlanView {
    Edit,
    ListOfCabinets,
    /// `id` tells new Schächte apart (see `IdOrNew`), kept in the path so it stays the same
    NewCabinet {
        id: Uuid,
    },
    Cabinet {
        id: i32,
        #[target(nested)]
        view: CabinetView,
    },
    ListOfCables,
    Cable {
        id: i32,
        #[target(nested)]
        view: CableView,
    },
    Map,
    ListOfDucts,
    /// Like `NewCabinet`
    NewDuct {
        id: Uuid,
    },
    Duct {
        id: i32,
        #[target(nested)]
        view: DuctView,
    },
    ListOfOwners,
    ListOfCabinetTypes,
    /// Like `NewCabinet`
    NewCabinetType {
        id: Uuid,
    },
    CabinetType {
        id: i32,
    },
    /// The delivery to the Leitungskataster (admins)
    Leitungskataster,
    /// The automatic sync to Netbox (admins)
    Netbox,
    Panel {
        id: i32,
        #[target(nested)]
        view: PanelView,
    },
}

impl PlanView {
    pub fn append_breadcrumbs(&self, plan_id: i32, item_contents: &mut Vec<VNode>) {
        // The area the current page lies in: its entry leads to the area's start page
        let (title, area): (Cow<'static, str>, PlanView) = match self {
            PlanView::Edit => ("Ändern".into(), PlanView::Edit),
            PlanView::Cabinet { .. }
            | PlanView::ListOfCabinets
            | PlanView::NewCabinet { .. }
            | PlanView::Panel { .. } => ("Schacht".into(), PlanView::ListOfCabinets),
            PlanView::Cable { .. } | PlanView::ListOfCables => {
                ("Kabel".into(), PlanView::ListOfCables)
            }
            PlanView::Duct { .. } | PlanView::ListOfDucts | PlanView::NewDuct { .. } => {
                ("Trasse".into(), PlanView::ListOfDucts)
            }
            PlanView::Map => ("Karte".into(), PlanView::Map),
            PlanView::ListOfOwners => ("Eigentümer".into(), PlanView::ListOfOwners),
            PlanView::ListOfCabinetTypes
            | PlanView::NewCabinetType { .. }
            | PlanView::CabinetType { .. } => ("Schachttyp".into(), PlanView::ListOfCabinetTypes),
            PlanView::Leitungskataster => ("Leitungskataster".into(), PlanView::Leitungskataster),
            PlanView::Netbox => ("Netbox".into(), PlanView::Netbox),
        };

        // The areas of the plan, each leading to its start page
        let areas: [(&str, PlanView); 9] = [
            ("Plan bearbeiten", PlanView::Edit),
            ("Karte", PlanView::Map),
            ("Schacht", PlanView::ListOfCabinets),
            ("Kabel", PlanView::ListOfCables),
            ("Trasse", PlanView::ListOfDucts),
            ("Eigentümer", PlanView::ListOfOwners),
            ("Schachttyp", PlanView::ListOfCabinetTypes),
            ("Leitungskataster", PlanView::Leitungskataster),
            ("Netbox", PlanView::Netbox),
        ];
        let entries = areas
            .into_iter()
            .map(|(text, view)| MenuEntry {
                selected: view == area,
                text: text.into(),
                target: AppRoute::Plan { plan_id, view },
            })
            .collect::<Box<[_]>>();
        item_contents.push(html!(<MenuDropdown {title} {entries}/>));

        match self {
            PlanView::Cabinet { id, view } => {
                item_contents.push(
                    html!(<ListCabinet plan_id={plan_id} cabinet_id={*id} view={Some(view.clone())}/>),
                );
            }
            PlanView::Cable { id, view } => {
                item_contents.push(html!(<ListCable {plan_id} cable_id={id} view={view.clone()}/>))
            }
            PlanView::Panel { id, view } => {
                item_contents
                    .push(html!(<ListPanel {plan_id} panel_id={*id} view={view.clone()}/>));
            }
            PlanView::NewCabinet { .. } => item_contents.push(html!("Neuer Schacht")),
            PlanView::NewDuct { .. } => item_contents.push(html!("Neue Trasse")),
            PlanView::NewCabinetType { .. } => item_contents.push(html!("Neuer Schachttyp")),
            PlanView::CabinetType { id } => {
                item_contents.push(html!(<ListCabinetType {plan_id} typ_id={*id}/>))
            }
            PlanView::Duct { id, view } => {
                item_contents.push(html!(<ListDuct {plan_id} duct_id={*id} view={view.clone()}/>))
            }
            _ => {}
        }
    }
}

impl AppRoute {
    /// Renders the page in PatternFly's page layout: the breadcrumb in a `PageSection` inside
    /// `<main>`, followed by the page, which adds its title and content sections with
    /// `PageLayout`. There is no masthead or sidebar, and unlike PatternFly's `Page` the document
    /// scrolls instead of the main container (`.app-page` in `style.scss`). `RetryScope` lets
    /// an error build it all anew, as the installed app has no reload (`components/recovery.rs`).
    pub fn content(self) -> Html {
        let breadcrumb = self.breadcrumb();
        let role = self.required_role();
        let route = self.clone();
        let content = match self {
            AppRoute::ListOfPlans => html!(<ListOfPlannings/>),
            AppRoute::Plan { plan_id, view } => view.content(plan_id),
        };
        // After a failed request its error offers to build breadcrumb and page anew
        html! {
            <RetryScope>
                <div class="pf-v6-c-page pf-m-no-sidebar app-page">
                    <div class="pf-v6-c-page__main-container">
                        <main class="pf-v6-c-page__main" id="main-content" tabindex="-1">
                            <PageSection r#type={PageSectionType::Breadcrumbs}>
                                <div class="breadcrumb-bar">{breadcrumb}<NetboxHint {route}/><UserMenu/></div>
                            </PageSection>
                            <RequireRole {role}>{content}</RequireRole>
                        </main>
                    </div>
                </div>
            </RetryScope>
        }
    }
    /// Role needed for the page: the editors of a Schacht's panels and of a panel need
    /// `Planner`, all other pages can be read by everyone and hide the changes they offer.
    /// Menus leave out the pages the user may not open.
    pub fn required_role(&self) -> Role {
        match self {
            AppRoute::ListOfPlans => Role::Reader,
            AppRoute::Plan { view, .. } => view.required_role(),
        }
    }
    fn breadcrumb(&self) -> Html {
        let mut item_contents = Vec::new();
        item_contents.push(match self {
            AppRoute::ListOfPlans => {
                html!(<ListPlan/>)
            }
            AppRoute::Plan { plan_id, view } => {
                html!(<ListPlan {plan_id} view={view.clone()}/>)
            }
        });
        if let AppRoute::Plan { plan_id, view } = self {
            view.append_breadcrumbs(*plan_id, &mut item_contents);
        }
        let items = item_contents
            .into_iter()
            .map(|item_content| html_nested!(<BreadcrumbItem>{item_content}</BreadcrumbItem>));
        html! {
            <Breadcrumb>
                {for items}
            </Breadcrumb>
        }
    }
}

impl PlanView {
    /// See `AppRoute::required_role`.
    pub fn required_role(&self) -> Role {
        match self {
            PlanView::Cabinet {
                view: CabinetView::Edit,
                ..
            }
            | PlanView::NewCabinet { .. }
            | PlanView::NewDuct { .. }
            | PlanView::Panel {
                view: PanelView::Edit | PanelView::Loop | PanelView::Attach,
                ..
            } => Role::Planner,
            PlanView::NewCabinetType { .. } | PlanView::Leitungskataster | PlanView::Netbox => {
                Role::Admin
            }
            _ => Role::Reader,
        }
    }
    fn content(self, plan_id: i32) -> Html {
        match self {
            PlanView::Edit => html!(<EditPlan {plan_id}/>),
            PlanView::ListOfCabinets => html! {<ListOfCabinets {plan_id}/>},
            PlanView::NewCabinet { id } => {
                html!(<CabinetProperties {plan_id} cabinet={IdOrNew::Temporary(id)}/>)
            }
            PlanView::Cabinet { id, view } => view.content(plan_id, id),
            PlanView::ListOfCables => html! {<ListOfCables/>},
            PlanView::Cable { id, view } => view.content(plan_id, id),
            PlanView::Panel { id, view } => view.content(plan_id, id),
            PlanView::Map => html!(<Map {plan_id}/>),
            PlanView::ListOfDucts => html!(<ListOfDucts/>),
            PlanView::ListOfOwners => html!(<ListOfOwners/>),
            PlanView::ListOfCabinetTypes => html!(<ListOfCabinetTypes/>),
            PlanView::NewCabinetType { id } => {
                html!(<CabinetTypeProperties {plan_id} typ={IdOrNew::Temporary(id)}/>)
            }
            PlanView::CabinetType { id } => {
                html!(<CabinetTypeProperties {plan_id} typ={IdOrNew::Id(id)}/>)
            }
            PlanView::NewDuct { id } => {
                html!(<EditDuctProperties {plan_id} duct={IdOrNew::Temporary(id)}/>)
            }
            PlanView::Duct { id, view } => view.content(plan_id, id),
            PlanView::Leitungskataster => html!(<Leitungskataster {plan_id}/>),
            PlanView::Netbox => html!(<NetboxPage {plan_id}/>),
        }
    }
}

impl DuctView {
    fn content(self, plan_id: i32, duct_id: i32) -> Html {
        match self {
            DuctView::Show => html!(<ShowDuct {plan_id} {duct_id}/>),
            DuctView::Properties => {
                html!(<EditDuctProperties {plan_id} duct={IdOrNew::Id(duct_id)}/>)
            }
        }
    }

    /// The views in the breadcrumb menu; `Properties` opens from the overview's button
    pub const ALL: [DuctView; 1] = [DuctView::Show];

    pub fn title(&self) -> &'static str {
        match self {
            DuctView::Show => "Übersicht",
            DuctView::Properties => "Bearbeiten",
        }
    }
}

impl CableView {
    fn content(self, plan_id: i32, cable_id: i32) -> Html {
        match self {
            CableView::Edit => {
                html!(<EditCable {plan_id} {cable_id}/>)
            }
        }
    }
}

impl CabinetView {
    fn content(self, plan_id: i32, cabinet_id: i32) -> Html {
        match self {
            CabinetView::Overview => html!(<CabinetOverview {plan_id} {cabinet_id}/>),
            CabinetView::Properties => {
                html!(<CabinetProperties {plan_id} cabinet={IdOrNew::Id(cabinet_id)}/>)
            }
            CabinetView::Edit => html!(<EditCabinetPanels {plan_id} {cabinet_id}/>),
        }
    }

    /// The views in the breadcrumb menu; `Properties` opens from the overview's button
    pub const ALL: [CabinetView; 2] = [CabinetView::Overview, CabinetView::Edit];

    pub fn title(&self) -> &'static str {
        match self {
            CabinetView::Overview => "Übersicht",
            CabinetView::Properties => "Bearbeiten",
            CabinetView::Edit => "Panels bearbeiten",
        }
    }
}
