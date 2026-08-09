use crate::components::table::{
    TreeModel, TreeState, TreeTable, TreeTableColumn, TreeTableContext,
};
use crate::create_simple_dialog;
use crate::error::FrontendError;
use crate::graphql::authenticated::IdOrNew;
use crate::graphql::authenticated::cabinet_details::{
    FlatPanelInput, PanelTreeEntry, update_panels_in_cabinet,
};
use crate::util::get_credentials;
use patternfly_yew::prelude::*;
use std::collections::{HashMap, HashSet};
use web_sys::HtmlElement;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::prelude::*;
use yew::{Callback, Component, Context, Html, Properties, html};

pub struct EditCabinet {
    loading: bool,
    error: Option<FrontendError>,
    state: TreeState<IdOrNew>,
    model: TreeModel<IdOrNew, PanelEntry>,
    loaded_panels: Option<Box<[PanelTreeEntry]>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PanelEntry {
    pub id: IdOrNew,
    pub name: Option<Box<str>>,
}

pub enum Msg {
    FetchPanels,
    PanelsFetched(Box<[PanelTreeEntry]>),
    CreatePanel,
    PanelCreated(()),
    UpdatePanel(i32, String, i32),
    PanelUpdated(Result<(), FrontendError>),
    DeletePanel(i32),
    PanelDeleted(Result<(), FrontendError>),
    Error(FrontendError),
    PanelEvent(PanelEditAction),
    Save,
}
#[derive(PartialEq, Properties)]
pub struct EditCabinetProps {
    #[prop_or_default]
    pub plan_id: i32,
    pub cabinet_id: i32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum PanelColumn {
    Name,
    Id,
    Actions,
}
#[derive(Clone, PartialEq, Hash, Eq)]
enum PanelEditAction {
    Remove(IdOrNew),
    ExchangeSiblings {
        parent: Option<IdOrNew>,
        siblings: [IdOrNew; 2],
    },
    NewParent {
        entry: IdOrNew,
        new_parent: IdOrNew,
    },
    MoveUp(IdOrNew),
    SetName {
        id: IdOrNew,
        text: Box<str>,
    },
}

impl TreeTableColumn<IdOrNew, PanelEntry, PanelEditAction> for PanelColumn {
    fn render_cell(&self, context: TreeTableContext<IdOrNew, PanelEntry, PanelEditAction>) -> Cell {
        match self {
            PanelColumn::Name => {
                let text = context
                    .row
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or("<no name>");
                let onblur = {
                    let callback = context.callback.clone();
                    let id = *context.key;
                    Callback::from(move |e: FocusEvent| {
                        if let Some(target) = e.target_dyn_into::<HtmlElement>() {
                            callback.emit(PanelEditAction::SetName {
                                id,
                                text: target.inner_text().into_boxed_str(),
                            });
                        }
                    })
                };
                Cell::new(html!(<span {onblur} contenteditable="true" key={text}>{text}</span>))
            }
            PanelColumn::Actions => {
                let mut buttons = Vec::new();
                if context.parent.is_some() {
                    let key = *context.key;
                    let callback = context.callback.clone();
                    let onclick =
                        Callback::from(move |_| callback.emit(PanelEditAction::MoveUp(key)));

                    buttons.push(html!(<Button icon={Icon::AngleDoubleLeft} {onclick} variant={ButtonVariant::Secondary} />))
                }
                if let (Some(other_sibling)) = (context.previous_sibling) {
                    let parent = context.parent.copied();
                    let siblings = [*context.key, *other_sibling];
                    let callback = context.callback.clone();
                    let onclick = Callback::from(move |_| {
                        callback.emit(PanelEditAction::ExchangeSiblings { parent, siblings })
                    });

                    buttons.push(html!(<Button icon={Icon::LongArrowAltUp} {onclick} variant={ButtonVariant::Secondary}/>))
                }
                if let Some(previous_sibling) = context.previous_sibling {
                    let entry = *context.key;
                    let new_parent = *previous_sibling;
                    let callback = context.callback.clone();
                    let onclick = Callback::from(move |_| {
                        callback.emit(PanelEditAction::NewParent { entry, new_parent })
                    });
                    buttons.push(html!(<Button icon={Icon::AngleDoubleRight} {onclick} variant={ButtonVariant::Secondary}/>))
                }
                if context.children.is_empty() {
                    let key = *context.key;
                    let callback = context.callback.clone();
                    let onclick =
                        Callback::from(move |_| callback.emit(PanelEditAction::Remove(key)));

                    buttons.push(html!(<Button icon={Icon::Trash} {onclick} variant={ButtonVariant::DangerSecondary} />))
                }

                Cell::new(buttons.into_iter().collect())
            }
            PanelColumn::Id => Cell::new(format!("{:?}", context.key).into_prop_value()),
        }
    }
}
impl Component for EditCabinet {
    type Message = Msg;
    type Properties = EditCabinetProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            loading: true,
            error: None,
            state: TreeState::default(),
            model: TreeModel::default(),
            loaded_panels: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchPanels => {
                if let Some(credentials) = get_credentials(ctx.link()) {
                    self.loading = true;
                    self.error = None;
                    let cabinet_id = ctx.props().cabinet_id;
                    let scope = ctx.link().clone();
                    spawn_local(async move {
                        scope.send_message(
                            PanelTreeEntry::fetch(Some(&credentials), cabinet_id)
                                .await
                                .map_or_else(Msg::Error, Msg::PanelsFetched),
                        );
                    });
                }
                true
            }
            Msg::PanelsFetched(panel_entries) => {
                self.loading = false;
                self.error = None;
                let mut entries = HashMap::new();
                let mut child_rels = HashMap::new();
                let mut roots = Vec::with_capacity(panel_entries.len());
                for PanelTreeEntry { id, name, children } in panel_entries.iter().cloned() {
                    roots.push(id.into());
                    entries.insert(
                        id.into(),
                        PanelEntry {
                            id: id.into(),
                            name,
                        },
                    );
                    append_children(&mut entries, &mut child_rels, id.into(), children);
                }
                self.loaded_panels = Some(panel_entries);
                self.model = TreeModel::new(roots.into_boxed_slice(), entries, child_rels);
                true
            }
            Msg::CreatePanel => {
                let new_id = IdOrNew::default();
                let roots = self
                    .model
                    .roots()
                    .iter()
                    .copied()
                    .chain(Some(new_id))
                    .collect();
                let entries = self
                    .model
                    .entries()
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .chain(Some((
                        new_id,
                        PanelEntry {
                            id: new_id,
                            name: None,
                        },
                    )))
                    .collect();
                self.model = TreeModel::new(
                    roots,
                    entries,
                    self.model
                        .children()
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .collect(),
                );
                true
            }
            Msg::PanelCreated(result) => {
                ctx.link().send_message(Msg::FetchPanels);
                true
            }
            Msg::UpdatePanel(id, name, position) => {
                let link = ctx.link().clone();
                spawn_local(async move {
                    // TODO: Replace with actual GraphQL mutation
                    let result = Self::update_panel(id, name, position).await;
                    link.send_message(Msg::PanelUpdated(result));
                });
                false
            }
            Msg::PanelUpdated(result) => {
                match result {
                    Ok(_) => {
                        ctx.link().send_message(Msg::FetchPanels);
                    }
                    Err(e) => {
                        //self.error = Some(format!("Failed to update panel: {:?}", e));
                    }
                }
                true
            }
            Msg::DeletePanel(id) => {
                let link = ctx.link().clone();
                spawn_local(async move {
                    // TODO: Replace with actual GraphQL mutation
                    let result = Self::delete_panel(id).await;
                    link.send_message(Msg::PanelDeleted(result));
                });
                false
            }
            Msg::PanelDeleted(result) => {
                match result {
                    Ok(_) => {
                        ctx.link().send_message(Msg::FetchPanels);
                    }
                    Err(e) => {
                        //self.error = Some(format!("Failed to delete panel: {:?}", e));
                    }
                }
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                self.loading = false;
                true
            }
            Msg::PanelEvent(PanelEditAction::Remove(id)) => {
                self.model = self.model.remove(&id);
                true
            }
            Msg::PanelEvent(PanelEditAction::ExchangeSiblings { parent, siblings }) => {
                self.model = self.model.exchange_siblings(parent, siblings);
                true
            }
            Msg::PanelEvent(PanelEditAction::NewParent { entry, new_parent }) => {
                self.model = self.model.new_parent(entry, Some(new_parent));
                self.state.open(new_parent);
                true
            }
            Msg::PanelEvent(PanelEditAction::MoveUp(entry)) => {
                if let Some((current_parent, _)) = self
                    .model
                    .children()
                    .iter()
                    .find(|(_, children)| children.contains(&entry))
                {
                    let new_parent = self
                        .model
                        .children()
                        .iter()
                        .find(|(_, children)| children.contains(current_parent))
                        .map(|(key, _)| *key);
                    if let Some(parent) = new_parent {
                        self.state.open(parent);
                    }

                    self.model = self.model.new_parent(entry, new_parent);
                    true
                } else {
                    false
                }
            }
            Msg::PanelEvent(PanelEditAction::SetName { id, text }) => {
                let mut entries = self.model.entries().clone();
                if let Some(entry) = entries.get_mut(&id) {
                    entry.name = Some(text);
                }
                let roots = Box::from(self.model.roots());
                let children = self.model.children().clone();
                self.model = TreeModel::new(roots, entries, children);
                true
            }
            Msg::Save => {
                self.loading = true;

                // 1. Original-Zustand flachklopfen
                let mut original_nodes = HashMap::new();
                if let Some(loaded) = &self.loaded_panels {
                    flatten_loaded(loaded, None, &mut original_nodes);
                }

                // 2. Aktuellen Zustand flachklopfen
                let mut current_nodes = Vec::new();
                flatten_current(&self.model, self.model.roots(), None, &mut current_nodes);

                let mut to_create = Vec::new();
                let mut to_update = Vec::new();
                let mut to_delete = Vec::new();
                let mut current_ids = HashSet::new();

                // 3. Neue und geänderte Panels ermitteln
                for node in current_nodes {
                    current_ids.insert(node.id);

                    match node.id {
                        IdOrNew::Temporary(_) => {
                            to_create.push(node);
                        }
                        IdOrNew::Id(_) => {
                            if let Some(orig) = original_nodes.get(&node.id)
                                && &node != orig
                            {
                                to_update.push(node);
                            }
                        }
                    }
                }

                // 4. Gelöschte Panels ermitteln
                for orig_id in original_nodes.keys() {
                    if !current_ids.contains(orig_id)
                        && let IdOrNew::Id(deleted_id) = orig_id
                    {
                        to_delete.push(*deleted_id);
                    }
                }

                let mut changes = Vec::with_capacity(to_create.len() + to_update.len());

                for node in to_create.into_iter().chain(to_update) {
                    changes.push(FlatPanelInput {
                        id: node.id.into(),
                        name: node.name.map(str::into_string),
                        parent_id: node.parent_id.map(Into::into),
                        order: node.order,
                    });
                }

                let cabinet_id = ctx.props().cabinet_id;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        update_panels_in_cabinet(to_delete, changes, cabinet_id, credentials)
                            .await
                            .map_or_else(Msg::Error, |_| Msg::FetchPanels),
                    );
                });
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            html!(<Spinner />)
        } else {
            let header = html_nested! {
                <TableHeader<PanelColumn>>
                    //<TableColumn<PanelColumn> label="ID" index={PanelColumn::Id} />
                    <TableColumn<PanelColumn> label="Name" index={PanelColumn::Name} />
                    <TableColumn<PanelColumn> index={PanelColumn::Actions} />
                </TableHeader<PanelColumn>>
            };
            let model = self.model.clone();
            let error = self
                .error
                .as_ref()
                .map(IntoPropValue::<Html>::into_prop_value);
            let create_panel_callback = ctx.link().callback(|_| Msg::CreatePanel);
            let save_callback = ctx.link().callback(|_| Msg::Save);
            let row_event = ctx.link().callback(Msg::PanelEvent);

            html! {
                <>
                <Title>{"Panels im Schacht"}</Title>
                {error}
                <TreeTable<IdOrNew, PanelEntry,PanelEditAction, PanelColumn>
                    state={self.state.clone()}
                    mode={TableMode::Default}
                    {row_event}
                    {header}
                    {model}
                    />
                    <ActionGroup>
                     <Button
                         label="Panel hinzufügen"
                         variant={ButtonVariant::Secondary}
                         onclick={create_panel_callback}
                     />
                     <Button
                         label="Speichern"
                         variant={ButtonVariant::Primary}
                         onclick={save_callback}
                     />
                 </ActionGroup>
                </>
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchPanels);
        }
    }
}

