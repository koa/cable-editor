use crate::components::select::Select;
use crate::{
    components::{dialog::confirm_delete, unsaved::Unsaved},
    error::FrontendError,
    geo::{
        coordinates::{Check, CoordinateSystem, check, parse_number, split_pair},
        map::{MapHolder, div_icon, duct_line, fit_points, lat_lng, schacht_marker},
    },
    graphql::authenticated::{
        Genauigkeit, GeoPoint, IdOrNew,
        current_user::Role,
        map::{MapData, fetch_map_data},
        schacht_properties::{
            ConvertedPoint, GeoPointInput, Lv95Input, PositionInput, SchachtChoices, SchachtInput,
            SchachtProperties, convert_point, create_schacht, delete_schacht,
            fetch_schacht_choices, fetch_schacht_properties, update_schacht,
        },
        schacht_types::type_icon,
    },
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, get_role, navigate, toast_error, toast_success},
};
use cable_editor_common::ObjectKind;
use gloo_timers::callback::Timeout;
use leaflet::{DragEvents, Marker, MarkerOptions, MouseEvent};
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Button, ButtonVariant, Form, FormGroup, Icon, Spinner,
    TextInput, ToggleGroup, ToggleGroupItem,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
// The stable (older) names of GeolocationPosition and GeolocationPositionError
use web_sys::{Position, PositionError, PositionOptions};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

/// Waiting time after typing before the position is converted and shown on the map.
const CONVERT_DELAY_MS: u32 = 400;
/// Zoom when showing the position; the tiles of the map end at 18.
const POSITION_ZOOM: f64 = 18.0;

/// Name, type, owner, position and Lagebestimmung of a Schacht, or a new one
/// (`IdOrNew::Temporary`). The position can be set on the map (click, drag the marker), typed in
/// LV95 or WGS84 or taken from the device's location; the Lagebestimmung stays as set (ungenau
/// unless set explicitly), also for a position from the device. The map also shows all other
/// ducts and Schächte for context, so the position can be judged relative to them; the one being
/// placed stands out with its own marker style (`.schacht-properties__marker`). Readers see the
/// same form read-only. Renders only its form and map, no `PageLayout`: for an existing Schacht
/// it's embedded in `CabinetOverview`'s page, for a new one (`PlanView::NewCabinet`) the router
/// wraps it in one itself.
pub struct CabinetProperties {
    /// The Schacht as stored (missing for a new one) and the types and owners to choose from
    loaded: Option<(Option<SchachtProperties>, SchachtChoices)>,
    /// All ducts and Schächte, drawn for context (`draw_base`)
    map_data: Option<MapData>,
    error: Option<FrontendError>,
    name: String,
    type_id: Option<i32>,
    owner: Option<i32>,
    lagebestimmung: Genauigkeit,
    system: CoordinateSystem,
    /// The coordinate fields as typed
    first: String,
    second: String,
    position: PositionState,
    /// Accuracy of the device's location, while it is the position
    accuracy: Option<f64>,
    locating: bool,
    saving: bool,
    map: MapHolder,
    /// The position on the map, moved rather than replaced
    marker: Option<Marker>,
    /// Number of the last conversion request, older answers are dropped
    conversion: u32,
    _convert_delay: Option<Timeout>,
    /// A new Schacht is only unsaved once something was entered
    touched: bool,
    unsaved: Unsaved,
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
    Loaded(Option<SchachtProperties>, SchachtChoices, MapData),
    LoadError(FrontendError),
    SetName(String),
    SetType(Option<i32>),
    SetOwner(Option<i32>),
    SetLagebestimmung(Option<Genauigkeit>),
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
    /// An existing Schacht was saved, so the embedding page can refresh what it shows of it
    /// itself (its name, "Geändert"); not called after creating a new one, which navigates away
    pub onsaved: Callback<()>,
}

