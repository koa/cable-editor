use crate::{
    components::{
        dialog::DeleteConfirmationDialog,
        page_layout::{PageLayout, object_title},
    },
    error::FrontendError,
    geo::{
        geo_file::{GeoFile, read_geo_file},
        map::{create_map, duct_line, fit_points, schacht_marker},
    },
    graphql::authenticated::{
        GeoPoint, IdOrNew,
        current_user::Role,
        duct_properties::{
            CoordinateSystem, DuctInput, DuctLineCheck, DuctProperties, LineInput, SchachtChoice,
            check_duct_line, create_duct, delete_duct, fetch_duct_properties, set_duct_line,
            update_duct,
        },
        list_ducts::duct_title,
    },
    pages::router::{AppRoute, DuctView, PlanView},
    util::{get_backdrop, get_credentials, get_role, get_toaster},
};
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Backdrop, Button, ButtonVariant, Checkbox, CheckboxState, Form,
    FormGroup, FormSelect, FormSelectOption, Icon, Spinner, TextInput, Toast, ToggleGroup,
    ToggleGroupItem,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::{
    Callback, Component, Context, Html, NodeRef, Properties, html, html::IntoPropValue,
    html_nested, platform::spawn_local,
};
use yew_nested_router::prelude::RouterContext;

/// Schächte, description and course of a duct, or a new one (`IdOrNew::Temporary`). The
/// course comes from a GeoJSON or GPX file: the backend converts it, turns it to run from
/// Schacht A to Z and leaves out ends repeating the Schächte (`checkDuctLine`), shown as
/// preview before it is stored. Readers see the page read-only.
pub struct EditDuctProperties {
    /// The duct as stored (missing for a new one) and the Schächte to choose from
    loaded: Option<(Option<DuctProperties>, Vec<SchachtChoice>)>,
    error: Option<FrontendError>,
    schacht_a: Option<i32>,
    schacht_z: Option<i32>,
    description: String,
    file: Option<ChosenFile>,
    check: CheckState,
    /// An end far from its Schacht is accepted
    confirmed: bool,
    saving: bool,
    file_input: NodeRef,
    container: NodeRef,
    map: Option<leaflet::Map>,
    /// Drawn on the map, replaced on changes
    layers: Vec<leaflet::Layer>,
    /// Number of the last check, older answers are dropped
    check_request: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct ChosenFile {
    name: String,
    content: GeoFile,
    /// Index of the line in the file
    line: usize,
    system: CoordinateSystem,
}

#[derive(Debug, Clone, PartialEq)]
enum CheckState {
    None,
    Checking,
    Checked(DuctLineCheck),
    Failed(String),
}

pub enum Msg {
    Loaded(Option<DuctProperties>, Vec<SchachtChoice>),
    LoadError(FrontendError),
    SetSchachtA(Option<i32>),
    SetSchachtZ(Option<i32>),
    SetDescription(String),
    ChooseFile,
    FileChosen,
    FileRead(String, Result<GeoFile, String>),
    SelectLine(usize),
    SetSystem(CoordinateSystem),
    Checked(u32, Result<DuctLineCheck, FrontendError>),
    SetConfirmed(bool),
    DiscardFile,
    Save,
    SaveLine,
    StraightLine,
    Done(Result<Option<i32>, FrontendError>),
    Delete,
}

#[derive(Clone, PartialEq, Properties)]
pub struct EditDuctPropertiesProps {
    pub plan_id: i32,
    /// A temporary id: create a new duct
    pub duct: IdOrNew,
}

impl Component for EditDuctProperties {
    type Message = Msg;
    type Properties = EditDuctPropertiesProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::fetch(ctx);
        Self {
            loaded: None,
            error: None,
            schacht_a: None,
            schacht_z: None,
            description: String::new(),
            file: None,
            check: CheckState::None,
            confirmed: false,
            saving: false,
            file_input: NodeRef::default(),
            container: NodeRef::default(),
            map: None,
            layers: Vec::new(),
            check_request: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(duct, schaechte) => {
                self.error = None;
                self.schacht_a = duct.as_ref().map(|d| d.schacht_a.id);
                self.schacht_z = duct.as_ref().map(|d| d.schacht_z.id);
                self.description = duct
                    .as_ref()
                    .and_then(|d| d.description.clone())
                    .unwrap_or_default();
                self.loaded = Some((duct, schaechte));
                self.check(ctx);
            }
            Msg::LoadError(error) => self.error = Some(error),
            Msg::SetSchachtA(id) => {
                self.schacht_a = id;
                self.check(ctx);
            }
            Msg::SetSchachtZ(id) => {
                self.schacht_z = id;
                self.check(ctx);
            }
            Msg::SetDescription(description) => self.description = description,
            Msg::ChooseFile => {
                if let Some(input) = self.file_input.cast::<HtmlInputElement>() {
                    input.click();
                }
                return false;
            }
            Msg::FileChosen => {
                self.read_file(ctx);
                return false;
            }
            Msg::FileRead(name, result) => match result {
                Ok(content) => {
                    self.file = Some(ChosenFile {
                        name,
                        system: content.system,
                        content,
                        line: 0,
                    });
                    self.check(ctx);
                }
                Err(error) => {
                    self.toast(ctx, &format!("{name} konnte nicht gelesen werden"), error)
                }
            },
            Msg::SelectLine(line) => {
                if let Some(file) = &mut self.file {
                    file.line = line;
                }
                self.check(ctx);
            }
            Msg::SetSystem(system) => {
                if let Some(file) = &mut self.file {
                    file.system = system;
                }
                self.check(ctx);
            }
            Msg::Checked(request, result) => {
                if request != self.check_request {
                    return false;
                }
                self.check = match result {
                    Ok(check) => CheckState::Checked(check),
                    Err(error) => CheckState::Failed(error.to_string()),
                };
                self.redraw();
            }
            Msg::SetConfirmed(confirmed) => self.confirmed = confirmed,
            Msg::DiscardFile => {
                self.file = None;
                self.check(ctx);
            }
            Msg::Save => self.save(ctx),
            Msg::SaveLine => self.save_line(ctx, self.file_line()),
            Msg::StraightLine => self.save_line(ctx, None),
            Msg::Done(result) => {
                self.saving = false;
                match result {
                    Ok(Some(id)) => self.navigate(
                        ctx,
                        PlanView::Duct {
                            id,
                            view: DuctView::Properties,
                        },
                    ),
                    Ok(None) => {
                        self.file = None;
                        self.check = CheckState::None;
                        Self::fetch(ctx);
                        if let Some(toaster) = get_toaster(ctx.link()) {
                            toaster.toast(Toast {
                                title: "Trasse gespeichert".into(),
                                r#type: AlertType::Success,
                                timeout: Some(std::time::Duration::from_secs(3)),
                                ..Toast::default()
                            });
                        }
                    }
                    Err(error) => self.toast(ctx, "Trasse konnte nicht gespeichert werden", error),
                }
            }
            Msg::Delete => {
                if let IdOrNew::Id(id) = ctx.props().duct {
                    let scope = ctx.link().clone();
                    let credentials = get_credentials(&scope);
                    let plan_id = ctx.props().plan_id;
                    let toaster = get_toaster(&scope);
                    spawn_local(async move {
                        match delete_duct(credentials.as_ref(), id).await {
                            Ok(()) => {
                                if let Some((router, _)) =
                                    scope.context::<RouterContext<AppRoute>>(Callback::noop())
                                {
                                    router.push(AppRoute::Plan {
                                        plan_id,
                                        view: PlanView::ListOfDucts,
                                    });
                                }
                            }
                            Err(error) => {
                                if let Some(toaster) = toaster {
                                    toaster.toast(Toast {
                                        title: "Trasse konnte nicht gelöscht werden".into(),
                                        r#type: AlertType::Danger,
                                        body: html!(error.to_string()),
                                        ..Toast::default()
                                    });
                                }
                            }
                        }
                    });
                }
                return false;
            }
        }
        self.redraw();
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        // Every IdOrNew::default() is a new temporary id, that's still the same new duct
        let other = match (&ctx.props().duct, &old_props.duct) {
            (IdOrNew::Id(new), IdOrNew::Id(old)) => new != old,
            (IdOrNew::Temporary(_), IdOrNew::Temporary(_)) => false,
            _ => true,
        };
        if other {
            self.loaded = None;
            self.file = None;
            self.check = CheckState::None;
            Self::fetch(ctx);
        }
        other
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let is_new = self.is_new(ctx);
        let title = if is_new {
            "Neue Trasse".into()
        } else {
            let stored = self.stored().map(|duct| {
                duct_title(
                    duct.description.as_deref(),
                    self.schacht_name(duct.schacht_a.id),
                    self.schacht_name(duct.schacht_z.id),
                )
            });
            object_title(DuctView::Properties.title(), stored)
        };
        let content = if let Some(error) = &self.error {
            error.into_prop_value()
        } else if let Some((stored, schaechte)) = &self.loaded {
            if stored.is_none() && !is_new {
                (&FrontendError::NotFound).into_prop_value()
            } else {
                self.view_form(ctx, schaechte)
            }
        } else {
            html!(<Spinner/>)
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <PageLayout {title}>
                <div class="map-layout">
                    <div class="duct-properties__form">{content}</div>
                    <div class="map-layout__map" ref={self.container.clone()}/>
                </div>
            </PageLayout>
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        let Some(container) = self.container.cast::<HtmlElement>() else {
            return;
        };
        match create_map(&container) {
            Ok(map) => {
                self.map = Some(map);
                self.redraw();
            }
            Err(error) => self.error = Some(FrontendError::Map(error)),
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        self.layers.clear();
        if let Some(map) = self.map.take() {
            map.remove();
        }
    }
}

impl EditDuctProperties {
    fn fetch(ctx: &Context<Self>) {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let id = match ctx.props().duct {
            IdOrNew::Id(id) => Some(id),
            IdOrNew::Temporary(_) => None,
        };
        spawn_local(async move {
            scope.send_message(
                match fetch_duct_properties(credentials.as_ref(), id).await {
                    Ok((duct, schaechte)) => Msg::Loaded(duct, schaechte),
                    Err(error) => Msg::LoadError(error),
                },
            );
        });
    }

