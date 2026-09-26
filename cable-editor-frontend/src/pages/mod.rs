pub mod cabinet;
pub mod cable;
pub mod duct;
pub mod list_of_cables;
pub mod map;
pub mod panel;
pub mod planning;
pub mod router;

use crate::components::{label_printer::PrinterStatusBar, user::UserProvider};
use crate::{
    error::FrontendError,
    graphql::{
        anonymous::{AuthenticationData, AuthenticationQuery},
        query_anonymous,
    },
    pages::router::{AppRoute, RedirectToPlans},
};
use brady_web_sdk::BradyProvider;
use patternfly_yew::prelude::{
    Alert, AlertType, BackdropViewer, Bullseye, Button, ButtonVariant, Spinner, ToastViewer,
};
use yew::{
    Callback, Component, Context, ContextHandle, Html, Properties, function_component, html,
    html::IntoPropValue, platform::spawn_local,
};
use yew_nested_router::{Router, Switch};
use yew_oauth2::{
    agent::OAuth2Operations,
    components::context::Agent,
    context::OAuth2Context,
    openid::{Client, OAuth2},
    prelude::Authenticated,
};

#[derive(Debug)]
pub struct App {
    oauth2_config: Option<AuthenticationData>,
    error: Option<FrontendError>,
}
#[derive(Debug)]
pub enum AppMessage {
    AuthenticationData(AuthenticationData),
    Error(FrontendError),
}

impl yew::Component for App {
    type Message = AppMessage;
    type Properties = ();
    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            oauth2_config: None,
            error: None,
        }
    }
    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMessage::AuthenticationData(config) => {
                self.oauth2_config = Some(config);
                true
            }
            AppMessage::Error(e) => {
                self.error = Some(e);
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            let error: Html = error.into_prop_value();
            html!(<Bullseye>{error}</Bullseye>)
        } else if let Some(config) = self.oauth2_config.clone() {
            html! {
                <MainOAuth2 {config}/>
            }
        } else {
            html! {
                <Bullseye><Spinner/></Bullseye>
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            spawn_local(async move {
                scope.send_message(match query_anonymous::<AuthenticationQuery, _>(()).await {
                    Ok(AuthenticationQuery { authentication }) => {
                        AppMessage::AuthenticationData(authentication)
                    }
                    Err(error) => AppMessage::Error(error),
                });
            });
        }
    }
}
#[derive(Properties, Clone, PartialEq, Debug)]
pub struct MainOAuth2Props {
    config: AuthenticationData,
}
#[function_component(MainOAuth2)]
pub fn main_oauth2(props: &MainOAuth2Props) -> Html {
    let oauth2_config = &props.config;
    let scopes = oauth2_config.scopes.clone();
    html! {
     <OAuth2 config={oauth2_config.clone()} {scopes}>
        // Above BackdropViewer, so dialogs can follow the printer status
        <BradyProvider>
            <BackdropViewer>
                <ToastViewer>
                    <Authenticated>
                        <UserProvider>
                            <Router<AppRoute>>
                                <Switch<AppRoute> render={AppRoute::content} default={html!(<RedirectToPlans/>)}/>
                            </Router<AppRoute>>
                        </UserProvider>
                        <PrinterStatusBar/>
                    </Authenticated>
                    <Login/>
                </ToastViewer>
            </BackdropViewer>
        </BradyProvider>
      </OAuth2>
    }
}

/// Starts the login when there is no session. A failed login (e.g. the provider refused the
/// user, whose groups aren't allowed for this app) is shown with a button to try again instead
/// of starting the next login right away, which would redirect forever.
pub struct Login {
    auth: Option<OAuth2Context>,
    _auth_handle: Option<ContextHandle<OAuth2Context>>,
    /// Starting the login failed, e.g. storing its state
    start_error: Option<String>,
}

pub enum LoginMsg {
    Auth(OAuth2Context),
    Start,
}

impl Component for Login {
    type Message = LoginMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (auth, auth_handle) = ctx
            .link()
            .context::<OAuth2Context>(ctx.link().callback(LoginMsg::Auth))
            .unzip();
        let login = Self {
            auth,
            _auth_handle: auth_handle,
            start_error: None,
        };
        login.start_if_needed(ctx);
        login
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LoginMsg::Auth(auth) => {
                self.auth = Some(auth);
                self.start_if_needed(ctx);
            }
            LoginMsg::Start => {
                self.start_error = ctx
                    .link()
                    .context::<Agent<Client>>(Callback::noop())
                    .and_then(|(agent, _)| agent.start_login().err())
                    .map(|error| error.to_string());
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let failure = match (&self.start_error, &self.auth) {
            (Some(error), _) => error.clone(),
            (None, Some(OAuth2Context::Failed(error))) => error.clone(),
            (None, Some(OAuth2Context::NotAuthenticated { .. })) => {
                return html!(<Bullseye><Spinner/></Bullseye>);
            }
            _ => return Html::default(),
        };
        // The provider only reports a code, e.g. `login result: access_denied`
        let title = if failure.contains("access_denied") {
            "Kein Zugriff auf den Cable Editor".to_string()
        } else {
            format!("Anmeldung fehlgeschlagen: {failure}")
        };
        let hint = failure.contains("access_denied").then(|| {
            html! {
                <p>{"Das Konto ist beim Login-Anbieter nicht für diese Anwendung freigegeben. \
                    Bitte beim Support die Freigabe beantragen."}</p>
            }
        });
        html! {
            <Bullseye>
                <Alert inline=true title={title} r#type={AlertType::Danger}>
                    {hint}
                    <Button
                        variant={ButtonVariant::Secondary}
                        label="Erneut anmelden"
                        onclick={ctx.link().callback(|_| LoginMsg::Start)}
                    />
                </Alert>
            </Bullseye>
        }
    }
}

impl Login {
    /// Only without a session: after a failure the user decides when to try again.
    fn start_if_needed(&self, ctx: &Context<Self>) {
        if matches!(self.auth, Some(OAuth2Context::NotAuthenticated { .. })) {
            ctx.link().send_message(LoginMsg::Start);
        }
    }
}