impl Component for CabinetProperties {
    type Message = Msg;
    type Properties = CabinetPropertiesProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::fetch(ctx);
        Self {
            loaded: None,
            map_data: None,
            error: None,
            name: String::new(),
            type_id: None,
            owner: None,
            lagebestimmung: Genauigkeit::Ungenau,
            system: CoordinateSystem::default(),
            first: String::new(),
            second: String::new(),
            position: PositionState::Empty,
            accuracy: None,
            locating: false,
            saving: false,
            map: MapHolder::default(),
            marker: None,
            conversion: 0,
            _convert_delay: None,
            touched: false,
            unsaved: Unsaved::new(ctx.link()),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if matches!(
            msg,
            Msg::SetName(_)
                | Msg::SetType(_)
                | Msg::SetOwner(_)
                | Msg::SetLagebestimmung(_)
                | Msg::SetFirst(_)
                | Msg::SetSecond(_)
                | Msg::Picked(..)
        ) {
            self.touched = true;
        }
        match msg {
            Msg::Loaded(schacht, choices, map_data) => {
                self.error = None;
                self.take_stored(ctx, schacht.as_ref());
                if schacht.is_none() {
                    // A new Schacht: the default owner
                    self.owner = choices.owners.iter().find(|o| o.is_default).map(|o| o.id);
                    // Show the project's area instead of all of Switzerland, unless this is
                    // the first Schacht
                    if let Some(map) = self.map.map() {
                        fit_points(
                            map,
                            map_data
                                .schaechte
                                .iter()
                                .filter_map(|s| s.location.as_ref()),
                        );
                    }
                }
                self.draw_base(ctx, &map_data);
                self.loaded = Some((schacht, choices));
                self.map_data = Some(map_data);
            }
            Msg::LoadError(error) => self.error = Some(error),
            Msg::SetName(name) => self.name = name,
            Msg::SetType(type_id) => self.type_id = type_id,
            Msg::SetOwner(owner) => self.owner = owner,
            Msg::SetLagebestimmung(lagebestimmung) => {
                if let Some(lagebestimmung) = lagebestimmung {
                    self.lagebestimmung = lagebestimmung;
                }
            }
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
                    toast_error(ctx.link(), "Standort konnte nicht geladen werden", error);
                }
            }
            Msg::LocateFailed(error) => {
                self.locating = false;
                toast_error(ctx.link(), "Standort konnte nicht geladen werden", error);
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
                    // Stays here (embedded in the Schacht's overview); the form and the
                    // embedding page both take the saved values as the new baseline
                    Ok(schacht) => {
                        toast_success(ctx.link(), "Schacht gespeichert");
                        self.take_stored(ctx, Some(&schacht));
                        if let Some(loaded) = &mut self.loaded {
                            loaded.0 = Some(schacht);
                        }
                        ctx.props().onsaved.emit(());
                    }
                    Err(error) => {
                        toast_error(ctx.link(), "Schacht konnte nicht gespeichert werden", error)
                    }
                }
            }
            Msg::Created(result) => {
                self.saving = false;
                match result {
                    Ok(id) => {
                        toast_success(ctx.link(), "Schacht angelegt");
                        navigate(
                            ctx.link(),
                            ctx.props().plan_id,
                            PlanView::Cabinet {
                                id,
                                view: CabinetView::Overview,
                            },
                        );
                    }
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
                            Ok(()) => {
                                toast_success(&scope, "Schacht gelöscht");
                                navigate(&scope, plan_id, PlanView::ListOfCabinets)
                            }
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
        let content = if let Some(error) = &self.error {
            error.into_prop_value()
        } else if let Some((stored, choices)) = &self.loaded {
            if stored.is_none()
                && let IdOrNew::Id(id) = ctx.props().cabinet
            {
                (&FrontendError::not_found(ObjectKind::Schacht, id)).into_prop_value()
            } else {
                self.view_form(ctx, choices)
            }
        } else {
            html!(<Spinner/>)
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <div class="map-layout">
                <div class="schacht-properties__form">{content}</div>
                <div class="map-layout__map" ref={self.map.container()}/>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        self.unsaved.set(self.has_unsaved());
        if !first_render {
            return;
        }
        if let Err(error) = self.map.create() {
            self.error = Some(error);
            return;
        }
        if let Some(map) = self.map.map()
            && self.can_edit(ctx)
        {
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
        if let PositionState::Valid(point) = self.position {
            self.show_marker(ctx, point.wgs84, true);
        }
    }
}

impl CabinetProperties {
    fn fetch(ctx: &Context<Self>) {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let cabinet = ctx.props().cabinet;
        spawn_local(async move {
            let loaded = match cabinet {
                IdOrNew::Id(id) => fetch_schacht_properties(credentials.as_ref(), id).await,
                IdOrNew::Temporary(_) => fetch_schacht_choices(credentials.as_ref())
                    .await
                    .map(|choices| (None, choices)),
            };
            let result = match loaded {
                Ok((schacht, choices)) => fetch_map_data(credentials.as_ref())
                    .await
                    .map(|map_data| (schacht, choices, map_data)),
                Err(error) => Err(error),
            };
            scope.send_message(match result {
                Ok((schacht, choices, map_data)) => Msg::Loaded(schacht, choices, map_data),
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
        self.owner = schacht.map(|s| s.owner.id);
        // Ungenau unless set explicitly
        self.lagebestimmung = schacht.map_or(Genauigkeit::Ungenau, |s| s.lagebestimmung);
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
        let Some(map) = self.map.map() else {
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

    /// Draws all other ducts and Schächte for context, so the position being placed (its own
    /// marker from `show_marker`) can be judged relative to them. Excludes the Schacht being
    /// placed itself, which would otherwise sit right under its own marker.
    fn draw_base(&mut self, ctx: &Context<Self>, data: &MapData) {
        if self.map.map().is_none() {
            return;
        }
        let current_id = match ctx.props().cabinet {
            IdOrNew::Id(id) => Some(id),
            IdOrNew::Temporary(_) => None,
        };
        let mut layers: Vec<leaflet::Layer> = Vec::new();
        for line in data.ducts.iter().filter_map(|duct| duct.line.as_deref()) {
            layers.push(duct_line(line, "map-view__duct").unchecked_into());
        }
        for schacht in &data.schaechte {
            if Some(schacht.id) == current_id {
                continue;
            }
            if let Some(location) = schacht.location {
                layers.push(
                    schacht_marker(&schacht.name, location, type_icon(&schacht.typ), || {})
                        .unchecked_into(),
                );
            }
        }
        self.map.replace_layers(layers);
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
        let Some(owner_id) = self.owner else {
            return;
        };
        let input = SchachtInput {
            name: self.name.trim().to_string(),
            type_id: self.type_id,
            position,
            owner_id,
            lagebestimmung: self.lagebestimmung,
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

    fn has_unsaved(&self) -> bool {
        match &self.loaded {
            Some((None, _)) => self.touched,
            _ => self.has_changes(),
        }
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
            || self.owner != Some(stored.owner.id)
            || self.lagebestimmung != stored.lagebestimmung
    }

    fn view_form(&self, ctx: &Context<Self>, choices: &SchachtChoices) -> Html {
        let readonly = !self.can_edit(ctx);
        let link = ctx.link();
        let types = &choices.types;

        let type_options = types
            .iter()
            .map(|t| {
                (
                    t.id,
                    t.name.clone().unwrap_or_else(|| format!("Typ {}", t.id)),
                )
            })
            .collect::<Box<[_]>>();
        // A select ignores its `disabled`, so readers get a text field
        let type_field = if readonly {
            let value = self
                .type_id
                .and_then(|id| types.iter().find(|t| t.id == id))
                .and_then(|t| t.name.clone())
                .unwrap_or_default();
            html!(<TextInput {value} readonly=true/>)
        } else {
            html! {
                <Select<i32>
                    value={self.type_id}
                    onchange={link.callback(Msg::SetType)}
                    placeholder=" - "
                    options={type_options}
                />
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
                <FormGroup label="Eigentümer" required={!readonly}>
                    {self.view_owner(ctx, choices)}
                </FormGroup>
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
                }
                <FormGroup label="Lagebestimmung">{self.view_lagebestimmung(ctx)}</FormGroup>
                if !readonly {
                    {self.view_actions(ctx)}
                }
            </Form>
        }
    }

    /// The owner; a new Schacht gets the default one.
    fn view_owner(&self, ctx: &Context<Self>, choices: &SchachtChoices) -> Html {
        if !self.can_edit(ctx) {
            let value = choices
                .owners
                .iter()
                .find(|o| Some(o.id) == self.owner)
                .map(|o| o.name.clone())
                .unwrap_or_default();
            return html!(<TextInput {value} readonly=true/>);
        }
        let options = choices
            .owners
            .iter()
            .map(|o| (o.id, o.name.clone()))
            .collect::<Box<[_]>>();
        html! {
            <Select<i32> value={self.owner} onchange={ctx.link().callback(Msg::SetOwner)} placeholder=" - " {options}/>
        }
    }

    /// The accuracy of the position for the Leitungskataster; ungenau unless set explicitly.
    fn view_lagebestimmung(&self, ctx: &Context<Self>) -> Html {
        if !self.can_edit(ctx) {
            return html!(<TextInput value={self.lagebestimmung.title()} readonly=true/>);
        }
        let options = Genauigkeit::ALL
            .iter()
            .map(|g| (*g, g.title().to_string()))
            .collect::<Box<[_]>>();
        html! {
            <Select<Genauigkeit>
                value={Some(self.lagebestimmung)}
                onchange={ctx.link().callback(Msg::SetLagebestimmung)}
                {options}
            />
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
        let can_save = position_ok
            && !self.name.trim().is_empty()
            && self.owner.is_some()
            && self.has_changes();
        let delete = (!is_new && get_role(ctx.link()) >= Role::Admin).then(|| {
            let onclick = confirm_delete(ctx.link(), "Schacht", self.name.trim(), ctx.link().callback(|()| Msg::Delete));
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
        lv95: schacht.position?,
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
