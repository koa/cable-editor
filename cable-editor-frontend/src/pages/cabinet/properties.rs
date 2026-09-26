use crate::{
    components::{
        dialog::confirm_delete,
        page_layout::{PageLayout, object_title},
    },
    error::FrontendError,
    geo::{
        coordinates::{Check, CoordinateSystem, check, parse_number, split_pair},
        map::{create_map, div_icon, lat_lng},
    },
    graphql::authenticated::{
        GeoPoint, IdOrNew,
        current_user::Role,
        schacht_properties::{
            ConvertedPoint, GeoPointInput, Lv95Input, PositionInput, SchachtInput,
            SchachtProperties, SchachtTypeEntry, convert_point, create_schacht, delete_schacht,
            fetch_schacht_properties, fetch_schacht_types, update_schacht,
        },
    },
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, get_role, navigate, toast_error, toast_success},
};
use gloo_timers::callback::Timeout;
use leaflet::{DragEvents, Marker, MarkerOptions, MouseEvent};
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Button, ButtonVariant, Form, FormGroup, FormSelect,
    FormSelectOption, Icon, Spinner, TextInput, ToggleGroup, ToggleGroupItem,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
// The stable (older) names of GeolocationPosition and GeolocationPositionError
use web_sys::{HtmlElement, Position, PositionError, PositionOptions};
use yew::{
    Component, Context, Html, NodeRef, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

/// Waiting time after typing before the position is converted and shown on the map.
const CONVERT_DELAY_MS: u32 = 400;
/// Zoom when showing the position; the cadastral map starts at 17.
const POSITION_ZOOM: f64 = 18.0;

/// Name, type and position of a Schacht, or a new one (`IdOrNew::Temporary`). The position
/// can be set on the map (click, drag the marker), typed in LV95 or WGS84 or taken from the
/// device's location. Readers see the same page read-only.
pub struct CabinetProperties {
    /// The Schacht as stored (missing for a new one) and the types to choose from
    loaded: Option<(Option<SchachtProperties>, Vec<SchachtTypeEntry>)>,
    error: Option<FrontendError>,
    name: String,
    type_id: Option<i32>,
    system: CoordinateSystem,
    /// The coordinate fields as typed
    first: String,
    second: String,
    position: PositionState,
    /// Accuracy of the device's location, while it is the position
    accuracy: Option<f64>,
    locating: bool,
    saving: bool,
    container: NodeRef,
    map: Option<leaflet::Map>,
    marker: Option<Marker>,
    /// Number of the last conversion request, older answers are dropped
    conversion: u32,
    _convert_delay: Option<Timeout>,
}

#[derive(Debug, Clone, PartialEq)]
enum PositionState {
    /// No position (fields empty)
    Empty,
    /// Waiting for the backend to convert
    Checking,
    Valid(ConvertedPoint),
    /// Not plausible, probably meant as the proposed values
    Proposal {
        reason: &'static str,
        first: f64,
        second: f64,
    },
    Invalid(String),
}

pub enum Msg {
    Loaded(Option<SchachtProperties>, Vec<SchachtTypeEntry>),
    LoadError(FrontendError),
    SetName(String),
    SetType(Option<i32>),
    SetSystem(CoordinateSystem),
    SetFirst(String),
    SetSecond(String),
    Convert,
    Converted {
        request: u32,
        result: Result<ConvertedPoint, FrontendError>,
        /// Fill the fields with the result (the position came from the map or the device)
        fill: bool,
    },
    ApplyProposal,
    /// A position from the map or the device
    Picked(GeoPoint, Option<f64>),
    Locate,
    LocateFailed(String),
    ClearPosition,
    Save,
    Saved(Result<SchachtProperties, FrontendError>),
    Created(Result<i32, FrontendError>),
    Delete,
    DeleteFailed(FrontendError),
}

#[derive(Clone, PartialEq, Properties)]
pub struct CabinetPropertiesProps {
    pub plan_id: i32,
    /// A temporary id: create a new Schacht
    pub cabinet: IdOrNew,
}

impl Component for CabinetProperties {
    type Message = Msg;
    type Properties = CabinetPropertiesProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::fetch(ctx);
        Self {
            loaded: None,
            error: None,
            name: String::new(),
            type_id: None,
            system: CoordinateSystem::default(),
            first: String::new(),
            second: String::new(),
            position: PositionState::Empty,
            accuracy: None,
            locating: false,
            saving: false,
            container: NodeRef::default(),
            map: None,
            marker: None,
            conversion: 0,
            _convert_delay: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(schacht, types) => {
                self.error = None;
                self.take_stored(ctx, schacht.as_ref());
                self.loaded = Some((schacht, types));
            }
            Msg::LoadError(error) => self.error = Some(error),
            Msg::SetName(name) => self.name = name,
            Msg::SetType(type_id) => self.type_id = type_id,
            Msg::SetSystem(system) => {
                self.system = system;
                if let PositionState::Valid(point) = self.position {
                    self.fill_fields(point);
                } else {
                    self.check_fields(ctx);
                }
            }
            Msg::SetFirst(text) => {
                // Both values pasted into the first field
                if let Some((first, second)) = split_pair(&text) {
                    self.first = first;
                    self.second = second;
                } else {
                    self.first = text;
                }
                self.accuracy = None;
                self.check_fields(ctx);
            }
            Msg::SetSecond(text) => {
                self.second = text;
                self.accuracy = None;
                self.check_fields(ctx);
            }
            Msg::Convert => {
                self._convert_delay = None;
                if let (Some(first), Some(second)) =
                    (parse_number(&self.first), parse_number(&self.second))
                {
                    self.convert(ctx, self.system.input(first, second), false);
                }
            }
            Msg::Converted {
                request,
                result,
                fill,
            } => {
                if request != self.conversion {
                    return false;
                }
                match result {
                    Ok(point) => {
                        if fill {
                            self.fill_fields(point);
                        }
                        self.position = PositionState::Valid(point);
                        // Typed positions are brought into view, picked ones are there already
                        self.show_marker(ctx, point.wgs84, !fill);
                    }
                    Err(error) => self.position = PositionState::Invalid(error.to_string()),
                }
            }
            Msg::ApplyProposal => {
                if let PositionState::Proposal { first, second, .. } = self.position {
                    (self.first, self.second) = self.system.format(first, second);
                    self.check_fields(ctx);
                }
            }
            Msg::Picked(point, accuracy) => {
                self.locating = false;
                self.accuracy = accuracy;
                let input = PositionInput::Wgs84(GeoPointInput {
                    lat: point.lat,
                    lng: point.lng,
                });
                self.position = PositionState::Checking;
                self.convert(ctx, input, true);
                if accuracy.is_some() {
                    self.show_marker(ctx, point, true);
                }
            }
            Msg::Locate => {
                self.locating = true;
                if let Err(error) = locate(ctx) {
                    self.locating = false;
                    toast_error(ctx.link(), "Standort nicht verfügbar", error);
                }
            }
            Msg::LocateFailed(error) => {
                self.locating = false;
                toast_error(ctx.link(), "Standort nicht verfügbar", error);
            }
            Msg::ClearPosition => {
                self.first.clear();
                self.second.clear();
                self.accuracy = None;
                self.check_fields(ctx);
            }
            Msg::Save => self.save(ctx),
            Msg::Saved(result) => {
                self.saving = false;
                match result {
                    Ok(schacht) => {
                        self.take_stored(ctx, Some(&schacht));
                        if let Some((stored, _)) = &mut self.loaded {
                            *stored = Some(schacht);
                        }
                        toast_success(ctx.link(), "Schacht gespeichert");
                    }
                    Err(error) => {
                        toast_error(ctx.link(), "Schacht konnte nicht gespeichert werden", error)
                    }
                }
            }
            Msg::Created(result) => {
                self.saving = false;
                match result {
                    Ok(id) => navigate(
                        ctx.link(),
                        ctx.props().plan_id,
                        PlanView::Cabinet {
                            id,
                            view: CabinetView::Properties,
                        },
                    ),
                    Err(error) => {
                        toast_error(ctx.link(), "Schacht konnte nicht angelegt werden", error)
                    }
                }
            }
            Msg::Delete => {
                if let IdOrNew::Id(id) = ctx.props().cabinet {
                    let scope = ctx.link().clone();
                    let credentials = get_credentials(&scope);
                    let plan_id = ctx.props().plan_id;
                    spawn_local(async move {
                        match delete_schacht(credentials.as_ref(), id).await {
                            Ok(()) => navigate(&scope, plan_id, PlanView::ListOfCabinets),
                            Err(error) => scope.send_message(Msg::DeleteFailed(error)),
                        }
                    });
                }
                return false;
            }
            Msg::DeleteFailed(error) => {
                toast_error(ctx.link(), "Schacht konnte nicht gelöscht werden", error);
                return false;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        // A new Schacht's temporary id comes from the route, so it's stable
        if ctx.props().cabinet != old_props.cabinet {
            self.loaded = None;
            Self::fetch(ctx);
            true
        } else {
            false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let is_new = matches!(ctx.props().cabinet, IdOrNew::Temporary(_));
        let title = if is_new {
            "Neuer Schacht".into()
        } else {
            let stored_name = self
                .loaded
                .as_ref()
                .and_then(|(schacht, _)| schacht.as_ref())
                .map(|schacht| schacht.name.clone());
            object_title(CabinetView::Properties.title(), stored_name)
        };
        let content = if let Some(error) = &self.error {
            error.into_prop_value()
        } else if let Some((stored, types)) = &self.loaded {
            if stored.is_none() && !is_new {
                (&FrontendError::NotFound).into_prop_value()
            } else {
                self.view_form(ctx, types)
            }
        } else {
            html!(<Spinner/>)
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <PageLayout {title}>
                <div class="map-layout">
                    <div class="schacht-properties__form">{content}</div>
                    <div class="map-layout__map" ref={self.container.clone()}/>
                </div>
            </PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        let Some(container) = self.container.cast::<HtmlElement>() else {
            return;
        };
        match create_map(&container) {
            Ok(map) => {
                if self.can_edit(ctx) {
                    let scope = ctx.link().clone();
                    map.on_mouse_click(Box::new(move |event: MouseEvent| {
                        let at = event.lat_lng();
                        scope.send_message(Msg::Picked(
                            GeoPoint {
                                lat: at.lat(),
                                lng: at.lng(),
                            },
                            None,
                        ))
                    }));
                }
                self.map = Some(map);
                if let PositionState::Valid(point) = self.position {
                    self.show_marker(ctx, point.wgs84, true);
                }
            }
            Err(error) => self.error = Some(FrontendError::Map(error)),
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        self.marker = None;
        if let Some(map) = self.map.take() {
            map.remove();
        }
    }
}

impl CabinetProperties {
    fn fetch(ctx: &Context<Self>) {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let cabinet = ctx.props().cabinet;
        spawn_local(async move {
            let result = match cabinet {
                IdOrNew::Id(id) => fetch_schacht_properties(credentials.as_ref(), id).await,
                IdOrNew::Temporary(_) => fetch_schacht_types(credentials.as_ref())
                    .await
                    .map(|types| (None, types)),
            };
            scope.send_message(match result {
                Ok((schacht, types)) => Msg::Loaded(schacht, types),
                Err(error) => Msg::LoadError(error),
            });
        });
    }

    fn can_edit(&self, ctx: &Context<Self>) -> bool {
        get_role(ctx.link()) >= Role::Planner
    }

    /// Shows the stored values in the fields.
    fn take_stored(&mut self, ctx: &Context<Self>, schacht: Option<&SchachtProperties>) {
        self.name = schacht.map(|s| s.name.clone()).unwrap_or_default();
        self.type_id = schacht.and_then(|s| s.typ).map(|t| t.id);
        self.accuracy = None;
        let stored = schacht.and_then(stored_position);
        match stored {
            Some(point) => {
                self.fill_fields(point);
                self.position = PositionState::Valid(point);
                self.show_marker(ctx, point.wgs84, true);
            }
            None => {
                self.first.clear();
                self.second.clear();
                self.position = PositionState::Empty;
                if let Some(marker) = self.marker.take() {
                    marker.remove();
                }
            }
        }
    }

    fn fill_fields(&mut self, point: ConvertedPoint) {
        (self.first, self.second) = match self.system {
            CoordinateSystem::Lv95 => self.system.format(point.lv95.e, point.lv95.n),
            CoordinateSystem::Wgs84 => self.system.format(point.wgs84.lat, point.wgs84.lng),
        };
    }

    /// Checks the typed values and, if plausible, converts them after a short delay.
    fn check_fields(&mut self, ctx: &Context<Self>) {
        self._convert_delay = None;
        // Invalidates conversions under way
        self.conversion += 1;
        self.position = match (
            self.first.trim().is_empty() && self.second.trim().is_empty(),
            parse_number(&self.first),
            parse_number(&self.second),
        ) {
            (true, _, _) => {
                if let Some(marker) = self.marker.take() {
                    marker.remove();
                }
                PositionState::Empty
            }
            (false, Some(first), Some(second)) => match check(self.system, first, second) {
                Check::Valid(..) => {
                    let scope = ctx.link().clone();
                    self._convert_delay = Some(Timeout::new(CONVERT_DELAY_MS, move || {
                        scope.send_message(Msg::Convert)
                    }));
                    PositionState::Checking
                }
                Check::Proposal {
                    reason,
                    first,
                    second,
                } => PositionState::Proposal {
                    reason,
                    first,
                    second,
                },
                Check::Invalid(reason) => PositionState::Invalid(reason.to_string()),
            },
            _ => PositionState::Invalid("Bitte beide Werte als Zahl eingeben".to_string()),
        };
    }

    fn convert(&mut self, ctx: &Context<Self>, input: PositionInput, fill: bool) {
        self.conversion += 1;
        let request = self.conversion;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            let result = convert_point(credentials.as_ref(), input).await;
            scope.send_message(Msg::Converted {
                request,
                result,
                fill,
            });
        });
    }

    /// Places the marker (creates it on the first position); `center` brings it into view.
    fn show_marker(&mut self, ctx: &Context<Self>, point: GeoPoint, center: bool) {
        let Some(map) = &self.map else {
            return;
        };
        let position = lat_lng(point);
        match &self.marker {
            Some(marker) => marker.set_lat_lng(&position),
            None => {
                let options = MarkerOptions::default();
                let editable = self.can_edit(ctx);
                options.set_draggable(editable);
                match div_icon("schacht-properties__marker", 22.0) {
                    Ok(icon) => options.set_icon(icon),
                    // Leaflet's default icon still works, only looks different
                    Err(error) => log::warn!("Can't create the marker icon: {error:?}"),
                }
                let marker = Marker::new_with_options(&position, &options);
                if editable {
                    let scope = ctx.link().clone();
                    let dragged = marker.clone();
                    marker.on_drag_end(Box::new(move |_| {
                        let at = dragged.get_lat_lng();
                        scope.send_message(Msg::Picked(
                            GeoPoint {
                                lat: at.lat(),
                                lng: at.lng(),
                            },
                            None,
                        ))
                    }));
                }
                marker.add_to(map);
                self.marker = Some(marker);
            }
        }
        if center {
            map.set_view(&position, map.get_zoom().max(POSITION_ZOOM));
        }
    }

    fn save(&mut self, ctx: &Context<Self>) {
        let position = match self.position {
            PositionState::Empty => None,
            PositionState::Valid(point) => Some(PositionInput::Lv95(Lv95Input {
                e: point.lv95.e,
                n: point.lv95.n,
            })),
            _ => return,
        };
        let input = SchachtInput {
            name: self.name.trim().to_string(),
            type_id: self.type_id,
            position,
        };
        self.saving = true;
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let cabinet = ctx.props().cabinet;
        spawn_local(async move {
            scope.send_message(match cabinet {
                IdOrNew::Id(id) => {
                    Msg::Saved(update_schacht(credentials.as_ref(), id, input).await)
                }
                IdOrNew::Temporary(_) => {
                    Msg::Created(create_schacht(credentials.as_ref(), input).await)
                }
            });
        });
    }

    /// Whether the fields differ from the stored Schacht (always for a new one).
    fn has_changes(&self) -> bool {
        let Some((stored, _)) = &self.loaded else {
            return false;
        };
        let Some(stored) = stored else {
            return true;
        };
        let stored_point = stored_position(stored).map(|p| p.lv95);
        let position = match self.position {
            PositionState::Empty => None,
            PositionState::Valid(point) => Some(point.lv95),
            _ => return true,
        };
        self.name.trim() != stored.name
            || self.type_id != stored.typ.map(|t| t.id)
            || position != stored_point
    }

    fn view_form(&self, ctx: &Context<Self>, types: &[SchachtTypeEntry]) -> Html {
        let readonly = !self.can_edit(ctx);
        let link = ctx.link();

        let type_options = types.iter().map(|t| {
            let description = t.name.clone().unwrap_or_else(|| format!("Typ {}", t.id));
            html_nested!(<FormSelectOption<i32> value={t.id} {description}/>)
        });
        // FormSelect ignores its `disabled`, so readers get a text field
        let type_field = if readonly {
            let value = self
                .type_id
                .and_then(|id| types.iter().find(|t| t.id == id))
                .and_then(|t| t.name.clone())
                .unwrap_or_default();
            html!(<TextInput {value} readonly=true/>)
        } else {
            html! {
                <FormSelect<i32>
                    value={self.type_id}
                    onchange={link.callback(Msg::SetType)}
                    placeholder=" - "
                >
                    {for type_options}
                </FormSelect<i32>>
            }
        };
        let system_item = |system: CoordinateSystem| {
            html_nested! {
                <ToggleGroupItem
                    text={system.title()}
                    selected={self.system == system}
                    disabled={readonly}
                    onchange={link.callback(move |_| Msg::SetSystem(system))}
                />
            }
        };
        let (first_label, second_label) = self.system.labels();
        let (first_placeholder, second_placeholder) = self.system.placeholders();

        html! {
            <Form>
                <FormGroup label="Name" required={!readonly}>
                    <TextInput
                        value={self.name.clone()}
                        onchange={link.callback(Msg::SetName)}
                        {readonly}
                    />
                </FormGroup>
                <FormGroup label="Typ">{type_field}</FormGroup>
                <FormGroup label="Position">
                    <ToggleGroup>
                        {system_item(CoordinateSystem::Lv95)}
                        {system_item(CoordinateSystem::Wgs84)}
                    </ToggleGroup>
                </FormGroup>
                <div class="schacht-properties__coordinates">
                    <FormGroup label={first_label}>
                        <TextInput
                            value={self.first.clone()}
                            onchange={link.callback(Msg::SetFirst)}
                            placeholder={first_placeholder}
                            {readonly}
                        />
                    </FormGroup>
                    <FormGroup label={second_label}>
                        <TextInput
                            value={self.second.clone()}
                            onchange={link.callback(Msg::SetSecond)}
                            placeholder={second_placeholder}
                            {readonly}
                        />
                    </FormGroup>
                </div>
                {self.view_position_state(ctx)}
                if !readonly {
                    <ActionGroup>
                        <Button
                            variant={ButtonVariant::Secondary}
                            icon={Icon::MapMarker}
                            label="Aktuelle Position"
                            onclick={link.callback(|_| Msg::Locate)}
                            disabled={self.locating}
                        />
                        <Button
                            variant={ButtonVariant::Link}
                            label="Position entfernen"
                            onclick={link.callback(|_| Msg::ClearPosition)}
                            disabled={self.position == PositionState::Empty}
                        />
                    </ActionGroup>
                    {self.view_actions(ctx)}
                }
            </Form>
        }
    }

    /// The position in the other system, or what is wrong with the typed one.
    fn view_position_state(&self, ctx: &Context<Self>) -> Html {
        let accuracy = self
            .accuracy
            .map(|accuracy| format!(" (Genauigkeit ± {accuracy:.0} m)"))
            .unwrap_or_default();
        match &self.position {
            PositionState::Empty => {
                html!(<p class="schacht-properties__hint">{"Keine Position – auf der Karte klicken oder Koordinaten eingeben"}</p>)
            }
            PositionState::Checking => {
                html!(<Spinner size={patternfly_yew::prelude::SpinnerSize::Md}/>)
            }
            PositionState::Valid(point) => {
                let other = match self.system {
                    CoordinateSystem::Lv95 => {
                        let (lat, lng) =
                            CoordinateSystem::Wgs84.format(point.wgs84.lat, point.wgs84.lng);
                        format!("WGS84: {lat}, {lng}")
                    }
                    CoordinateSystem::Wgs84 => {
                        let (e, n) = CoordinateSystem::Lv95.format(point.lv95.e, point.lv95.n);
                        format!("LV95: {e} / {n}")
                    }
                };
                html!(<p class="schacht-properties__hint">{other}{accuracy}</p>)
            }
            PositionState::Proposal {
                reason,
                first,
                second,
            } => {
                let (first, second) = self.system.format(*first, *second);
                html! {
                    <Alert inline=true r#type={AlertType::Warning} title={*reason}>
                        <p>{format!("Gemeint ist wohl {first} / {second}.")}</p>
                        <Button
                            variant={ButtonVariant::Secondary}
                            label="Übernehmen"
                            onclick={ctx.link().callback(|_| Msg::ApplyProposal)}
                        />
                    </Alert>
                }
            }
            PositionState::Invalid(reason) => {
                html!(<Alert inline=true r#type={AlertType::Danger} title={reason.clone()}/>)
            }
        }
    }

    fn view_actions(&self, ctx: &Context<Self>) -> Html {
        if self.saving {
            return html!(<Spinner/>);
        }
        let is_new = matches!(ctx.props().cabinet, IdOrNew::Temporary(_));
        let position_ok = matches!(
            self.position,
            PositionState::Empty | PositionState::Valid(_)
        );
        let can_save = position_ok && !self.name.trim().is_empty() && self.has_changes();
        let delete = (!is_new && get_role(ctx.link()) >= Role::Admin).then(|| {
            let onclick = confirm_delete(ctx.link(), ctx.link().callback(|()| Msg::Delete));
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
}

/// The stored position in both systems.
fn stored_position(schacht: &SchachtProperties) -> Option<ConvertedPoint> {
    Some(ConvertedPoint {
        lv95: crate::graphql::authenticated::Lv95Point {
            e: schacht.position?.x,
            n: schacht.position?.y,
        },
        wgs84: schacht.location?,
    })
}

/// Asks the device for its location; the answer comes as `Msg::Picked` or `Msg::LocateFailed`.
fn locate(ctx: &Context<CabinetProperties>) -> Result<(), String> {
    let geolocation = gloo_utils::window()
        .navigator()
        .geolocation()
        .map_err(|_| "Der Browser kann den Standort nicht bestimmen".to_string())?;
    let scope = ctx.link().clone();
    let on_success = Closure::once_into_js(move |position: JsValue| {
        let position: Position = position.unchecked_into();
        let coords = position.coords();
        scope.send_message(Msg::Picked(
            GeoPoint {
                lat: coords.latitude(),
                lng: coords.longitude(),
            },
            Some(coords.accuracy()),
        ));
    });
    let scope = ctx.link().clone();
    let on_error = Closure::once_into_js(move |error: JsValue| {
        let error: PositionError = error.unchecked_into();
        let message = match error.code() {
            PositionError::PERMISSION_DENIED => {
                "Der Zugriff auf den Standort wurde nicht erlaubt".to_string()
            }
            PositionError::TIMEOUT => {
                "Der Standort konnte nicht rechtzeitig bestimmt werden".to_string()
            }
            _ => error.message(),
        };
        scope.send_message(Msg::LocateFailed(message));
    });
    let options = PositionOptions::new();
    options.set_enable_high_accuracy(true);
    options.set_timeout(20_000);
    geolocation
        .get_current_position_with_error_callback_and_options(
            on_success.unchecked_ref(),
            Some(on_error.unchecked_ref()),
            &options,
        )
        .map_err(|error| format!("{error:?}"))
}