    fn is_new(&self, ctx: &Context<Self>) -> bool {
        matches!(ctx.props().duct, IdOrNew::Temporary(_))
    }

    fn can_edit(&self, ctx: &Context<Self>) -> bool {
        get_role(ctx.link()) >= Role::Planner
    }

    fn stored(&self) -> Option<&DuctProperties> {
        self.loaded.as_ref()?.0.as_ref()
    }

    fn schaechte(&self) -> &[SchachtChoice] {
        self.loaded
            .as_ref()
            .map(|(_, schaechte)| schaechte.as_slice())
            .unwrap_or_default()
    }

    fn schacht(&self, id: Option<i32>) -> Option<&SchachtChoice> {
        let id = id?;
        self.schaechte().iter().find(|s| s.id == id)
    }

    fn schacht_name(&self, id: i32) -> &str {
        self.schacht(Some(id)).map_or("?", |s| s.name.as_str())
    }

    /// Whether cables run through the duct, then its Schächte are fixed.
    fn has_cables(&self) -> bool {
        self.stored().is_some_and(|duct| !duct.cables.is_empty())
    }

    /// The Schächte the course is checked against: the stored ones of an existing duct (the
    /// course is stored on its own), the chosen ones of a new one.
    fn ends(&self, ctx: &Context<Self>) -> Option<(i32, i32)> {
        if self.is_new(ctx) {
            Some((self.schacht_a?, self.schacht_z?))
        } else {
            let duct = self.stored()?;
            Some((duct.schacht_a.id, duct.schacht_z.id))
        }
    }

