use crate::graphql::authenticated::list_cables::delete_cable;
use crate::graphql::authenticated::select_duct::DuctListEntry;
use crate::pages::duct::select_duct::SelectDuct;
use crate::pages::router::{AppRoute, PlanView};
use crate::util::get_credentials;
use crate::{
    components::table::ListModel,
    error::FrontendError,
    graphql::authenticated::cable_details::{
        CableDetails, CableDuct, CablePath, CablePathSegment, CableSegmentEndSchacht,
        PotentialDuct, UpdateCableStructure,
    },
    util,
};
use log::{error, info};
use patternfly_yew::prelude::{
    Alert, AlertType, Backdrop, Backdropper, Bullseye, Button, ButtonVariant, Cell, CellContext,
    ExpansionState, Form, FormGroup, Icon, InputState, LabelIcon, MemoizedTableModel, Modal,
    ModalVariant, Panel, PanelVariant, SimpleList, SimpleListItem, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableMode, TextInput, Toolbar, ToolbarContent,
    ToolbarItem,
};
use std::{cell::RefCell, collections::HashMap, mem, rc::Rc};
use yew::{
    BaseComponent, Callback, Component, Context, Html, Properties, function_component, html,
    html::IntoPropValue, html::Scope, html_nested, platform::spawn_local,
};
use yew_nested_router::prelude::{RouterContext, State};
use yew_oauth2::prelude::OAuth2Context;

