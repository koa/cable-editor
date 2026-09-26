use crate::{
    components::menu::popup::{MenuGroup, PopupMenu},
    components::page_layout::PageLayout,
    error::FrontendError,
    graphql::authenticated::current_user::{CurrentUser, Role},
    util::get_credentials,
};
use patternfly_yew::prelude::{Alert, AlertType, Bullseye, Icon, MenuToggleVariant, Spinner};
use yew::{
    Component, Context, ContextProvider, Html, Properties, function_component, html,
    html::IntoPropValue, platform::spawn_local, use_context,
};

#[derive(Properties, PartialEq)]
pub struct UserProviderProps {
    #[prop_or_default]
    pub children: Html,
}

/// Loads the logged in user and provides it to its children as `CurrentUser` context
/// (`UserMenu`) and its role as `Role` context (`util::get_role`, `RequireRole`), so pages can
/// hide what the user may not do.
pub struct UserProvider {
    user: Option<Result<CurrentUser, FrontendError>>,
}

pub enum UserProviderMsg {
    User(Result<CurrentUser, FrontendError>),
}

impl Component for UserProvider {
    type Message = UserProviderMsg;
    type Properties = UserProviderProps;

    fn create(ctx: &Context<Self>) -> Self {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            let user = CurrentUser::fetch(credentials.as_ref()).await;
            scope.send_message(UserProviderMsg::User(user));
        });
        Self { user: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            UserProviderMsg::User(user) => {
                self.user = Some(user);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match &self.user {
            None => html!(<Bullseye><Spinner/></Bullseye>),
            Some(Err(error)) => {
                let error: Html = error.into_prop_value();
                html!(<Bullseye>{error}</Bullseye>)
            }
            Some(Ok(user)) => html! {
                <ContextProvider<CurrentUser> context={user.clone()}>
                    <ContextProvider<Role> context={user.role}>
                        {ctx.props().children.clone()}
                    </ContextProvider<Role>>
                </ContextProvider<CurrentUser>>
            },
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct RequireRoleProps {
    pub role: Role,
    #[prop_or_default]
    pub children: Html,
}

/// Its children if the user has at least `role`, else a page saying so (for pages reached by
/// URL although no link leads there).
#[function_component]
pub fn RequireRole(props: &RequireRoleProps) -> Html {
    let role = use_context::<Role>().unwrap_or(Role::Reader);
    if role >= props.role {
        props.children.clone()
    } else {
        html! {
            <PageLayout title="Keine Berechtigung">
                <Alert inline=true r#type={AlertType::Info} title="Für diese Seite fehlt die Berechtigung"/>
            </PageLayout>
        }
    }
}

/// Menu at the end of the breadcrumb bar showing who is logged in, the role and the groups it
/// is derived from, for support ("why can't I change this?").
#[function_component]
pub fn UserMenu() -> Html {
    let Some(user) = use_context::<CurrentUser>() else {
        return Html::default();
    };
    let name = if user.display_name.is_empty() {
        user.preferred_username.clone()
    } else {
        user.display_name.clone()
    };
    let groups = if user.groups.is_empty() {
        "keine".to_string()
    } else {
        user.groups.join(", ")
    };
    let entry = |title: &str, text: String| {
        html! {
            <li class="pf-v6-c-menu__list-item">
                <div class="pf-v6-c-menu__item user-menu__entry">
                    <span class="pf-v6-c-menu__item-main">
                        <span class="pf-v6-c-menu__item-text">{text}</span>
                    </span>
                    <span class="pf-v6-c-menu__item-description">{title}</span>
                </div>
            </li>
        }
    };
    let text = html!(<span class="user-menu__name">{name}</span>);
    html! {
        <PopupMenu
            variant={MenuToggleVariant::Plain}
            icon={html!({Icon::User})}
            {text}
            aria_label="Benutzer"
            align_end=true
        >
            <MenuGroup title="Angemeldet">
                {entry("Benutzername", user.preferred_username.clone())}
                {entry(user.role.description(), format!("Rolle: {}", user.role.title()))}
                {entry("Gruppen beim Login-Anbieter", groups)}
            </MenuGroup>
        </PopupMenu>
    }
}