    fn file_line(&self) -> Option<LineInput> {
        let file = self.file.as_ref()?;
        let line = file.content.lines.get(file.line)?;
        Some(LineInput {
            system: file.system,
            points: line.points.clone(),
        })
    }

    /// Checks the chosen line against the Schächte, the answer comes as `Msg::Checked`.
    fn check(&mut self, ctx: &Context<Self>) {
        self.check_request += 1;
        self.confirmed = false;
        let (Some(line), Some((schacht_a, schacht_z))) = (self.file_line(), self.ends(ctx)) else {
            self.check = CheckState::None;
            return;
        };
        if schacht_a == schacht_z {
            self.check =
                CheckState::Failed("Anfangs- und Endschacht müssen verschieden sein".into());
            return;
        }
        self.check = CheckState::Checking;
        let request = self.check_request;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            let result = check_duct_line(credentials.as_ref(), schacht_a, schacht_z, line).await;
            scope.send_message(Msg::Checked(request, result));
        });
    }

    fn read_file(&self, ctx: &Context<Self>) {
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
                Ok(text) => read_geo_file(&name, &text.as_string().unwrap_or_default()),
                Err(error) => Err(format!("{error:?}")),
            };
            scope.send_message(Msg::FileRead(name, result));
        });
    }

    /// Whether the checked line may be stored (an end far from its Schacht needs confirmation).
    fn line_ready(&self) -> bool {
        match &self.check {
            CheckState::Checked(check) => !check.needs_confirmation || self.confirmed,
            _ => false,
        }
    }

    fn input(&self) -> Option<DuctInput> {
        let description = self.description.trim();
        Some(DuctInput {
            schacht_a: self.schacht_a?,
            schacht_z: self.schacht_z?,
            description: (!description.is_empty()).then(|| description.to_string()),
        })
    }

    fn has_changes(&self, ctx: &Context<Self>) -> bool {
        match self.stored() {
            None => self.is_new(ctx),
            Some(duct) => {
                self.schacht_a != Some(duct.schacht_a.id)
                    || self.schacht_z != Some(duct.schacht_z.id)
                    || self.description.trim() != duct.description.as_deref().unwrap_or_default()
            }
        }
    }

    fn save(&mut self, ctx: &Context<Self>) {
        let Some(input) = self.input() else {
            return;
        };
        // A new duct takes its course along if one is checked
        let line = self.file_line().filter(|_| self.line_ready());
        let confirmed = self.confirmed;
        self.saving = true;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let duct = ctx.props().duct;
        spawn_local(async move {
            let result = match duct {
                IdOrNew::Id(id) => update_duct(credentials.as_ref(), id, input)
                    .await
                    .map(|()| None),
                IdOrNew::Temporary(_) => create_duct(credentials.as_ref(), input, line, confirmed)
                    .await
                    .map(Some),
            };
            scope.send_message(Msg::Done(result));
        });
    }

    fn save_line(&mut self, ctx: &Context<Self>, line: Option<LineInput>) {
        let IdOrNew::Id(id) = ctx.props().duct else {
            return;
        };
        let confirmed = self.confirmed;
        self.saving = true;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            let result = set_duct_line(credentials.as_ref(), id, line, confirmed)
                .await
                .map(|()| None);
            scope.send_message(Msg::Done(result));
        });
    }

    fn navigate(&self, ctx: &Context<Self>, view: PlanView) {
        if let Some((router, _)) = ctx
            .link()
            .context::<RouterContext<AppRoute>>(Callback::noop())
        {
            router.push(AppRoute::Plan {
                plan_id: ctx.props().plan_id,
                view,
            });
        }
    }

    fn toast(&self, ctx: &Context<Self>, title: &str, error: impl ToString) {
        if let Some(toaster) = get_toaster(ctx.link()) {
            toaster.toast(Toast {
                title: title.to_string(),
                r#type: AlertType::Danger,
                body: html!(error.to_string()),
                ..Toast::default()
            });
        }
    }

    /// The chosen Schächte, the stored course and the checked one from the file.
    fn redraw(&mut self) {
        let Some(map) = &self.map else {
            return;
        };
        for layer in self.layers.drain(..) {
            layer.remove();
        }
        let mut layers: Vec<leaflet::Layer> = Vec::new();
        let mut shown: Vec<GeoPoint> = Vec::new();
        if let Some(line) = self.stored().and_then(|duct| duct.line.as_ref()) {
            let stored = duct_line(line, "map-view__duct");
            stored.add_to(map);
            layers.push(stored.unchecked_into());
            shown.extend(line);
        }
        if let CheckState::Checked(check) = &self.check {
            let preview = duct_line(&check.line, "map-view__duct map-view__duct--preview");
            preview.add_to(map);
            layers.push(preview.unchecked_into());
            shown.extend(&check.line);
        }
        let ends = [self.schacht_a, self.schacht_z];
        for schacht in ends.into_iter().filter_map(|id| self.schacht(id)) {
            let Some(location) = schacht.location else {
                continue;
            };
            let marker = schacht_marker(&schacht.name, location, || {});
            marker.add_to(map);
            layers.push(marker.unchecked_into());
            shown.push(location);
        }
        fit_points(map, &shown);
        self.layers = layers;
    }

    fn view_form(&self, ctx: &Context<Self>, schaechte: &[SchachtChoice]) -> Html {
        let readonly = !self.can_edit(ctx);
        let link = ctx.link();
        let fixed_ends = readonly || self.has_cables();
        let schacht_field = |value: Option<i32>, onchange: Callback<Option<i32>>| {
            if fixed_ends {
                let value = self
                    .schacht(value)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                html!(<TextInput {value} readonly=true/>)
            } else {
                let options = schaechte.iter().map(|s| {
                    html_nested!(<FormSelectOption<i32> value={s.id} description={s.name.clone()}/>)
                });
                html! {
                    <FormSelect<i32> {value} {onchange} placeholder=" - ">
                        {for options}
                    </FormSelect<i32>>
                }
            }
        };
        html! {
            <Form>
                <FormGroup label="Von Schacht (A)" required={!fixed_ends}>
                    {schacht_field(self.schacht_a, link.callback(Msg::SetSchachtA))}
                </FormGroup>
                <FormGroup label="Bis Schacht (Z)" required={!fixed_ends}>
                    {schacht_field(self.schacht_z, link.callback(Msg::SetSchachtZ))}
                </FormGroup>
                if self.has_cables() && !readonly {
                    <p class="duct-properties__hint">
                        {"Durch die Trasse führen Kabel, ihre Schächte sind deshalb fest."}
                    </p>
                }
                <FormGroup label="Beschreibung">
                    <TextInput
                        value={self.description.clone()}
                        onchange={link.callback(Msg::SetDescription)}
                        placeholder="nach den Schächten benannt"
                        {readonly}
                    />
                </FormGroup>
                if !readonly && !self.is_new(ctx) {
                    {self.view_actions(ctx)}
                }
                <FormGroup label="Verlauf">{self.view_course(ctx)}</FormGroup>
                if !readonly && self.is_new(ctx) {
                    {self.view_actions(ctx)}
                }
            </Form>
        }
    }

    fn view_actions(&self, ctx: &Context<Self>) -> Html {
        if self.saving {
            return html!(<Spinner/>);
        }
        let is_new = self.is_new(ctx);
        let can_save = self.input().is_some() && self.has_changes(ctx);
        let delete = get_backdrop(ctx.link())
            .filter(|_| !is_new && !self.has_cables() && get_role(ctx.link()) >= Role::Admin)
            .map(|backdropper| {
                let scope = ctx.link().clone();
                let onclick = Callback::from(move |_| {
                    let on_confirm = {
                        let backdropper = backdropper.clone();
                        let scope = scope.clone();
                        Callback::from(move |_| {
                            backdropper.close();
                            scope.send_message(Msg::Delete);
                        })
                    };
                    let on_cancel = {
                        let backdropper = backdropper.clone();
                        Callback::from(move |_| backdropper.close())
                    };
                    backdropper.open(Backdrop::new(html! {
                        <DeleteConfirmationDialog {on_confirm} {on_cancel}/>
                    }));
                });
                html_nested!(<Button variant={ButtonVariant::DangerSecondary} label="Löschen" {onclick}/>)
            });
        html! {
            <ActionGroup>
                <Button
                    variant={ButtonVariant::Primary}
                    label={if is_new { "Anlegen" } else { "Speichern" }}
                    onclick={ctx.link().callback(|_| Msg::Save)}
                    disabled={!can_save}
                />
                {for delete}
            </ActionGroup>
        }
    }

    /// The stored course and, for planners, a file to replace it.
    fn view_course(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let stored = match self.stored() {
            None => Html::default(),
            Some(duct) => {
                let points = duct
                    .line
                    .as_ref()
                    .map_or(0, |line| line.len().saturating_sub(2));
                let text = match (points, duct.length) {
                    (0, Some(length)) => format!("Gerade Linie, {length:.1} m"),
                    (1, Some(length)) => format!("1 Zwischenpunkt, {length:.1} m"),
                    (points, Some(length)) => {
                        format!("{points} Zwischenpunkte, {length:.1} m")
                    }
                    (_, None) => "Die Schächte haben keine Position".to_string(),
                };
                html!(<p>{text}</p>)
            }
        };
        if !self.can_edit(ctx) {
            return stored;
        }
        let has_course = self
            .stored()
            .and_then(|duct| duct.line.as_ref())
            .is_some_and(|line| line.len() > 2);
        let straight = (has_course && !self.is_new(ctx) && self.file.is_none()).then(|| {
            html_nested! {
                <Button
                    variant={ButtonVariant::Link}
                    label="Gerade Linie"
                    onclick={link.callback(|_| Msg::StraightLine)}
                    disabled={self.saving}
                />
            }
        });
        html! {
            <>
                {stored}
                <input
                    type="file"
                    accept=".geojson,.json,.gpx"
                    class="duct-properties__file"
                    ref={self.file_input.clone()}
                    onchange={link.callback(|_| Msg::FileChosen)}
                />
                <ActionGroup>
                    <Button
                        variant={ButtonVariant::Secondary}
                        icon={Icon::Upload}
                        label="Verlauf aus Datei (GeoJSON, GPX)"
                        onclick={link.callback(|_| Msg::ChooseFile)}
                    />
                    {for straight}
                </ActionGroup>
                {self.view_file(ctx)}
            </>
        }
    }

    /// The chosen file: its line and system, the check's result and, for an existing duct,
    /// storing the course.
    fn view_file(&self, ctx: &Context<Self>) -> Html {
        let Some(file) = &self.file else {
            return Html::default();
        };
        let link = ctx.link();
        let line_choice = (file.content.lines.len() > 1).then(|| {
            let options = file.content.lines.iter().enumerate().map(|(index, line)| {
                html_nested!(<FormSelectOption<usize> value={index} description={line.name.clone()}/>)
            });
            html! {
                <FormGroup label="Linie">
                    <FormSelect<usize>
                        value={Some(file.line)}
                        onchange={link.callback(|line: Option<usize>| Msg::SelectLine(line.unwrap_or_default()))}
                    >
                        {for options}
                    </FormSelect<usize>>
                </FormGroup>
            }
        });
        let system_item = |system: CoordinateSystem, text: &'static str| {
            html_nested! {
                <ToggleGroupItem
                    {text}
                    selected={file.system == system}
                    onchange={link.callback(move |_| Msg::SetSystem(system))}
                />
            }
        };
        let result = match &self.check {
            CheckState::None => html! {
                <p class="duct-properties__hint">{"Zum Prüfen beide Schächte wählen"}</p>
            },
            CheckState::Checking => {
                html!(<Spinner size={patternfly_yew::prelude::SpinnerSize::Md}/>)
            }
            CheckState::Failed(error) => {
                html!(<Alert inline=true r#type={AlertType::Danger} title={error.clone()}/>)
            }
            CheckState::Checked(check) => self.view_check(ctx, check),
        };
        let take_over = (!self.is_new(ctx)).then(|| {
            html_nested! {
                <Button
                    variant={ButtonVariant::Primary}
                    label="Verlauf übernehmen"
                    onclick={link.callback(|_| Msg::SaveLine)}
                    disabled={!self.line_ready() || self.saving}
                />
            }
        });
        html! {
            <div class="duct-properties__file-choice">
                <p><strong>{&file.name}</strong></p>
                {line_choice}
                <FormGroup label="Koordinatensystem der Datei">
                    <ToggleGroup>
                        {system_item(CoordinateSystem::Lv95, "LV95")}
                        {system_item(CoordinateSystem::Lv03, "LV03")}
                        {system_item(CoordinateSystem::Wgs84, "WGS84")}
                    </ToggleGroup>
                </FormGroup>
                {result}
                <ActionGroup>
                    {for take_over}
                    <Button
                        variant={ButtonVariant::Link}
                        label="Verwerfen"
                        onclick={link.callback(|_| Msg::DiscardFile)}
                    />
                </ActionGroup>
            </div>
        }
    }

    fn view_check(&self, ctx: &Context<Self>, check: &DuctLineCheck) -> Html {
        let mut notes = Vec::new();
        if check.reversed {
            notes.push("Die Linie lief von Schacht Z nach A und wurde umgedreht.".to_string());
        }
        match check.removed_ends {
            0 => {}
            1 => notes.push("Ein Endpunkt lag auf seinem Schacht und wurde weggelassen.".into()),
            _ => notes
                .push("Beide Endpunkte lagen auf ihren Schächten und wurden weggelassen.".into()),
        }
        notes.push(format!(
            "Abstand der Enden zu den Schächten: {:.1} m (A) und {:.1} m (Z), Länge {:.1} m.",
            check.start_distance, check.end_distance, check.length
        ));
        let confirm = check.needs_confirmation.then(|| {
            let checked = if self.confirmed {
                CheckboxState::Checked
            } else {
                CheckboxState::Unchecked
            };
            html! {
                <Alert inline=true r#type={AlertType::Warning}
                    title="Die Linie endet weit von den Schächten entfernt">
                    <p>{"Falsche Datei oder falscher Schacht?"}</p>
                    <Checkbox
                        label="Trotzdem verwenden"
                        {checked}
                        onchange={ctx.link().callback(|state: CheckboxState| Msg::SetConfirmed(state == CheckboxState::Checked))}
                    />
                </Alert>
            }
        });
        html! {
            <>
                <ul class="duct-properties__notes">
                    {for notes.into_iter().map(|note| html!(<li>{note}</li>))}
                </ul>
                {confirm}
            </>
        }
    }
}
