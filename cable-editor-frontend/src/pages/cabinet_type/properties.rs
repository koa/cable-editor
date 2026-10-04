//! Name, icon, Objektart and dimensions of a type of Schacht, or a new one
//! (`IdOrNew::Temporary`); changed by admins, as the type gives the Objektart of its Schächte in
//! the delivery to the Leitungskataster. Everyone else sees the page read-only.

use crate::components::load::Load;
use crate::components::select::Select;
use crate::{
    components::{
        dialog::confirm_delete,
        page_layout::{PageLayout, object_title},
        unsaved::Unsaved,
    },
    error::FrontendError,
    graphql::authenticated::{
        IdOrNew,
        current_user::Role,
        schacht_types::{
            LkmapPunktObjektart, SchachtTypEntry, SchachtTypInput, create_schacht_typ,
            delete_schacht_typ, fetch_schacht_typ, icon_src, update_schacht_typ,
        },
    },
    pages::router::PlanView,
    util::{get_credentials, get_role, navigate, toast_error, toast_success},
};
use cable_editor_common::ObjectKind;
use patternfly_yew::prelude::{
    ActionGroup, Button, ButtonVariant, Form, FormGroup, Icon, Spinner, TextInput, TextInputType,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;
use yew::{
    Component, Context, Html, NodeRef, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

pub struct CabinetTypeProperties {
    /// `None` while loading; the type as stored, missing for a new one
    loaded: Load<Option<SchachtTypEntry>>,
    name: String,
    objektart: LkmapPunktObjektart,
    /// As typed, empty: none
    dimension1: String,
    dimension2: String,
    /// A new icon from a file, not stored yet
    icon: Option<String>,
    saving: bool,
    file_input: NodeRef,
    /// A new type is only unsaved once something was entered
    touched: bool,
    unsaved: Unsaved,
}

pub enum Msg {
    Loaded(Result<Option<SchachtTypEntry>, FrontendError>),
    SetName(String),
    SetObjektart(Option<LkmapPunktObjektart>),
    SetDimension1(String),
    SetDimension2(String),
    ChooseIcon,
    IconChosen,
    IconRead(String, Result<String, String>),
    Save,
    /// The id of a new type
    Done(Result<Option<i32>, FrontendError>),
    Delete,
}

#[derive(Clone, PartialEq, Properties)]
pub struct CabinetTypePropertiesProps {
    pub plan_id: i32,
    /// A temporary id: create a new type
    pub typ: IdOrNew,
}

impl Component for CabinetTypeProperties {
    type Message = Msg;
    type Properties = CabinetTypePropertiesProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::fetch(ctx);
        Self {
            loaded: Load::Pending,
            name: String::new(),
            objektart: LkmapPunktObjektart::SchachtRund,
            dimension1: String::new(),
            dimension2: String::new(),
            icon: None,
            saving: false,
            file_input: NodeRef::default(),
            touched: false,
            unsaved: Unsaved::new(ctx.link()),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if matches!(
            msg,
            Msg::SetName(_)
                | Msg::SetObjektart(_)
                | Msg::SetDimension1(_)
                | Msg::SetDimension2(_)
                | Msg::IconRead(..)
        ) {
            self.touched = true;
        }
        match msg {
            Msg::Loaded(Ok(typ)) => {
                if let Some(typ) = &typ {
                    self.name = typ.name.clone().unwrap_or_default();
                    self.objektart = typ.lkmap_objektart;
                    self.dimension1 = typ.dimension1_mm.map(|d| d.to_string()).unwrap_or_default();
                    self.dimension2 = typ.dimension2_mm.map(|d| d.to_string()).unwrap_or_default();
                }
                self.icon = None;
                self.loaded = Load::Loaded(typ);
            }
            Msg::Loaded(Err(error)) => self.loaded = Load::Failed(error),
            Msg::SetName(name) => self.name = name,
            Msg::SetObjektart(objektart) => {
                if let Some(objektart) = objektart {
                    self.objektart = objektart;
                }
            }
            Msg::SetDimension1(value) => self.dimension1 = value,
            Msg::SetDimension2(value) => self.dimension2 = value,
            Msg::ChooseIcon => {
                if let Some(input) = self.file_input.cast::<HtmlInputElement>() {
                    input.click();
                }
                return false;
            }
            Msg::IconChosen => {
                self.read_icon(ctx);
                return false;
            }
            Msg::IconRead(_, Ok(svg)) => self.icon = Some(svg),
            Msg::IconRead(name, Err(error)) => {
                toast_error(
                    ctx.link(),
                    format!("{name} konnte nicht gelesen werden"),
                    error,
                );
                return false;
            }
            Msg::Save => self.save(ctx),
            Msg::Done(result) => {
                self.saving = false;
                match result {
                    Ok(Some(id)) => {
                        toast_success(ctx.link(), "Schachttyp angelegt");
                        navigate(
                            ctx.link(),
                            ctx.props().plan_id,
                            PlanView::CabinetType { id },
                        )
                    }
                    Ok(None) => {
                        toast_success(ctx.link(), "Schachttyp gespeichert");
                        Self::fetch(ctx);
                    }
                    Err(error) => toast_error(
                        ctx.link(),
                        "Schachttyp konnte nicht gespeichert werden",
                        error,
                    ),
                }
            }
            Msg::Delete => {
                if let IdOrNew::Id(id) = ctx.props().typ {
                    let scope = ctx.link().clone();
                    let credentials = get_credentials(&scope);
                    let plan_id = ctx.props().plan_id;
                    spawn_local(async move {
                        match delete_schacht_typ(credentials.as_ref(), id).await {
                            Ok(()) => {
                                toast_success(&scope, "Schachttyp gelöscht");
                                navigate(&scope, plan_id, PlanView::ListOfCabinetTypes)
                            }
                            Err(error) => toast_error(
                                &scope,
                                "Schachttyp konnte nicht gelöscht werden",
                                error,
                            ),
                        }
                    });
                }
                return false;
            }
        }
        true
    }

    fn rendered(&mut self, ctx: &Context<Self>, _first_render: bool) {
        self.unsaved.set(self.has_unsaved(ctx));
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        // A new type's temporary id comes from the route, so it's stable
        let other = ctx.props().typ != old_props.typ;
        if other {
            self.loaded = Load::Pending;
            self.icon = None;
            Self::fetch(ctx);
        }
        other
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let is_new = self.is_new(ctx);
        let title = if is_new {
            "Neuer Schachttyp".into()
        } else {
            let stored = self.stored().map(|typ| typ.title().to_string());
            object_title("Schachttyp", stored)
        };
        let content = self.loaded.view(|stored| {
            if stored.is_none()
                && let IdOrNew::Id(id) = ctx.props().typ
            {
                (&FrontendError::not_found(ObjectKind::SchachtTyp, id)).into_prop_value()
            } else {
                self.view_form(ctx)
            }
        });
        html! {
            <PageLayout {title}>
                <div class="cabinet-type__form">{content}</div>
            </PageLayout>
        }
    }
}

impl CabinetTypeProperties {
    fn fetch(ctx: &Context<Self>) {
        let IdOrNew::Id(id) = ctx.props().typ else {
            ctx.link().send_message(Msg::Loaded(Ok(None)));
            return;
        };
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            scope.send_message(Msg::Loaded(
                fetch_schacht_typ(credentials.as_ref(), id).await,
            ));
        });
    }

    fn is_new(&self, ctx: &Context<Self>) -> bool {
        matches!(ctx.props().typ, IdOrNew::Temporary(_))
    }

    fn can_edit(&self, ctx: &Context<Self>) -> bool {
        get_role(ctx.link()) >= Role::Admin
    }

    fn stored(&self) -> Option<&SchachtTypEntry> {
        self.loaded.loaded()?.as_ref()
    }

    /// The values to store; missing while a dimension isn't a number or the name is empty
    /// (the backend checks the rest).
    fn input(&self) -> Option<SchachtTypInput> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        Some(SchachtTypInput {
            name: name.to_string(),
            icon: self.icon.clone(),
            lkmap_objektart: self.objektart,
            dimension1_mm: parse_dimension(&self.dimension1)?,
            dimension2_mm: parse_dimension(&self.dimension2)?,
        })
    }

    fn has_unsaved(&self, ctx: &Context<Self>) -> bool {
        if self.is_new(ctx) {
            self.touched
        } else {
            self.has_changes(ctx)
        }
    }

    fn has_changes(&self, ctx: &Context<Self>) -> bool {
        let Some(input) = self.input() else {
            return false;
        };
        match self.stored() {
            None => self.is_new(ctx),
            Some(typ) => {
                input.icon.is_some()
                    || Some(input.name.as_str()) != typ.name.as_deref()
                    || input.lkmap_objektart != typ.lkmap_objektart
                    || input.dimension1_mm != typ.dimension1_mm
                    || input.dimension2_mm != typ.dimension2_mm
            }
        }
    }

    fn save(&mut self, ctx: &Context<Self>) {
        let Some(input) = self.input() else {
            return;
        };
        self.saving = true;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let typ = ctx.props().typ;
        spawn_local(async move {
            let result = match typ {
                IdOrNew::Id(id) => update_schacht_typ(credentials.as_ref(), id, input)
                    .await
                    .map(|()| None),
                IdOrNew::Temporary(_) => create_schacht_typ(credentials.as_ref(), input)
                    .await
                    .map(Some),
            };
            scope.send_message(Msg::Done(result));
        });
    }

    fn read_icon(&self, ctx: &Context<Self>) {
        let Some(input) = self.file_input.cast::<HtmlInputElement>() else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        // Choosing the same file again fires a change again
        input.set_value("");
        let scope = ctx.link().clone();
        spawn_local(async move {
            let name = file.name();
            let result = match JsFuture::from(file.text()).await {
                Ok(text) => Ok(text.as_string().unwrap_or_default()),
                Err(error) => Err(format!("{error:?}")),
            };
            scope.send_message(Msg::IconRead(name, result));
        });
    }

    fn view_form(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let readonly = !self.can_edit(ctx);
        let objektart = if readonly {
            html!(<TextInput value={self.objektart.title()} readonly=true/>)
        } else {
            let options = LkmapPunktObjektart::ALL
                .iter()
                .map(|objektart| (*objektart, objektart.title().to_string()))
                .collect::<Box<[_]>>();
            html! {
                <Select<LkmapPunktObjektart>
                    value={Some(self.objektart)}
                    onchange={link.callback(Msg::SetObjektart)}
                    {options}
                />
            }
        };
        let dimension = |value: &String, onchange| {
            html! {
                <TextInput
                    r#type={TextInputType::Number}
                    value={value.clone()}
                    {onchange}
                    placeholder="mm"
                    {readonly}
                />
            }
        };
        let schaechte = self.stored().map(|typ| match typ.schacht_count {
            0 => "Kein Schacht hat diesen Typ.".to_string(),
            1 => "1 Schacht hat diesen Typ.".to_string(),
            count => format!("{count} Schächte haben diesen Typ."),
        });
        html! {
            <Form>
                <FormGroup label="Name" required={!readonly}>
                    <TextInput
                        value={self.name.clone()}
                        onchange={link.callback(Msg::SetName)}
                        {readonly}
                    />
                </FormGroup>
                <FormGroup label="Icon">{self.view_icon(ctx)}</FormGroup>
                <FormGroup label="Objektart im Leitungskataster">{objektart}</FormGroup>
                <div class="cabinet-type__dimensions">
                    <FormGroup label="Dimension 1 (grösseres Innenmass, mm)">
                        {dimension(&self.dimension1, link.callback(Msg::SetDimension1))}
                    </FormGroup>
                    <FormGroup label="Dimension 2 (kleineres Innenmass, mm)">
                        {dimension(&self.dimension2, link.callback(Msg::SetDimension2))}
                    </FormGroup>
                </div>
                if let Some(schaechte) = schaechte {
                    <p class="cabinet-type__hint">{schaechte}</p>
                }
                if !readonly {
                    {self.view_actions(ctx)}
                }
            </Form>
        }
    }

    /// The icon as it will be stored and, for admins, a file to replace it.
    fn view_icon(&self, ctx: &Context<Self>) -> Html {
        let shown = self
            .icon
            .as_deref()
            .or(self.stored().map(|typ| typ.icon.as_str()));
        let preview = match shown {
            Some(svg) => html!(<img class="cabinet-type__preview" src={icon_src(svg)} alt="Icon"/>),
            None => html!(<p class="cabinet-type__hint">{"Ohne Datei ein einfacher Kreis"}</p>),
        };
        if !self.can_edit(ctx) {
            return preview;
        }
        let link = ctx.link();
        html! {
            <div class="cabinet-type__icon-choice">
                {preview}
                <input
                    type="file"
                    accept=".svg,image/svg+xml"
                    class="cabinet-type__file"
                    ref={self.file_input.clone()}
                    onchange={link.callback(|_| Msg::IconChosen)}
                />
                <Button
                    variant={ButtonVariant::Secondary}
                    icon={Icon::Upload}
                    label="SVG-Datei wählen"
                    onclick={link.callback(|_| Msg::ChooseIcon)}
                />
            </div>
        }
    }

    fn view_actions(&self, ctx: &Context<Self>) -> Html {
        if self.saving {
            return html!(<Spinner/>);
        }
        let is_new = self.is_new(ctx);
        let unused = self.stored().is_some_and(|typ| typ.schacht_count == 0);
        let name = self
            .stored()
            .and_then(|typ| typ.name.clone())
            .unwrap_or_default();
        let delete = (!is_new && unused).then(|| {
            let onclick = confirm_delete(ctx.link(), "Schachttyp", &name, ctx.link().callback(|()| Msg::Delete));
            html_nested!(<Button variant={ButtonVariant::DangerSecondary} label="Löschen" {onclick}/>)
        });
        html! {
            <ActionGroup>
                <Button
                    variant={ButtonVariant::Primary}
                    label={if is_new { "Anlegen" } else { "Speichern" }}
                    onclick={ctx.link().callback(|_| Msg::Save)}
                    disabled={!self.has_changes(ctx)}
                />
                {for delete}
            </ActionGroup>
        }
    }
}

/// Empty: `Some(None)`; not a whole number: `None`.
fn parse_dimension(value: &str) -> Option<Option<i32>> {
    let value = value.trim();
    if value.is_empty() {
        Some(None)
    } else {
        value.parse().ok().map(Some)
    }
}