fn append_children(
    entries: &mut HashMap<IdOrNew, PanelEntry>,
    child_rels: &mut HashMap<IdOrNew, Box<[IdOrNew]>>,
    parent: IdOrNew,
    children: Box<[PanelTreeEntry]>,
) {
    let mut child_ids = Vec::with_capacity(children.len());
    for PanelTreeEntry { id, name, children } in children {
        child_ids.push(id.into());
        entries.insert(
            id.into(),
            PanelEntry {
                id: id.into(),
                name,
            },
        );
        append_children(entries, child_rels, id.into(), children);
    }
    child_rels.insert(parent, child_ids.into_boxed_slice());
}

impl EditCabinet {
    // TODO: Implement actual GraphQL queries/mutations

    async fn update_panel(id: i32, name: String, position: i32) -> Result<(), FrontendError> {
        // Placeholder implementation
        Ok(())
    }

    async fn delete_panel(id: i32) -> Result<(), FrontendError> {
        // Placeholder implementation
        Ok(())
    }
}

create_simple_dialog!(NewPanel, NewPanelProps, NewPanelData, (name, "Name"),);

#[derive(Debug, Clone, PartialEq)]
struct FlatPanelNode {
    id: IdOrNew,
    name: Option<Box<str>>,
    parent_id: Option<IdOrNew>,
    order: i32,
}

// Rekursives Flachklopfen der vom Server geladenen Daten
fn flatten_loaded(
    panels: &[PanelTreeEntry],
    parent_id: Option<IdOrNew>,
    result: &mut HashMap<IdOrNew, FlatPanelNode>,
) {
    for (i, panel) in panels.iter().enumerate() {
        let current_id = IdOrNew::Id(panel.id);
        result.insert(
            current_id,
            FlatPanelNode {
                id: current_id,
                name: panel.name.clone(),
                parent_id,
                order: (i + 1) as i32,
            },
        );
        flatten_loaded(&panel.children, Some(current_id), result);
    }
}

fn flatten_current(
    model: &TreeModel<IdOrNew, PanelEntry>,
    entries: &[IdOrNew],
    parent_id: Option<IdOrNew>,
    result: &mut Vec<FlatPanelNode>,
) {
    for (i, id) in entries.iter().enumerate() {
        if let Some(entry) = model.entries().get(id) {
            result.push(FlatPanelNode {
                id: *id,
                name: entry.name.clone(),
                parent_id,
                order: (i + 1) as i32,
            });

            if let Some(children) = model.children().get(id) {
                flatten_current(model, children, Some(*id), result);
            }
        }
    }
}
