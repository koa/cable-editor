use crate::graphql::authenticated::PortType;
use crate::graphql::authenticated::edit_ports::FetchedPanelWithPorts;
use crate::{
    error::FrontendError,
    graphql::authenticated::{
        IdOrNew,
        edit_ports::{FlatPortInput, update_panel_ports},
    },
    util::get_credentials,
};
use patternfly_yew::prelude::{
    ActionGroup, Button, ButtonVariant, Icon, Spinner, TextInput, ToggleGroup, ToggleGroupItem,
};
use yew::{Component, Context, Html, Properties, html, html::IntoPropValue, platform::spawn_local};

#[derive(Clone, PartialEq, Debug)]
pub struct EditablePort {
    id: IdOrNew,
    order_number: i32,
    label: Box<str>,
    port_type: PortType,
    deleted: bool,
}

pub enum Msg {
    FetchPorts,
    PortsFetched {
        ports: Vec<EditablePort>,
        panel_name: Option<Box<str>>,
        duct_name: Option<Box<str>>,
    },
    AddPort,
    UpdateLabel(usize, Box<str>),
    UpdateType(usize, PortType),
    MarkDeleted(usize),
    Save,
    Error(FrontendError),
    MoveUp(usize),
    MoveDown(usize),
}

#[derive(Properties, PartialEq, Clone)]
pub struct PortEditorProps {
    pub panel_id: i32,
}

pub struct PortEditor {
    ports: Vec<EditablePort>,
    loading: bool,
    error: Option<FrontendError>,
    panel_name: Option<Box<str>>,
    cabinet_name: Option<Box<str>>,
}
impl PortEditor {
    fn recalculate_orders(&mut self) {
        let mut current_order = 1;
        for port in &mut self.ports {
            if !port.deleted {
                port.order_number = current_order;
                current_order += 1;
            }
        }
    }
}