#[derive(Debug, Clone, PartialEq)]
enum DuctPathEntry {
    Schacht {
        schacht: CableSegmentEndSchacht,
        pos: f64,
        on_extend: Option<Callback<CableSegmentEndSchacht>>,
    },
    Duct {
        duct: CableDuct,
        on_remove: Option<Callback<i32>>,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CablePathColumn {
    Schacht,
    Length,
    Position,
    Actions,
}

impl TableEntryRenderer<CablePathColumn> for DuctPathEntry {
    fn render_cell(&self, context: CellContext<'_, CablePathColumn>) -> Cell {
        match context.column {
            CablePathColumn::Schacht => match self {
                DuctPathEntry::Schacht { schacht, .. } => {
                    Cell::new(schacht.name.as_str().into_prop_value())
                }
                _ => Cell::default(),
            },
            CablePathColumn::Length => if let DuctPathEntry::Duct { duct, .. } = self {
                duct.length
            } else {
                None
            }
            .map(|l| Cell::new(format!("{l:.1} m").into_prop_value()))
            .unwrap_or_default(),
            CablePathColumn::Position => {
                if let DuctPathEntry::Schacht { pos, .. } = self {
                    Cell::new(format!("{pos:.1} m").into_prop_value())
                } else {
                    Cell::default()
                }
            }
            CablePathColumn::Actions => match self {
                DuctPathEntry::Duct {
                    duct,
                    on_remove: Some(on_remove),
                } => {
                    let duct_id = duct.id;
                    let onclick_remove = {
                        let on_remove = on_remove.clone();
                        Callback::from(move |_| {
                            on_remove.emit(duct_id);
                        })
                    };

                    Cell::new(html! {
                        <Toolbar>
                            <ToolbarContent>
                                <ToolbarItem>
                                    <Button label="Entfernen" icon={Icon::MinusCircle} onclick={onclick_remove}/>
                                </ToolbarItem>
                            </ToolbarContent>
                        </Toolbar>
                    })
                }
                DuctPathEntry::Schacht {
                    schacht,
                    pos,
                    on_extend: Some(on_extend),
                } => {
                    let onclick_extend = {
                        let on_extend = on_extend.clone();
                        let schacht = schacht.clone();
                        Callback::from(move |_| {
                            on_extend.emit(schacht.clone());
                        })
                    };

                    Cell::new(html! {
                        <Toolbar>
                            <ToolbarContent>
                                <ToolbarItem>
                                    <Button label="Verlängern" icon={Icon::PlusCircle} onclick={onclick_extend} />
                                </ToolbarItem>
                            </ToolbarContent>
                        </Toolbar>
                    })
                }

                _ => Cell::default(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
struct DuctSelectionDialogProperties {
    pub available_ducts: Vec<PotentialDuct>,
    #[prop_or_default]
    pub on_select: Callback<PotentialDuct>,
    #[prop_or_default]
    pub on_cancel: Callback<()>,
}

#[derive(Debug, Clone, PartialEq, Properties)]
struct DeleteConfirmationDialogProperties {
    #[prop_or_default]
    pub on_confirm: Callback<()>,
    #[prop_or_default]
    pub on_cancel: Callback<()>,
}

#[function_component]
fn DeleteConfirmationDialog(props: &DeleteConfirmationDialogProperties) -> Html {
    let footer = html! {
        <>
            <Button
                label="Ja"
                onclick={props.on_confirm.reform(|_| ())}
                variant={ButtonVariant::Danger}
            />
            <Button
                label="Nein"
                onclick={props.on_cancel.reform(|_| ())}
                variant={ButtonVariant::Secondary}
            />
        </>
    };
    html! {
        <Bullseye>
            <Modal
                title="Bestätigung"
                variant={ModalVariant::Small}
                {footer}
            >
                <p>{"Wirklich löschen?"}</p>
            </Modal>
        </Bullseye>
    }
}

#[function_component]
fn DuctSelectionDialog(props: &DuctSelectionDialogProperties) -> Html {
    let footer = html! {
    <Button
        label="Abbrechen"
        onclick={props.on_cancel.reform(|_| ())}
        variant={ButtonVariant::Secondary}
    />};
    html! {
        <Bullseye>
            <Modal
                title="Trasse auswählen"
                variant={ModalVariant::Small}
                {footer}
            >
                    {
                        if props.available_ducts.is_empty() {
                            html! { <p>{"Keine verfügbaren Trassen gefunden"}</p> }
                        } else {
                            html! {
                                <SimpleList>
                                    {
                                        for props.available_ducts.iter().map(|potential_duct| {
                                            let duct_id = potential_duct.duct.id;
                                            let duct_desc = potential_duct.duct.description.clone()
                                                .filter(|v| !v.is_empty())
                                                .unwrap_or_else(|| format!("Trasse #{}", duct_id));
                                            let schacht_name = potential_duct.schacht.name.clone();
                                            let onclick = {
                                                let on_select = props.on_select.clone();
                                                let duct = potential_duct.clone();
                                                Callback::from(move |_| {
                                                    on_select.emit(duct.clone());
                                                })
                                            };
                                            html_nested! {
                                                <SimpleListItem {onclick}>
                                                    {format!("{} → {}", duct_desc, schacht_name)}
                                                </SimpleListItem>
                                            }
                                        })
                                    }
                                </SimpleList>
                            }
                        }
                    }
            </Modal>
        </Bullseye>
    }
}

#[derive(Debug, Default)]
pub struct EditCable {
    state: DataState,
    cable_name: String,
    bundle_count: String,
    fiber_count: String,
    saving: bool,
    path: Option<CablePath>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<CablePathColumn>>>>,
}
#[derive(Debug, Default)]
pub enum DataState {
    Data(CableDetails),
    Error(FrontendError),
    #[default]
    Pending,
    NotFound,
}
pub enum Msg {
    Data(CableDetails),
    Error(FrontendError),
    NotFound,
    SetName(String),
    SetBundleCount(String),
    SetFiberCount(String),
    Save,
    AppendSegment {
        end: PathEnd,
        duct: CableDuct,
        other_schacht: CableSegmentEndSchacht,
    },
    RemoveSegment {
        end: PathEnd,
    },
    InitFirstSegment {
        schacht_a: CableSegmentEndSchacht,
        duct: CableDuct,
        schacht_z: CableSegmentEndSchacht,
    },
    RemoveEntry,
}
#[derive(Copy, Clone, Debug)]
pub enum PathEnd {
    Front,
    Tail,
}
impl Component for EditCable {
    type Message = Msg;
    type Properties = EditCableProperties;

    fn create(ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.cable_name = data.name.clone();
                self.bundle_count = data.bundle_count.to_string();
                self.fiber_count = data.fiber_count.to_string();
                self.path = data.path.clone();
                self.state = DataState::Data(data);
                self.saving = false;
                true
            }
            Msg::Error(error) => {
                self.state = DataState::Error(error);
                true
            }
            Msg::NotFound => {
                self.state = DataState::NotFound;
                true
            }
            Msg::SetName(name) => {
                self.cable_name = name;
                true
            }
            Msg::SetBundleCount(count) => {
                self.bundle_count = count;
                true
            }
            Msg::SetFiberCount(count) => {
                self.fiber_count = count;
                true
            }
            Msg::Save => {
                self.saving = true;
                if let DataState::Data(data) = &self.state {
                    let scope = ctx.link().clone();
                    let cable_name = self.cable_name.clone();
                    let bundle_count = self.bundle_count.clone();
                    let string = self.fiber_count.clone();
                    let path = self.path.clone();
                    let cable_details = data.clone();
                    update_cable(scope, cable_name, bundle_count, string, path, cable_details);
                }
                true
            }
            Msg::AppendSegment {
                end,
                duct,
                other_schacht,
            } => {
                if let Some(path) = self.path.as_mut() {
                    match end {
                        PathEnd::Front => {
                            let far_schacht = mem::replace(&mut path.near_schacht, other_schacht);
                            path.segments
                                .insert(0, CablePathSegment { duct, far_schacht })
                        }
                        PathEnd::Tail => {
                            path.segments.push(CablePathSegment {
                                duct,
                                far_schacht: other_schacht,
                            });
                        }
                    }
                }
                true
            }
            Msg::RemoveSegment { end } => {
                if let Some(mut path) = self.path.take() {
                    match end {
                        PathEnd::Front => {
                            let segments = &mut path.segments;

                            let segment_count = segments.len();
                            if segment_count > 0 {
                                path.near_schacht = segments.remove(0).far_schacht;
                            } else {
                                path.segments.clear();
                            }
                        }
                        PathEnd::Tail => {
                            let segments = &mut path.segments;
                            let segment_count = segments.len();
                            if segment_count > 1 {
                                segments.remove(segment_count - 1);
                            }
                        }
                    }
                    if !path.segments.is_empty() {
                        self.path = Some(path);
                    }
                }
                true
            }
            Msg::InitFirstSegment {
                schacht_a,
                duct,
                schacht_z,
            } => {
                if self.path.is_none() {
                    self.path = Some(CablePath {
                        near_schacht: schacht_a,
                        segments: vec![CablePathSegment {
                            duct,
                            far_schacht: schacht_z,
                        }],
                    });
                }
                true
            }
            Msg::RemoveEntry => {
                if let DataState::Data(data) = &self.state {
                    let id = data.id;
                    if let (Some((rt, _)), Some((credentials, _))) = (
                        ctx.link()
                            .context::<RouterContext<AppRoute>>(Callback::noop()),
                        ctx.link().context::<OAuth2Context>(Callback::noop()),
                    ) {
                        let plan_id = ctx.props().plan_id;
                        spawn_local(async move {
                            if let Err(err) = delete_cable(Some(&credentials), id).await {
                                error!("Cannot delete cable: {err}")
                            } else {
                                rt.push(AppRoute::Plan {
                                    plan_id,
                                    view: PlanView::ListOfCables,
                                });
                            }
                        });
                    }
                }
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().cable_id != old_props.cable_id {
            Self::fetch_data(&ctx);
        }
        false
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match &self.state {
            DataState::Data(data) => {
                let mut has_changes = false;
                let mut has_error = false;
                let name_edit = {
                    let value = self.cable_name.clone();
                    let onchange = ctx.link().callback(Msg::SetName);
                    let state = if value == data.name {
                        InputState::Default
                    } else if value.is_empty() {
                        has_error = true;
                        InputState::Error
                    } else {
                        has_changes = true;
                        InputState::Success
                    };
                    html! {<TextInput {value} {onchange} {state}/>}
                };
                let bundle_count_edit = {
                    let value = self.bundle_count.clone();
                    let onchange = ctx.link().callback(Msg::SetBundleCount);
                    let state = match value.parse::<i32>() {
                        Ok(count) if count == data.bundle_count => InputState::Default,
                        Ok(_) => {
                            has_changes = true;
                            InputState::Success
                        }
                        Err(_) => {
                            has_error = true;
                            InputState::Error
                        }
                    };
                    html! {<TextInput {value} {onchange} {state}/>}
                };
                let fiber_count_edit = {
                    let value = self.fiber_count.clone();
                    let onchange = ctx.link().callback(Msg::SetFiberCount);
                    let state = match value.parse::<i32>() {
                        Ok(count) if count == data.fiber_count => InputState::Default,
                        Ok(count) => {
                            if count == data.fiber_count {
                                InputState::Default
                            } else if count > 24 {
                                has_error = true;
                                InputState::Error
                            } else {
                                has_changes = true;
                                InputState::Success
                            }
                        }
                        Err(_) => {
                            has_error = true;
                            InputState::Error
                        }
                    };
                    html! {<TextInput {value} {onchange} {state}/>}
                };

                let path_changed = data
                    .path
                    .as_ref()
                    .map(|p| p.duct_sequence().collect::<Vec<_>>())
                    != self
                        .path
                        .as_ref()
                        .map(|p| p.duct_sequence().collect::<Vec<_>>());
                has_changes |= path_changed;

                let scope = ctx.link().clone();

                let save_button = {
                    let on_save = Callback::from(move |_| scope.send_message(Msg::Save));
                    if self.saving {
                        html!(<Spinner/>)
                    } else {
                        let scope = ctx.link().clone();
                        let delete_button = scope.context::<Backdropper>(Callback::noop()).map(
                            |(backdropper, _)| {
                                let onclick = {
                                    let backdropper = backdropper.clone();
                                    let scope = scope.clone();
                                    Callback::from(move |_| {
                                        let backdropper = backdropper.clone();
                                        let scope = scope.clone();
                                        let on_confirm = {
                                            let backdropper = backdropper.clone();
                                            let scope = scope.clone();
                                            Callback::from(move |_| {
                                                backdropper.close();
                                                scope.send_message(Msg::RemoveEntry);
                                            })
                                        };
                                        let on_cancel = {
                                            let backdropper = backdropper.clone();
                                            Callback::from(move |_| {
                                                backdropper.close();
                                            })
                                        };
                                        backdropper.open(Backdrop::new(html! {
                                            <DeleteConfirmationDialog {on_confirm} {on_cancel} />
                                        }));
                                    })
                                };
                                html! {
                                <Button variant={ButtonVariant::DangerSecondary}
                                    label="Löschen"
                                    {onclick}
                                    disabled={has_changes}
                                />
                                }
                            },
                        );
                        html! {
                            <>
                            <Button variant={ButtonVariant::Primary}
                                label="Speichern"
                                onclick={on_save}
                                disabled={!has_changes || has_error}
                            />
                            {delete_button}
                            </>
                        }
                    }
                };

                let scope = ctx.link().clone();

                let cable_path = self.path.as_ref().map(|path| {
                    let mut entries =
                        Vec::<DuctPathEntry>::with_capacity(1 + path.segments.len() * 2);
                    let mut current_pos = 0.0;

                    entries.push(DuctPathEntry::Schacht {
                        schacht: path.near_schacht.clone(),
                        pos: current_pos,
                        on_extend: None,
                    });

                    for segment in path.segments.iter() {
                        entries.push(DuctPathEntry::Duct {
                            duct: segment.duct.clone(),
                            on_remove: None,
                        });

                        if let Some(l) = segment.duct.length {
                            current_pos += l;
                        }

                        entries.push(DuctPathEntry::Schacht {
                            schacht: segment.far_schacht.clone(),
                            pos: current_pos,
                            on_extend: None,
                        });
                    }
                    let credentials = get_credentials(ctx.link());
                    if let Some((backdrop, _)) = ctx.link().context::<Backdropper>(Callback::noop()) {
                        for (idx, end) in [(0, PathEnd::Front), (entries.len() - 1, PathEnd::Tail)] {
                            if let Some(first_schacht) = entries.get_mut(idx) && let DuctPathEntry::Schacht { on_extend, schacht, .. } = first_schacht {
                                let backdrop = backdrop.clone();
                                let schacht = schacht.clone();
                                let scope = scope.clone();
                                let credentials = credentials.clone();
                                *on_extend = Some(Callback::from(move |_| {
                                    let backdrop = backdrop.clone();
                                    let schacht = schacht.clone();
                                    let scope = scope.clone();
                                    let credentials = credentials.clone();
                                    spawn_local(async move {
                                        match schacht.fetch_connected_ducts(credentials.as_ref()).await {
                                            Ok(available_ducts) => {
                                                let on_select = {
                                                    let backdrop = backdrop.clone();
                                                    scope.callback(move |PotentialDuct { duct, schacht }| {
                                                        backdrop.close();
                                                        Msg::AppendSegment {
                                                            end,
                                                            duct,
                                                            other_schacht: schacht,
                                                        }
                                                    })
                                                };

                                                backdrop.open(Backdrop::new(html! {
                                                    <DuctSelectionDialog
                                                        {available_ducts}
                                                        {on_select}
                                                    />
                                                }));
                                            }
                                            Err(e) => {
                                                info!("Error fetching connected ducts: {e:?}");
                                            }
                                        }
                                    });
                                }));
                            }
                        }
                    }
                    for (idx, end) in [(1, PathEnd::Front), (entries.len() - 2, PathEnd::Tail)] {
                        if let Some(duct) = entries.get_mut(idx) && let DuctPathEntry::Duct { on_remove, .. } = duct {
                            *on_remove = Some(scope.callback(move |_| Msg::RemoveSegment { end }));
                        }
                    }

                    entries
                }).map(|cable_path| MemoizedTableModel::new(Rc::new(cable_path)))
                    .map(|d| ListModel::new(d, self.table_state.clone()))
                    .map(|entries| {
                        let table_header = html_nested! {
                            <TableHeader<CablePathColumn>>
                                <TableColumn<CablePathColumn> label="Name" index={CablePathColumn::Schacht} />
                                <TableColumn<CablePathColumn> label="Position" index={CablePathColumn::Position} />
                                <TableColumn<CablePathColumn> label="Segmentlänge" index={CablePathColumn::Length} />
                                <TableColumn<CablePathColumn> index={CablePathColumn::Actions} />
                            </TableHeader<CablePathColumn>>
                        };
                        let label_icon = if path_changed {
                            LabelIcon::Children(Icon::CheckCircle.as_html())
                        } else {
                            LabelIcon::None
                        };
                        html! {
                            <FormGroup label="Kabelweg" {label_icon}>
                                <Table<CablePathColumn, ListModel<CablePathColumn, MemoizedTableModel<DuctPathEntry>>>
                                    class="pf-m-warning"
                                    mode={TableMode::Compact}
                                    grid={TableGridMode::Medium}
                                    header={table_header}
                                    {entries}
                                />
                            </FormGroup>
                        }
                    }).unwrap_or_else(|| {
                    if let Some((backdrop, _)) = ctx.link().context::<Backdropper>(Callback::noop()) {
                        let scope=ctx.link().clone();
                        let onclick = Callback::from(move |_| {
                            let scope=scope.clone();
                            let on_select={
                                let backdrop=backdrop.clone();
                                Callback::from(move |details: DuctListEntry|{
                                    scope.send_message(Msg::InitFirstSegment{
                                        schacht_a: details.schacht_a,
                                        duct: CableDuct {
                                            id: details.id,
                                            description: details.description,
                                            length: details.length,
                                        },
                                        schacht_z: details.schacht_z,
                                    });
                                    backdrop.close();
                                })
                            };
                            let onclick={
                                let backdrop=backdrop.clone();
                                Callback::from(move |_|{
                                    backdrop.close();
                                })
                            };

                            let footer = html! {
                                <Button
                                    label="Abbrechen"
                                    {onclick}
                                    variant={ButtonVariant::Secondary}
                                />};
                            backdrop.open(Backdrop::new(html! {
                                        <Bullseye>
                                            <Modal
                                                title="Trasse auswählen"
                                                variant={ModalVariant::Small}
                                                {footer}>
                                                <SelectDuct {on_select}/>
                                            </Modal>
                                        </Bullseye>
                            }));
                        });
                        html!(<Button {onclick} variant={ButtonVariant::Primary} label="Trasse auswählen"/>)
                    } else {
                        "Kein Backdrop".into()
                    }
                });

                html! {
                    <Form>
                        <FormGroup label="Name">{name_edit}</FormGroup>
                        <FormGroup label="Anzahl der Bündel">{bundle_count_edit}</FormGroup>
                        <FormGroup label="Anzahl der Fasern">{fiber_count_edit}</FormGroup>
                        {cable_path}
                        <FormGroup>{save_button}</FormGroup>
                    </Form>
                }
            }
            DataState::Error(error) => error.into_prop_value(),
            DataState::Pending => {
                html!(<Spinner/>)
            }
            DataState::NotFound => {
                let cable_id = ctx.props().cable_id;
                format!("Kabel {cable_id} nicht gefunden").into_prop_value()
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            Self::fetch_data(ctx);
        }
    }
}

fn update_cable(
    scope: Scope<EditCable>,
    cable_name: String,
    bundle_count: String,
    fiber_count: String,
    path: Option<CablePath>,
    current_details: CableDetails,
) {
    let credentials = scope
        .context::<OAuth2Context>(Callback::noop())
        .map(|(c, _)| c);
    let update_cable = if cable_name == current_details.name {
        None
    } else {
        Some(cable_name)
    };
    let update_structure = Option::zip(
        bundle_count.parse::<i32>().ok(),
        fiber_count.parse::<i32>().ok(),
    )
    .and_then(|(bundle_count, fiber_count)| {
        if (fiber_count == current_details.fiber_count
            && bundle_count == current_details.bundle_count)
        {
            None
        } else {
            Some(UpdateCableStructure {
                bundle_count,
                fiber_count,
            })
        }
    });
    let target_path = path.map(|p| p.duct_sequence().collect::<Vec<_>>());
    let current_path = current_details
        .path
        .as_ref()
        .map(|p| p.duct_sequence().collect::<Vec<_>>());
    let path_update = if (current_path != target_path) {
        Some(target_path.unwrap_or_default())
    } else {
        None
    };
    spawn_local(async move {
        scope.send_message(
            match CableDetails::update_cable(
                credentials.as_ref(),
                current_details.id,
                update_cable,
                update_structure,
                path_update,
            )
            .await
            {
                Ok(Some(updated)) => Msg::Data(updated),
                Err(e) => Msg::Error(e),
                Ok(None) => Msg::NotFound,
            },
        );
    });
}

impl EditCable {
    fn fetch_data(ctx: &Context<EditCable>) {
        let scope = ctx.link().clone();
        let cable_id = ctx.props().cable_id;
        spawn_local(async move {
            if let Some((credentials, _)) = scope.context::<OAuth2Context>(Callback::noop()) {
                scope.send_message(
                    match CableDetails::fetch(Some(&credentials), cable_id).await {
                        Ok(Some(data)) => Msg::Data(data),
                        Err(error) => Msg::Error(error),
                        Ok(None) => Msg::NotFound,
                    },
                );
            } else {
                error!("Not logged in")
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct EditCableProperties {
    pub plan_id: i32,
    pub cable_id: i32,
}
