use crate::components::load::Load;
use crate::components::menu::list_cabinet::{ListCabinet, view_entries};
use crate::components::menu::{
    BreadcrumbDivider, MenuDropdown, MenuEntry, MenuEntryGroup, view_load,
};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_plans::BASELINE_PLAN_ID;
use crate::graphql::authenticated::panel_navigation::{ChildPanelNav, PanelHierarchy};
use crate::pages::router::{AppRoute, PanelView, PlanView};
use crate::util::get_credentials;
use std::borrow::Cow;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListPanel {
    loaded_panel: Load<PanelHierarchy>,
}

#[derive(Debug)]
pub enum Msg {
    FetchPanel,
    Error(FrontendError),
    UpdatePanel(PanelHierarchy),
}

#[derive(Properties, PartialEq)]
pub struct ListPanelProps {
    pub plan_id: i32,
    pub panel_id: i32,
    pub view: PanelView,
}

impl Component for ListPanel {
    type Message = Msg;
    type Properties = ListPanelProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListPanel {
            loaded_panel: Load::Pending,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchPanel => {
                // Keeps the panel shown while loading another, so the menu doesn't flicker
                if let Load::Failed(_) = self.loaded_panel {
                    self.loaded_panel = Load::Pending;
                }
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                let panel_id = ctx.props().panel_id;

                spawn_local(async move {
                    scope.send_message(
                        PanelHierarchy::fetch(credentials.as_ref(), panel_id)
                            .await
                            .map_or_else(Msg::Error, Msg::UpdatePanel),
                    );
                });
                false
            }
            Msg::Error(error) => {
                self.loaded_panel = Load::Failed(error);
                true
            }
            Msg::UpdatePanel(panel) => {
                self.loaded_panel = Load::Loaded(panel);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().panel_id != old_props.panel_id || ctx.props().plan_id != old_props.plan_id {
            ctx.link().send_message(Msg::FetchPanel);
            false
        } else {
            ctx.props().view != old_props.view
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let divider = html!(<BreadcrumbDivider/>);

        view_load(&self.loaded_panel, |panel| {
            let plan_id = ctx.props().plan_id;
            let current_view = &ctx.props().view;
            let mut elements = Vec::new();

            // The Schächte
            elements.push(html!(<ListCabinet {plan_id} cabinet_id={panel.schacht.id}/>));

            // One menu per level, offering what lies below the level above (the
            // Schacht's views and root panels, then a panel's views and children) with
            // the way to the current page selected
            let levels = panel
                .parent_chain
                .iter()
                .map(|p| (p.id, &p.name, p.parent_order, &p.siblings))
                .chain([(panel.id, &panel.name, panel.parent_order, &panel.siblings)]);
            let mut views = view_entries(plan_id, panel.schacht.id, None);
            let mut group_title = "Panels";
            for (id, name, parent_order, siblings) in levels {
                elements.push(divider.clone());
                elements.push(panel_menu(
                    plan_id,
                    panel_name(id, name),
                    views,
                    group_title,
                    &with_self(siblings, id, name, parent_order),
                    Some(id),
                ));
                views = panel_view_entries(plan_id, id, None);
                group_title = "Unterpanels";
            }

            // The current panel's views and children
            elements.push(divider.clone());
            elements.push(panel_menu(
                plan_id,
                current_view.title().into(),
                panel_view_entries(plan_id, panel.id, Some(current_view)),
                group_title,
                &panel.children,
                None,
            ));

            html! {
                <span class="breadcrumb-path">
                    { for elements }
                </span>
            }
        })
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchPanel);
        }
    }
}

/// Menu with `views` and, in a group titled `panels_title`, `panels`; `selected` is the panel
/// the current page lies in (none: a view is selected).
fn panel_menu(
    plan_id: i32,
    title: Cow<'static, str>,
    entries: Box<[MenuEntry]>,
    panels_title: &'static str,
    panels: &[ChildPanelNav],
    selected: Option<i32>,
) -> Html {
    let panels = panels
        .iter()
        .map(|panel| MenuEntry {
            selected: Some(panel.id) == selected,
            text: panel_name(panel.id, &panel.name).into_owned().into(),
            target: AppRoute::Plan {
                plan_id,
                view: PlanView::Panel {
                    id: panel.id,
                    view: PanelView::Show,
                },
            },
        })
        .collect();
    let groups: Box<[MenuEntryGroup]> = Box::new([MenuEntryGroup {
        title: panels_title,
        entries: panels,
    }]);
    html!(<MenuDropdown {title} {entries} {groups}/>)
}

fn panel_name(id: i32, name: &Option<String>) -> Cow<'static, str> {
    match name {
        Some(name) => Cow::Owned(name.clone()),
        None => Cow::Owned(format!("Panel {id}")),
    }
}

/// The views of a panel in its menu, `current` selected; planned changes only exist off the
/// baseline.
fn panel_view_entries(
    plan_id: i32,
    panel_id: i32,
    current: Option<&PanelView>,
) -> Box<[MenuEntry]> {
    let planned = plan_id != BASELINE_PLAN_ID;
    [
        (PanelView::Show, true),
        (PanelView::Edit, true),
        (PanelView::Attach, planned),
        (PanelView::Loop, planned),
    ]
    .into_iter()
    .filter(|(_, available)| *available)
    .map(|(view, _)| MenuEntry {
        selected: current == Some(&view),
        text: view.title().into(),
        target: AppRoute::Plan {
            plan_id,
            view: PlanView::Panel { id: panel_id, view },
        },
    })
    .collect()
}

/// The siblings of a panel (which the backend returns without it) plus the panel, in panel order.
fn with_self(
    siblings: &[ChildPanelNav],
    id: i32,
    name: &Option<String>,
    parent_order: Option<i32>,
) -> Box<[ChildPanelNav]> {
    let mut panels = siblings.to_vec();
    panels.push(ChildPanelNav {
        id,
        name: name.clone(),
        parent_order,
    });
    panels.sort_by_key(|panel| panel.parent_order);
    panels.into_boxed_slice()
}