impl Component for PortEditor {
    type Message = Msg;
    type Properties = PortEditorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            ports: Vec::new(),
            loading: true,
            error: None,
            panel_name: None,
            cabinet_name: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchPorts => {
                self.loading = true;
                let panel_id = ctx.props().panel_id;
                let scope = ctx.link().clone();

                spawn_local(async move {
                    let credentials = get_credentials(&scope);

                    scope.send_message(
                        FetchedPanelWithPorts::fetch(credentials.as_ref(), panel_id)
                            .await
                            .map(
                                |FetchedPanelWithPorts {
                                     ports,
                                     panel_name,
                                     schacht_name,
                                 }| {
                                    (
                                        ports
                                            .into_iter()
                                            .map(|p| EditablePort {
                                                id: IdOrNew::Id(p.id),
                                                order_number: p.order_number,
                                                label: p.label.unwrap_or_default().into_boxed_str(),
                                                port_type: p.port_type.into(),
                                                deleted: false,
                                            })
                                            .collect(),
                                        panel_name.map(|s| s.into_boxed_str()),
                                        schacht_name.map(|s| s.into_boxed_str()),
                                    )
                                },
                            )
                            .map_or_else(Msg::Error, |(ports, panel_name, duct_name)| {
                                Msg::PortsFetched {
                                    ports,
                                    panel_name,
                                    duct_name,
                                }
                            }),
                    );
                });
                true
            }
            Msg::PortsFetched {
                ports,
                panel_name,
                duct_name,
            } => {
                self.ports = ports;
                self.panel_name = panel_name;
                self.cabinet_name = duct_name;
                self.loading = false;
                true
            }
            Msg::AddPort => {
                let next_order = self.ports.iter().map(|p| p.order_number).max().unwrap_or(0) + 1;
                let (label, port_type) = self
                    .ports
                    .last()
                    .map(|last| {
                        let text = last.label.trim();
                        (
                            text.rfind(|ch: char| !ch.is_numeric())
                                .and_then(|digit_pos| {
                                    let (prefix, number) = text.split_at(digit_pos + 1);
                                    number.parse::<usize>().ok().map(|n| {
                                        let new_number_str = (n + 1).to_string();
                                        (String::from(prefix)
                                            + &if number.starts_with('0') {
                                                "0".repeat(number.len() - new_number_str.len())
                                                    + new_number_str.as_ref()
                                            } else {
                                                new_number_str
                                            })
                                            .into_boxed_str()
                                    })
                                })
                                .unwrap_or_else(|| format!("Port {next_order}").into_boxed_str()),
                            last.port_type,
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            format!("Port {next_order}").into_boxed_str(),
                            PortType::Splice,
                        )
                    });

                self.ports.push(EditablePort {
                    id: IdOrNew::default(),
                    order_number: next_order,
                    label,
                    port_type,
                    deleted: false,
                });
                self.recalculate_orders();
                true
            }
            Msg::UpdateLabel(index, label) => {
                if let Some(port) = self.ports.get_mut(index) {
                    port.label = label;
                }
                true
            }
            Msg::UpdateType(index, p_type) => {
                if let Some(port) = self.ports.get_mut(index) {
                    port.port_type = p_type;
                }
                true
            }
            Msg::MarkDeleted(index) => {
                if let Some(port) = self.ports.get_mut(index) {
                    match port.id {
                        IdOrNew::Temporary(_) => {
                            self.ports.remove(index);
                        }
                        IdOrNew::Id(_) => {
                            port.deleted = true;
                        }
                    }
                }
                self.recalculate_orders();
                true
            }
            Msg::MoveUp(index) => {
                if let Some(prev_idx) = (0..index).rev().find(|&i| !self.ports[i].deleted) {
                    self.ports.swap(index, prev_idx);
                    self.recalculate_orders();
                }
                true
            }
            Msg::MoveDown(index) => {
                if let Some(next_idx) =
                    ((index + 1)..self.ports.len()).find(|&i| !self.ports[i].deleted)
                {
                    self.ports.swap(index, next_idx);
                    self.recalculate_orders();
                }
                true
            }
            Msg::Save => {
                self.loading = true;
                let scope = ctx.link().clone();
                let panel_id = ctx.props().panel_id;

                let mut changes = Vec::new();
                let mut deletes = Vec::new();

                for port in &self.ports {
                    if port.deleted {
                        // Gelöschte Ports landen nur dann in deletes, wenn sie eine DB-ID haben
                        if let IdOrNew::Id(id) = port.id {
                            deletes.push(id);
                        }
                    } else {
                        // Alle aktiven Ports (egal ob Id oder Temporary) kommen in changes
                        changes.push(FlatPortInput {
                            id: port.id.into(), // Verwendet den bestehenden From<IdOrNew> Trait
                            order: port.order_number,
                            label: port.label.to_string(),
                            port_type: port.port_type.into(),
                        });
                    }
                }

                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        update_panel_ports(credentials.as_ref(), panel_id, changes, deletes)
                            .await
                            .map_or_else(Msg::Error, |_| Msg::FetchPorts),
                    );
                });
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html!(<Spinner />);
        }

        let visible_indices: Vec<usize> = self
            .ports
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.deleted)
            .map(|(i, _)| i)
            .collect();

        let rows = visible_indices.iter().enumerate().map(|(pos, &idx)| {
            let is_first = pos == 0;
            let is_last = pos == visible_indices.len() - 1;
            let port = &self.ports[idx];
            let row_key = match &port.id {
                IdOrNew::Id(db_id) => db_id.to_string(),
                IdOrNew::Temporary(uuid) => uuid.to_string(),
            };
            let on_label_change = ctx.link().callback(move |val: String| Msg::UpdateLabel(idx, val.into_boxed_str()));

            let onselect =ctx.link().callback(move |pt |{
                    Msg::UpdateType(idx, pt)
            });

            let on_delete = ctx.link().callback(move |_| Msg::MarkDeleted(idx));
            let on_up = ctx.link().callback(move |_| Msg::MoveUp(idx));
            let on_down = ctx.link().callback(move |_| Msg::MoveDown(idx));
            let selected=port.port_type;

            html! {
                <tr class="pf-v6-c-table__tr" key={row_key}>
                    //<td class="pf-v6-c-table__td">{ port.order_number }</td>
                    <td class="pf-v6-c-table__td">
                        <TextInput value={port.label.to_string()} onchange={on_label_change} />
                    </td>
                    <td class="pf-v6-c-table__td">
                        <ToggleGroup>
                            <ToggleGroupItem
                                text="Spleiss"
                                key=0
                                onchange={let cb = onselect.clone(); move |_| cb.emit(PortType::Splice)}
                                selected={selected == PortType::Splice}
                            />
                            <ToggleGroupItem
                                text="Stecker"
                                key=1
                                onchange={let cb = onselect.clone(); move |_| cb.emit(PortType::Connector)}
                                selected={selected == PortType::Connector}
                            />
                            <ToggleGroupItem
                                text="Loop"
                                key=2
                                onchange={let cb = onselect.clone(); move |_| cb.emit(PortType::Loop)}
                                selected={selected == PortType::Loop}
                            />
                        </ToggleGroup>
                    </td>
                    <td class="pf-v6-c-table__td">
                        <Button icon={Icon::AngleUp} variant={ButtonVariant::Plain} onclick={on_up} disabled={is_first} />
                        <Button icon={Icon::AngleDown} variant={ButtonVariant::Plain} onclick={on_down} disabled={is_last} />
                        <Button icon={Icon::Trash} variant={ButtonVariant::DangerSecondary} onclick={on_delete} />
                    </td>
                </tr>
            }
        });

        let error: Option<Html> = self.error.as_ref().map(<&FrontendError>::into_prop_value);

        // Titel-Text aus den optionalen Namen zusammenbauen
        let title_text = match (&self.panel_name, &self.cabinet_name) {
            (Some(p), Some(s)) => format!("Panel: {} (Schacht: {})", p, s),
            (Some(p), None) => format!("Panel: {}", p),
            (None, Some(s)) => format!("Panel bearbeiten (Schacht: {})", s),
            (None, None) => "Panel bearbeiten".to_string(),
        };

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        <h2 class="pf-v6-c-title pf-m-xl pf-v6-u-mb-md">{title_text}</h2>
                        {error}
                        <ActionGroup>
                            <Button label="Port hinzufügen" variant={ButtonVariant::Secondary} onclick={ctx.link().callback(|_| Msg::AddPort)} />
                            <Button label="Speichern" variant={ButtonVariant::Primary} onclick={ctx.link().callback(|_| Msg::Save)} />
                        </ActionGroup>
                        <table class="pf-v6-c-table pf-m-grid-md pf-m-compact" role="grid">
                            <thead>
                                <tr class="pf-v6-c-table__tr">
                                    //<th class="pf-v6-c-table__th">{"Nr."}</th>
                                    <th class="pf-v6-c-table__th">{"Bezeichnung"}</th>
                                    <th class="pf-v6-c-table__th">{"Typ"}</th>
                                    <th class="pf-v6-c-table__th">{"Aktionen"}</th>
                                </tr>
                            </thead>
                            <tbody class="pf-v6-c-table__tbody">
                                { for rows }
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchPorts);
        }
    }
}
