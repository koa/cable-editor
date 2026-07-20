use crate::graphql::authenticated::cable_details::{CableDuct, CableSegmentEndSchacht};
use crate::{
    error::FrontendError,
    graphql::authenticated::cable_details::{CableDetails, UpdateCableStructure},
};
use patternfly_yew::prelude::{
    Button, ButtonVariant, Cell, CellContext, Form, FormGroup, MemoizedTableModel, Spinner, Table,
    TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableHeaderSortBy, TableMode,
    TextInput, UseTableData, use_table_data,
};
use std::sync::Arc;
use yew::{
    Callback, Html, HtmlResult, Properties, Suspense, function_component, html,
    html::IntoPropValue, html_nested, platform::spawn_local, props, suspense::use_future_with,
    use_memo, use_state,
};
use yew_oauth2::hook::use_auth_state;

#[derive(Debug, Clone, PartialEq)]
enum DuctPathEntry {
    Schacht {
        schacht: CableSegmentEndSchacht,
        pos: f64,
    },
    Duct(CableDuct),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CablePathColumn {
    Schacht,
    Length,
    Position,
}

impl TableEntryRenderer<CablePathColumn> for DuctPathEntry {
    fn render_cell(&self, context: CellContext<'_, CablePathColumn>) -> Cell {
        match context.column {
            CablePathColumn::Schacht => {
                if let DuctPathEntry::Schacht { schacht, pos } = self {
                    Cell::new(schacht.name.as_str().into_prop_value())
                } else {
                    Cell::default()
                }
            }
            CablePathColumn::Length => if let DuctPathEntry::Duct(duct) = self {
                duct.length
            } else {
                None
            }
            .map(|l| Cell::new(format!("{l:.1} m").into_prop_value()))
            .unwrap_or_default(),
            CablePathColumn::Position => {
                if let DuctPathEntry::Schacht { schacht, pos } = self {
                    Cell::new(format!("{pos:.1} m").into_prop_value())
                } else {
                    Cell::default()
                }
            }
        }
    }
}

#[function_component]
fn CableForm(props: &EditCableProperties) -> HtmlResult {
    let auth_state = use_auth_state();
    let cable_id = props.cable_id;
    let initial_cable_data = use_future_with(auth_state.clone(), |credentials| async move {
        CableDetails::fetch((*credentials).as_ref(), cable_id)
            .await
            .map_err(Arc::new)
    })?;
    let cable_data = use_state(|| {
        <Result<Option<CableDetails>, Arc<FrontendError>> as Clone>::clone(&*initial_cable_data)
    });
    let (details, error) = match &*cable_data {
        Ok(data) => (data.clone(), None),
        Err(e) => (None, Some(IntoPropValue::<Html>::into_prop_value(&**e))),
    };

    let original_data = use_state(|| details.clone());
    let name = use_state(|| details.as_ref().map(|d| d.name.clone()).unwrap_or_default());
    let bundle_count = use_state(|| {
        details
            .as_ref()
            .map(|d| d.bundle_count.to_string())
            .unwrap_or_default()
    });
    let fiber_count = use_state(|| {
        details
            .as_ref()
            .map(|d| d.fiber_count.to_string())
            .unwrap_or_default()
    });
    let cable_path = use_state(|| {
        details
            .as_ref()
            .and_then(|d| d.path.as_ref())
            .map(|path| {
                let mut entries = Vec::with_capacity(1 + path.segments.len() * 2);
                let mut current_pos = 0.0;
                entries.push(DuctPathEntry::Schacht {
                    schacht: path.near_schacht.clone(),
                    pos: current_pos,
                });
                for segment in path.segments.iter() {
                    entries.push(DuctPathEntry::Duct(segment.duct.clone()));
                    if let Some(l) = segment.duct.length {
                        current_pos += l;
                    }
                    entries.push(DuctPathEntry::Schacht {
                        schacht: segment.far_schacht.clone(),
                        pos: current_pos,
                    });
                }
                entries
            })
            .unwrap_or_default()
    });

    let on_name_change = {
        let name = name.clone();
        Callback::from(move |value: String| {
            name.set(value);
        })
    };

    let on_bundle_count_change = {
        let bundle_count = bundle_count.clone();
        Callback::from(move |value: String| {
            bundle_count.set(value);
        })
    };

    let on_fiber_count_change = {
        let fiber_count = fiber_count.clone();
        Callback::from(move |value: String| {
            fiber_count.set(value);
        })
    };

    let on_save = {
        let name = name.clone();
        let bundle_count = bundle_count.clone();
        let fiber_count = fiber_count.clone();
        let original_data = original_data.clone();
        let cable_data = cable_data.clone();
        let auth_state = auth_state.clone();
        Callback::from(move |_| {
            if let Some(original) = original_data.as_ref() {
                let update_name = if *name != original.name {
                    Some((*name).clone())
                } else {
                    None
                };

                let update_structure = if let (Ok(new_bundle_count), Ok(new_fiber_count)) =
                    (bundle_count.parse::<i32>(), fiber_count.parse::<i32>())
                {
                    if new_bundle_count != original.bundle_count
                        || new_fiber_count != original.fiber_count
                    {
                        Some(UpdateCableStructure {
                            bundle_count: new_bundle_count,
                            fiber_count: new_fiber_count,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if update_name.is_some() || update_structure.is_some() {
                    let cable_data = cable_data.clone();
                    let auth_state = auth_state.clone();
                    spawn_local(async move {
                        cable_data.set(
                            CableDetails::update_cable(
                                auth_state.as_ref(),
                                cable_id,
                                update_name,
                                update_structure,
                            )
                            .await
                            .map_err(Arc::new),
                        );
                    });
                }
            }
        })
    };

    let table_header = html_nested! {
        <TableHeader<CablePathColumn>>
            <TableColumn<CablePathColumn> label="Name" index={CablePathColumn::Schacht} />
            <TableColumn<CablePathColumn> label="Position" index={CablePathColumn::Position} />
            <TableColumn<CablePathColumn> label="Segmentlänge" index={CablePathColumn::Length} />
        </TableHeader<CablePathColumn>>
    };
    let cable_path = use_memo(cable_path.clone(), |p| (**p).clone());
    let (entries, _) = use_table_data(MemoizedTableModel::new(cable_path));

    Ok(html! {
        <Form>
            <FormGroup label="Name">
                <TextInput
                    value={(*name).clone()}
                    onchange={on_name_change}
                />
            </FormGroup>
            <FormGroup label="Anzahl der Bündel">
                <TextInput
                    value={(*bundle_count).clone()}
                    onchange={on_bundle_count_change}
                />
            </FormGroup>
            <FormGroup label="Anzahl der Fasern">
                <TextInput
                    value={(*fiber_count).clone()}
                    onchange={on_fiber_count_change}
                />
            </FormGroup>
            <FormGroup label="Kabelweg">
                <Table<CablePathColumn, UseTableData<CablePathColumn, MemoizedTableModel<DuctPathEntry>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    header={table_header}
                    {entries}
                />
            </FormGroup>
            <FormGroup>
                <Button variant={ButtonVariant::Primary}
                    label="Speichern"
                    onclick={on_save}
                />
            </FormGroup>
        </Form>
    })
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct EditCableProperties {
    pub cable_id: i32,
}

#[function_component]
pub fn EditCable(props: &EditCableProperties) -> Html {
    let fallback = html!(<Spinner/>);
    html! {
        <Suspense {fallback}>
            <CableForm ..props.clone()/>
        </Suspense>
    }
}
