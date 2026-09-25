use crate::components::menu::list_cabinet::ListCabinet;
use crate::components::menu::{BreadcrumbDivider, MenuDropdown, MenuEntry, MenuEntryGroup};
use crate::error::FrontendError;
use crate::graphql::authenticated::panel_navigation::{ChildPanelNav, PanelHierarchy};
use crate::pages::router::{AppRoute, PanelView, PlanView};
use crate::util::get_credentials;
use patternfly_yew::prelude::Spinner;
use std::borrow::Cow;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListPanel {
    loaded_panel: Option<PanelHierarchy>,
    error: Option<FrontendError>,
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
            loaded_panel: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchPanel => {
                self.error = None;
                // Absichtlich kein self.loaded_panel = None, um Flackern beim Wechseln zu vermeiden
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
                self.error = Some(error);
                true
            }
            Msg::UpdatePanel(panel) => {
                self.error = None;
                self.loaded_panel = Some(panel);
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

        if let Some(error) = &self.error {
            html!(<span>{divider} {<&FrontendError as IntoPropValue<Html>>::into_prop_value(error)}</span>)
        } else {
            match &self.loaded_panel {
                None => html!(<span>{divider} <Spinner/></span>),
                Some(panel) => {
                    let plan_id = ctx.props().plan_id;
                    let current_view = &ctx.props().view;
                    let mut elements = Vec::new();

                    // 0. Schacht, zu dem das Panel gehört
                    elements.push(html!(<ListCabinet {plan_id} cabinet_id={panel.schacht.id}/>));

                    // 1. Parent Chain durchgehen:
                    for parent in &panel.parent_chain {
                        let p_id = parent.id;
                        let p_name = parent
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Panel {}", p_id));
                        if !elements.is_empty() {
                            elements.push(divider.clone());
                        }
                        elements.push(self.render_panel_dropdown(
                            plan_id,
                            p_id,
                            p_name,
                            "Panels",
                            &with_self(&parent.siblings, p_id, &parent.name, parent.parent_order),
                        ));
                    }

                    // 2. Aktuelles Panel (Mit Child-Panels für Vorwärtsnavigation)
                    let c_id = panel.id;
                    let c_name = panel
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Panel {}", c_id));
                    if !elements.is_empty() {
                        elements.push(divider.clone());
                    }
                    elements.push(self.render_panel_dropdown(
                        plan_id,
                        c_id,
                        c_name,
                        "Panels",
                        &with_self(&panel.siblings, c_id, &panel.name, panel.parent_order),
                    ));

                    // 3. View Dropdown
                    let view_title: Cow<'static, str> = match current_view {
                        PanelView::Show => "Übersicht",
                        PanelView::Edit => "Ports ändern",
                        PanelView::Attach => "Fasern auflegen",
                        PanelView::Loop => "Loops verbinden",
                    }
                    .into();

                    elements.push(divider.clone());
                    elements.push(self.render_panel_dropdown(
                        plan_id,
                        c_id,
                        view_title.into(),
                        "Unterpanels",
                        &panel.children,
                    ));

                    html! {
                        <span class="breadcrumb-path">
                            { for elements }
                        </span>
                    }
                }
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchPanel);
        }
    }
}

impl ListPanel {
    /// Menu with the views of the panel and, in a group titled `others_title`, the panels at its
    /// level including itself, marked (on a panel), or its children (on the view).
    fn render_panel_dropdown(
        &self,
        plan_id: i32,
        panel_id: i32,
        name: String,
        others_title: &'static str,
        others: &[ChildPanelNav],
    ) -> Html {
        let title: Cow<'static, str> = name.into();
        let mut entries = vec![
            MenuEntry {
                selected: false,
                text: "Übersicht".into(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Panel {
                        id: panel_id,
                        view: PanelView::Show,
                    },
                },
            },
            MenuEntry {
                selected: false,
                text: "Ports ändern".into(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Panel {
                        id: panel_id,
                        view: PanelView::Edit,
                    },
                },
            },
        ];
        if plan_id != 0 {
            entries.extend([
                MenuEntry {
                    selected: false,
                    text: "Fasern auflegen".into(),
                    target: AppRoute::Plan {
                        plan_id,
                        view: PlanView::Panel {
                            id: panel_id,
                            view: PanelView::Attach,
                        },
                    },
                },
                MenuEntry {
                    selected: false,
                    text: "Loops verbinden".into(),
                    target: AppRoute::Plan {
                        plan_id,
                        view: PlanView::Panel {
                            id: panel_id,
                            view: PanelView::Loop,
                        },
                    },
                },
            ]);
        }

        let others = others
            .iter()
            .map(|other| MenuEntry {
                selected: other.id == panel_id,
                text: other
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Panel {}", other.id))
                    .into_boxed_str(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Panel {
                        id: other.id,
                        view: PanelView::Show,
                    },
                },
            })
            .collect();
        let groups = vec![MenuEntryGroup {
            title: others_title,
            entries: others,
        }];
        html!(<MenuDropdown {title} {entries} {groups}/>)
    }
}

/// The siblings of a panel (which the backend returns without it) plus the panel, in panel order.
fn with_self(
    siblings: &[ChildPanelNav],
    id: i32,
    name: &Option<String>,
    parent_order: Option<i32>,
) -> Vec<ChildPanelNav> {
    let mut panels = siblings.to_vec();
    panels.push(ChildPanelNav {
        id,
        name: name.clone(),
        parent_order,
    });
    panels.sort_by_key(|panel| panel.parent_order);
    panels
}
