use crate::graphql::anonymous::{AuthenticationData, AuthenticationQuery};
use crate::graphql::query_anonymous;
use cynic::{QueryBuilder, http::ReqwestExt};
use log::error;
use yew::platform::spawn_local;
use yew::{Context, Html, Properties, function_component, html};
use yew_oauth2::oauth2::Config;

#[derive(Debug)]
pub struct App {
    oauth2_config: Option<Config>,
}
#[derive(Debug)]
pub enum AppMessage {
    AuthenticationData(Config),
}
impl yew::Component for App {
    type Message = AppMessage;
    type Properties = ();
    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            oauth2_config: None,
        }
    }
    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMessage::AuthenticationData(config) => {
                self.oauth2_config = Some(config);
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        if let Some(config) = self.oauth2_config.clone() {
            html! {
                <MainOAuth2 {config}/>
            }
        } else {
            html! {
                <h1>{"Fetching"}</h1>
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            spawn_local(async move {
                let operation = AuthenticationQuery::build(());
                let result = query_anonymous::<AuthenticationQuery, _, _>(()).await;

                /*let result =
                    query_anonymous::<Settings, _>(scope.clone(), settings::Variables {}).await;
                match result {
                    Ok(ResponseData {
                        authentication:
                            SettingsAuthentication {
                                auth_url,
                                client_id,
                                token_url,
                            },
                    }) => {
                        scope.send_message(AppMessage::AuthenticationData(Config::new(
                            client_id, auth_url, token_url,
                        )));
                    }
                    Err(err) => error!("Error on server {err:?}"),
                }*/
            });
        }
    }
}
#[derive(Properties, Clone, PartialEq, Debug)]
pub struct MainOAuth2Props {
    config: Config,
}
#[function_component(MainOAuth2)]
pub fn main_oauth2(props: &MainOAuth2Props) -> Html {
    html! {
        <p>{"Dummy"}</p>
    }
}
