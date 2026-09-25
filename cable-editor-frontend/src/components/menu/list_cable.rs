use crate::components::menu::{MenuDropdown, MenuEntry};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_cables::{CableListEntry, fetch_cables_list};
use crate::pages::router::{AppRoute, CableView, PlanView};
use crate::util::get_credentials;
use patternfly_yew::prelude::Spinner;
use std::borrow::Cow;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListCable {
    loaded_cables: Option<Box<[CableListEntry]>>,
    error: Option<FrontendError>,
}

#[derive(Debug)]
pub enum Msg {
    FetchCables,
    Error(FrontendError),
    UpdateCableList(Box<[CableListEntry]>),
}

#[derive(Properties, PartialEq)]
pub struct ListPlanProps {
    pub plan_id: i32,
    #[prop_or_default]
    pub cable_id: Option<i32>,
    #[prop_or_default]
    pub view: Option<CableView>,
}

impl Component for ListCable {
    type Message = Msg;
    type Properties = ListPlanProps;

    fn create(ctx: &Context<Self>) -> Self {
        ListCable {
            loaded_cables: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchCables => {
                self.error = None;
                self.loaded_cables = None;
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(
                        fetch_cables_list(credentials.as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::UpdateCableList),
                    );
                });
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::UpdateCableList(list) => {
                self.error = None;
                self.loaded_cables = Some(list);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.loaded_cables {
                None => {
                    html!(<Spinner/>)
                }
                Some(cables) => {
                    let plan_id = ctx.props().plan_id;
                    let title = ctx
                        .props()
                        .cable_id
                        .and_then(|cid| cables.iter().find(|cables| cables.id == cid))
                        .map(|cable| Cow::Owned(cable.name.clone()))
                        .unwrap_or(Cow::Borrowed(" - "));
                    let view = ctx.props().view.as_ref().unwrap_or(&CableView::Edit);
                    let entries = cables
                        .iter()
                        .map(|e| MenuEntry {
                            text: e.name.clone().into_boxed_str(),
                            target: AppRoute::Plan {
                                plan_id,
                                view: PlanView::Cable {
                                    id: e.id,
                                    view: view.clone(),
                                },
                            },
                        })
                        .collect::<Vec<_>>();
                    html! {
                        <MenuDropdown {title} {entries}/>
                    }
                }
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchCables);
        }
    }
}
