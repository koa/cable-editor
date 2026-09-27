use crate::components::menu::{MenuDropdown, MenuEntry};
use crate::error::FrontendError;
use crate::graphql::authenticated::schacht_types::{SchachtTypEntry, fetch_schacht_typ_list};
use crate::pages::router::{AppRoute, PlanView};
use crate::util::get_credentials;
use patternfly_yew::prelude::Spinner;
use std::borrow::Cow;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

/// The type of Schacht of the page and the other types (like `ListDuct`, without views).
pub struct ListCabinetType {
    /// `None` while loading
    types: Option<Result<Vec<SchachtTypEntry>, FrontendError>>,
}

pub enum Msg {
    Loaded(Result<Vec<SchachtTypEntry>, FrontendError>),
}

#[derive(Properties, PartialEq)]
pub struct ListCabinetTypeProps {
    pub plan_id: i32,
    pub typ_id: i32,
}

impl Component for ListCabinetType {
    type Message = Msg;
    type Properties = ListCabinetTypeProps;

    fn create(ctx: &Context<Self>) -> Self {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            scope.send_message(Msg::Loaded(
                fetch_schacht_typ_list(credentials.as_ref()).await,
            ));
        });
        Self { types: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(types) => self.types = Some(types),
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let types = match &self.types {
            None => return html!(<Spinner/>),
            Some(Err(error)) => return error.into_prop_value(),
            Some(Ok(types)) => types,
        };
        let ListCabinetTypeProps { plan_id, typ_id } = *ctx.props();
        let title: Cow<'static, str> = types
            .iter()
            .find(|typ| typ.id == typ_id)
            .map(|typ| Cow::Owned(typ.title().to_string()))
            .unwrap_or(Cow::Borrowed(" - "));
        let entries = types
            .iter()
            .map(|typ| MenuEntry {
                selected: typ.id == typ_id,
                text: typ.title().into(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::CabinetType { id: typ.id },
                },
            })
            .collect::<Vec<_>>();
        html!(<MenuDropdown {title} {entries}/>)
    }
}
