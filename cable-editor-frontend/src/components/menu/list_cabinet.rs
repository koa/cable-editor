use crate::components::load::Load;
use crate::components::menu::{
    BreadcrumbDivider, MenuDropdown, MenuEntry, MenuEntryGroup, view_load,
};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_schacht::{SchachtListEntry, fetch_schacht_list};
use crate::pages::router::{AppRoute, CabinetView, PanelView, PlanView};
use crate::util::get_credentials;
use std::borrow::Cow;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListCabinet {
    loaded_cabinets: Load<Box<[SchachtListEntry]>>,
}

#[derive(Debug)]
pub enum Msg {
    FetchCabinets,
    Error(FrontendError),
    UpdateCabinetList(Box<[SchachtListEntry]>),
}

#[derive(Properties, PartialEq)]
pub struct ListCabinetProps {
    pub plan_id: i32,
    pub cabinet_id: i32,
    /// The view of the Schacht shown, `None` below it (in one of its panels)
    #[prop_or_default]
    pub view: Option<CabinetView>,
}

impl Component for ListCabinet {
    type Message = Msg;
    type Properties = ListCabinetProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListCabinet {
            loaded_cabinets: Load::Pending,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchCabinets => {
                // Keeps the Schächte shown while loading again, so the menu doesn't flicker
                if let Load::Failed(_) = self.loaded_cabinets {
                    self.loaded_cabinets = Load::Pending;
                }
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(
                        fetch_schacht_list(credentials.as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::UpdateCabinetList),
                    );
                });
                false
            }
            Msg::Error(error) => {
                self.loaded_cabinets = Load::Failed(error);
                true
            }
            Msg::UpdateCabinetList(list) => {
                self.loaded_cabinets = Load::Loaded(list);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().cabinet_id != old_props.cabinet_id
            || ctx.props().plan_id != old_props.plan_id
        {
            ctx.link().send_message(Msg::FetchCabinets);
            false
        } else {
            ctx.props().view != old_props.view
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        view_load(&self.loaded_cabinets, |cabinets| {
            self.view_menus(ctx, cabinets)
        })
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchCabinets);
        }
    }
}

impl ListCabinet {
    /// The Schächte with this one selected and, on its own pages, the menu one level down: its
    /// views and root panels, the view shown selected. On a panel's pages that menu comes from
    /// `list_panel.rs`, with the root panel selected.
    fn view_menus(&self, ctx: &Context<Self>, cabinets: &[SchachtListEntry]) -> Html {
        let ListCabinetProps {
            plan_id,
            cabinet_id,
            ref view,
        } = *ctx.props();
        let cabinet = cabinets.iter().find(|c| c.id == cabinet_id);
        let title: Cow<'static, str> = cabinet
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Schacht {cabinet_id}"))
            .into();
        let mut cabinet_entries = cabinets
            .iter()
            .map(|c| MenuEntry {
                selected: c.id == cabinet_id,
                text: c.name.clone().into_boxed_str(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Cabinet {
                        id: c.id,
                        view: view.clone().unwrap_or(CabinetView::Overview),
                    },
                },
            })
            .collect::<Box<[_]>>();
        cabinet_entries.sort_by(|a, b| a.text.cmp(&b.text));
        let cabinet_menu = html!(<MenuDropdown {title} entries={cabinet_entries}/>);

        let Some(view) = view else {
            return cabinet_menu;
        };
        let panels = cabinet
            .map(|c| c.root_panels.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|p| MenuEntry {
                selected: false,
                text: p
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Panel {}", p.id))
                    .into_boxed_str(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Panel {
                        id: p.id,
                        view: PanelView::Show,
                    },
                },
            })
            .collect();
        let title: Cow<'static, str> = view.title().into();
        let entries = view_entries(plan_id, cabinet_id, Some(view));
        let groups: Box<[MenuEntryGroup]> = Box::new([MenuEntryGroup {
            title: "Panels",
            entries: panels,
        }]);
        html! {
            <span class="breadcrumb-path">
                {cabinet_menu}
                <BreadcrumbDivider/>
                <MenuDropdown {title} {entries} {groups}/>
            </span>
        }
    }
}

/// The views of a Schacht in its menu, `current` selected.
pub fn view_entries(
    plan_id: i32,
    cabinet_id: i32,
    current: Option<&CabinetView>,
) -> Box<[MenuEntry]> {
    CabinetView::ALL
        .iter()
        .map(|view| MenuEntry {
            selected: current == Some(view),
            text: view.title().into(),
            target: AppRoute::Plan {
                plan_id,
                view: PlanView::Cabinet {
                    id: cabinet_id,
                    view: view.clone(),
                },
            },
        })
        .collect()
}
