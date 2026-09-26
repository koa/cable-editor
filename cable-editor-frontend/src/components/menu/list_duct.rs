use crate::components::menu::{BreadcrumbDivider, MenuDropdown, MenuEntry, MenuEntryGroup};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_ducts::{DuctListEntry, fetch_duct_list};
use crate::pages::router::{AppRoute, DuctView, PlanView};
use crate::util::get_credentials;
use patternfly_yew::prelude::Spinner;
use std::borrow::Cow;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

/// The duct of the page (its views, the other ducts) and, on its own pages, the view shown
/// (like `ListCabinet`).
pub struct ListDuct {
    loaded_ducts: Option<Box<[DuctListEntry]>>,
    error: Option<FrontendError>,
}

#[derive(Debug)]
pub enum Msg {
    FetchDucts,
    Error(FrontendError),
    UpdateDuctList(Box<[DuctListEntry]>),
}

#[derive(Properties, PartialEq)]
pub struct ListDuctProps {
    pub plan_id: i32,
    #[prop_or_default]
    pub duct_id: Option<i32>,
    #[prop_or_default]
    pub view: Option<DuctView>,
}

impl Component for ListDuct {
    type Message = Msg;
    type Properties = ListDuctProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListDuct {
            loaded_ducts: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchDucts => {
                self.error = None;
                self.loaded_ducts = None;
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(
                        fetch_duct_list(credentials.as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::UpdateDuctList),
                    );
                });
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::UpdateDuctList(list) => {
                self.error = None;
                self.loaded_ducts = Some(list);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.loaded_ducts {
                None => {
                    html!(<Spinner/>)
                }
                Some(ducts) => self.view_menus(ctx, ducts),
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchDucts);
        }
    }
}

impl ListDuct {
    fn view_menus(&self, ctx: &Context<Self>, ducts: &[DuctListEntry]) -> Html {
        let ListDuctProps {
            plan_id,
            duct_id,
            ref view,
        } = *ctx.props();
        let title: Cow<'static, str> = duct_id
            .and_then(|id| ducts.iter().find(|duct| duct.id == id))
            .map(|duct| Cow::Owned(duct.title()))
            .unwrap_or(Cow::Borrowed(" - "));
        let view_entries = duct_id
            .map(|id| {
                DuctView::ALL
                    .iter()
                    .map(|view| MenuEntry {
                        selected: false,
                        text: view.title().into(),
                        target: AppRoute::Plan {
                            plan_id,
                            view: PlanView::Duct {
                                id,
                                view: view.clone(),
                            },
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut duct_entries = ducts
            .iter()
            .map(|duct| MenuEntry {
                selected: Some(duct.id) == duct_id,
                text: duct.title().into_boxed_str(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Duct {
                        id: duct.id,
                        view: view.clone().unwrap_or(DuctView::Show),
                    },
                },
            })
            .collect::<Vec<_>>();
        duct_entries.sort_by(|a, b| a.text.cmp(&b.text));
        let groups = vec![MenuEntryGroup {
            title: "Trassen",
            entries: duct_entries,
        }];
        let duct_menu = html!(<MenuDropdown {title} entries={view_entries.clone()} {groups}/>);
        let Some(view) = view else {
            return duct_menu;
        };
        let title: Cow<'static, str> = view.title().into();
        html! {
            <span class="breadcrumb-path">
                {duct_menu}
                <BreadcrumbDivider/>
                <MenuDropdown {title} entries={view_entries}/>
            </span>
        }
    }
}
