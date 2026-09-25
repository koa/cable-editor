use crate::components::menu::{MenuDropdown, MenuEntry};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_schacht::{SchachtListEntry, fetch_schacht_list};
use crate::pages::router::{AppRoute, CabinetView, PlanView};
use crate::util::get_credentials;
use patternfly_yew::prelude::Spinner;
use std::borrow::Cow;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListCabinet {
    loaded_cabinets: Option<Box<[SchachtListEntry]>>,
    error: Option<FrontendError>,
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
    pub view: CabinetView,
}

impl Component for ListCabinet {
    type Message = Msg;
    type Properties = ListCabinetProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListCabinet {
            loaded_cabinets: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchCabinets => {
                self.error = None;
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
                self.error = Some(error);
                true
            }
            Msg::UpdateCabinetList(list) => {
                self.error = None;
                self.loaded_cabinets = Some(list);
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
        } else if ctx.props().view != old_props.view {
            true
        } else {
            false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.loaded_cabinets {
                None => html!(<Spinner/>),
                Some(cabinets) => {
                    let plan_id = ctx.props().plan_id;
                    let cabinet_id = ctx.props().cabinet_id;
                    let view = &ctx.props().view;

                    let title = cabinets
                        .iter()
                        .find(|c| c.id == cabinet_id)
                        .map(|c| Cow::Owned(c.name.clone()))
                        .unwrap_or_else(|| Cow::Owned(format!("Schacht {}", cabinet_id)));

                    let mut entries = cabinets
                        .iter()
                        .map(|c| MenuEntry {
                            text: c.name.clone().into_boxed_str(),
                            target: AppRoute::Plan {
                                plan_id,
                                view: PlanView::Cabinet {
                                    id: c.id,
                                    view: view.clone(),
                                },
                            },
                        })
                        .collect::<Vec<_>>();

                    entries.sort_by(|a, b| a.text.cmp(&b.text));

                    html!(<MenuDropdown {title} {entries}/>)
                }
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchCabinets);
        }
    }
}
