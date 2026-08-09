use log::info;
use patternfly_yew::ouia;
use patternfly_yew::prelude::{
    Button, ButtonVariant, Caption, Cell, ComposableTable, Dropdown, ExpandParams, ExpandType,
    ExpansionState, Icon, MenuChildVariant, MenuToggleVariant, Ouia, OuiaComponentType, OuiaSafe,
    StateModel, StateModelIter, TableBody, TableData, TableDataModel, TableGridMode, TableHeader,
    TableMode, TableModel,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;
use web_sys::MouseEvent;
use yew::virtual_dom::VChild;
use yew::{
    AttrValue, Callback, Classes, Component, Context, Html, Properties, classes,
    function_component, html, props,
};

pub struct ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    model: Rc<StateModel<C, M>>,
}

impl<C, M> ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    pub fn new(data: M, state: Rc<RefCell<HashMap<M::Key, ExpansionState<C>>>>) -> ListModel<C, M> {
        ListModel {
            model: Rc::new(StateModel::new(data, state)),
        }
    }
}

impl<C, M> TableModel<C> for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    type Iterator<'i> = StateModelIter<'i, M::Key, M::Item, C>;
    type Item = M::Item;
    type Key = M::Key;

    fn len(&self) -> usize {
        self.model.len()
    }

    fn is_empty(&self) -> bool {
        self.model.is_empty()
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.model.iter()
    }
}

impl<C, M> Clone for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
        }
    }
}

impl<C, M> PartialEq for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
    }
}

#[derive(PartialEq, Default)]
struct InnerTreeModel<Key, Row>
where
    Row: PartialEq + Clone,
    Key: Hash + Eq,
{
    roots: Box<[Key]>,
    entries: HashMap<Key, Row>,
    children: HashMap<Key, Box<[Key]>>,
}

#[derive(PartialEq, Clone)]
pub struct TreeModel<Key, Row>(Rc<InnerTreeModel<Key, Row>>)
where
    Row: PartialEq + Clone,
    Key: Hash + Eq;

impl<Key, Row> Default for TreeModel<Key, Row>
where
    Row: PartialEq + Clone,
    Key: Hash + Eq + std::clone::Clone,
{
    fn default() -> Self {
        TreeModel::new(Box::default(), HashMap::default(), HashMap::default())
    }
}

impl<Key, Row> TreeModel<Key, Row>
where
    Row: PartialEq + Clone,
    Key: Hash + Eq + Clone,
{
    pub fn new(
        roots: Box<[Key]>,
        entries: HashMap<Key, Row>,
        children: HashMap<Key, Box<[Key]>>,
    ) -> Self {
        TreeModel(Rc::new(InnerTreeModel {
            roots,
            entries,
            children,
        }))
    }
    pub fn roots(&self) -> &[Key] {
        self.0.roots.as_ref()
    }
    pub fn entries(&self) -> &HashMap<Key, Row> {
        &self.0.entries
    }
    pub fn children(&self) -> &HashMap<Key, Box<[Key]>> {
        &self.0.children
    }
    fn rows(&self, state: &TreeState<Key>) -> Box<[TreeRow<'_, Key>]> {
        let mut result = Vec::with_capacity(self.0.entries.len());
        let mut previous_sibling = None;
        for root_key in &self.0.roots {
            append_tree(
                &mut result,
                1,
                state,
                None,
                root_key,
                &self.0.children,
                previous_sibling,
            );
            previous_sibling = Some(root_key);
        }
        result.into_boxed_slice()
    }
    pub fn remove(&self, id: &Key) -> Self {
        let roots = self
            .roots()
            .iter()
            .filter(|rid| *rid != id)
            .cloned()
            .collect();
        let entries = self
            .entries()
            .iter()
            .filter(|(key, _)| *key != id)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let children = self
            .children()
            .iter()
            .map(|(key, children)| {
                (
                    key.clone(),
                    children
                        .into_iter()
                        .filter(|ch| *ch != id)
                        .cloned()
                        .collect(),
                )
            })
            .collect();
        Self::new(roots, entries, children)
    }
    pub fn exchange_siblings(&self, parent: Option<Key>, siblings: [Key; 2]) -> Self {
        let (children, roots) = if let Some(parent) = parent {
            let mut children = self.children().clone();
            if let Some(all_siblings) = children.remove(&parent) {
                children.insert(
                    parent,
                    Self::exchange_siblings_in_list(siblings, &all_siblings),
                );
            }
            (children, self.roots().iter().cloned().collect())
        } else {
            (
                self.children().clone(),
                Self::exchange_siblings_in_list(siblings, self.roots()),
            )
        };
        Self::new(
            roots,
            self.entries()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            children,
        )
    }

    fn exchange_siblings_in_list(switching_siblings: [Key; 2], all_siblings: &[Key]) -> Box<[Key]> {
        all_siblings
            .iter()
            .cloned()
            .map(|k| {
                if k == switching_siblings[0] {
                    switching_siblings[1].clone()
                } else if k == switching_siblings[1] {
                    switching_siblings[0].clone()
                } else {
                    k
                }
            })
            .collect()
    }

    pub fn new_parent(&self, entry: Key, new_parent: Option<Key>) -> Self {
        let mut roots = self
            .roots()
            .iter()
            .filter(|id| *id != &entry)
            .cloned()
            .collect::<Vec<_>>();
        let mut children: HashMap<Key, Box<[Key]>> = self
            .children()
            .iter()
            .map(|(key, children)| {
                (
                    key.clone(),
                    children
                        .into_iter()
                        .filter(|ch| **ch != entry)
                        .cloned()
                        .collect(),
                )
            })
            .collect();
        if let Some(new_parent) = new_parent {
            let mut new_list = children
                .remove(&new_parent)
                .map(|e| e.into_vec())
                .unwrap_or_default();
            new_list.push(entry);
            children.insert(new_parent, new_list.into_boxed_slice());
        } else {
            roots.push(entry);
        }

        Self::new(
            roots.into_boxed_slice(),
            self.entries()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            children,
        )
    }
}

fn append_tree<'a, Key>(
    result: &mut Vec<TreeRow<'a, Key>>,
    level: u8,
    states: &TreeState<Key>,
    parent_key: Option<&'a Key>,
    entry: &'a Key,
    children: &'a HashMap<Key, Box<[Key]>>,
    previous_sibling: Option<&'a Key>,
) where
    Key: Eq + Hash,
{
    if let Some(children_of_entry) = children.get(entry)
        && !children_of_entry.is_empty()
    {
        let expanded = states.is_open(entry);
        result.push(TreeRow {
            level,
            expanded,
            expandable: true,
            key: entry,
            parent_key,
            previous_sibling,
        });
        if expanded {
            let mut previous_sibling = None;
            for child_entry in children_of_entry {
                append_tree(
                    result,
                    level + 1,
                    states,
                    Some(entry),
                    child_entry,
                    children,
                    previous_sibling,
                );
                previous_sibling = Some(child_entry);
            }
        }
    } else {
        result.push(TreeRow {
            level,
            expanded: false,
            expandable: false,
            key: entry,
            parent_key,
            previous_sibling,
        });
    };
}

#[derive(Copy, Clone)]
struct TreeRow<'a, Key> {
    level: u8,
    expanded: bool,
    expandable: bool,
    key: &'a Key,
    parent_key: Option<&'a Key>,
    previous_sibling: Option<&'a Key>,
}

#[derive(PartialEq, Default)]
struct InnerTreeStateModel<K: Hash + Eq> {
    open_parents: HashSet<K>,
}
#[derive(PartialEq, Clone)]
pub struct TreeState<K: Hash + Eq>(Rc<RefCell<InnerTreeStateModel<K>>>);

impl<K: Hash + Eq> TreeState<K> {
    pub fn open(&self, k: K) {
        self.0.borrow_mut().open_parents.insert(k);
    }
    pub fn close(&self, k: K) {
        self.0.borrow_mut().open_parents.remove(&k);
    }
    pub fn toggle(&self, k: K) {
        let mut ref_mut = self.0.borrow_mut();
        let open_parents = &mut ref_mut.open_parents;
        if open_parents.contains(&k) {
            open_parents.remove(&k);
        } else {
            open_parents.insert(k);
        }
    }
    pub fn is_open(&self, k: &K) -> bool {
        self.0.borrow().open_parents.contains(k)
    }
}

impl<K: Hash + Eq> Default for TreeState<K> {
    fn default() -> Self {
        TreeState(Rc::new(RefCell::new(InnerTreeStateModel {
            open_parents: HashSet::default(),
        })))
    }
}

pub struct TreeTable<Key, Row, Msg, C>
where
    Row: PartialEq + Clone,
    Key: Hash + Eq,
    Msg: PartialEq,
{
    phantom: PhantomData<(Key, Row, Msg, C)>,
}

pub enum TreeTableMsg<RowMsg: PartialEq, Key> {
    RowEvent(RowMsg),
    ToogleRow(Key),
}
const OUIA: Ouia = ouia!("Table");
#[derive(PartialEq, Properties, Clone)]
pub struct TreeTableProps<Key, Row, Msg, C>
where
    Row: PartialEq + Clone,
    Key: Hash + Eq,
    Msg: PartialEq,
    C: Clone + Eq + 'static,
{
    pub model: TreeModel<Key, Row>,
    #[prop_or_default]
    pub state: TreeState<Key>,
    #[prop_or_default]
    pub row_event: Callback<Msg>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub id: AttrValue,
    #[prop_or_default]
    pub caption: Option<String>,
    #[prop_or_default]
    pub mode: TableMode,
    #[prop_or(true)]
    pub borders: bool,
    #[prop_or_default]
    pub header: Option<VChild<TableHeader<C>>>,

    #[prop_or_default]
    pub grid: Option<TableGridMode>,

    /// OUIA Component id
    #[prop_or_default]
    pub ouia_id: Option<String>,
    /// OUIA Component Type
    #[prop_or(OUIA.component_type())]
    pub ouia_type: OuiaComponentType,
    /// OUIA Component Safe
    #[prop_or(OuiaSafe::TRUE)]
    pub ouia_safe: OuiaSafe,
}

impl<Key, Row, Msg, C> Component for TreeTable<Key, Row, Msg, C>
where
    Row: Clone + 'static + Eq + Hash,
    Key: Hash + Eq + 'static + Clone,
    Msg: PartialEq + 'static + Clone,
    C: Clone + Eq + 'static + TreeTableColumn<Key, Row, Msg>,
{
    type Message = TreeTableMsg<Msg, Key>;
    type Properties = TreeTableProps<Key, Row, Msg, C>;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            phantom: Default::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            TreeTableMsg::RowEvent(evt) => {
                ctx.props().row_event.emit(evt);
                false
            }
            TreeTableMsg::ToogleRow(row) => {
                ctx.props().state.toggle(row);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let class = classes!(
            "pf-v6-c-table",
            "pf-m-tree-view-grid-lg",
            "pf-m-tree-view",
            &props.class
        );
        let toggle_row_callback = ctx.link().callback(TreeTableMsg::ToogleRow);
        html!(
            <ComposableTable
                id={&props.id}
                {class}
                sticky_header={props.header.as_ref().is_some_and(|header| header.props.sticky)}
                mode={props.mode}
                borders={props.borders}
                grid={props.grid}
                ouia_id={props.ouia_id.clone()}
                ouia_type={props.ouia_type}
                ouia_safe={props.ouia_safe}
            >
                if let Some(caption) = &props.caption {
                    <Caption>{ caption }</Caption>
                }
                if let Some(header) = props.header.clone() {
                    <TableHeader<C> ..(*header.props).clone() />
                }
                { render_entries(props, toggle_row_callback) }
            </ComposableTable>
        )
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct TreeTableContext<'a, Key, Row, Msg> {
    pub key: &'a Key,
    pub row: &'a Row,
    pub children: &'a [Key],
    pub parent: Option<&'a Key>,
    pub callback: &'a Callback<Msg>,
    pub previous_sibling: Option<&'a Key>,
}

pub trait TreeTableColumn<Key, Row, Msg> {
    fn render_cell(&self, context: TreeTableContext<Key, Row, Msg>) -> Cell;
    fn actions(context: TreeTableContext<Key, Row, Msg>) -> Vec<MenuChildVariant> {
        Vec::new()
    }
}

fn render_entries<Key, Row, Msg, C>(
    props: &TreeTableProps<Key, Row, Msg, C>,
    toggle_row_callback: Callback<Key>,
) -> Html
where
    C: 'static + Clone + Eq + TreeTableColumn<Key, Row, Msg>,
    Key: 'static + Eq + Hash + Clone,
    Msg: 'static + PartialEq + Clone,
    Row: 'static + Clone + Eq + std::hash::Hash,
{
    let rows = props.model.rows(&props.state);
    if let Some(header) = &props.header {
        for child in header.props.children.iter() {
            let idx = &child.props.index;
        }
    }

    let rows = rows.into_iter().filter_map(|entry| {
        {
            props
                .model
                .entries()
                .get(entry.key)
                .map(|data| (entry, data))
        }
        .map(|(tree_row, data_row)| {
            let TreeRow {
                level,
                expanded,
                expandable,
                ..
            } = tree_row;
            let toggle_expand = {
                let key = tree_row.key.clone();
                let toggle_row_callback = toggle_row_callback.clone();
                Callback::from(move |_| {
                    toggle_row_callback.emit(key.clone());
                })
            };
            let (header, tail) = render_row(props, tree_row, data_row);
            html! {
            <TreeTableRow {level} {expanded} {expandable} {header} {toggle_expand}>
                {tail}
            </TreeTableRow>
            }
        })
    });
    html!(<TableBody>{for  rows}</TableBody>)
}

fn render_row<Key, Row, Msg, C>(
    props: &TreeTableProps<Key, Row, Msg, C>,
    tree_row: TreeRow<Key>,
    data_row: &Row,
) -> (Option<Html>, Html)
where
    C: 'static + Clone + Eq + TreeTableColumn<Key, Row, Msg>,
    Key: 'static + Eq + Hash + Clone,
    Msg: 'static + PartialEq + Clone,
    Row: 'static + Clone + Eq + std::hash::Hash,
{
    let children = props
        .model
        .children()
        .get(tree_row.key)
        .map(|e| e.as_ref())
        .unwrap_or_default();
    let row_context = TreeTableContext {
        key: tree_row.key,
        row: data_row,
        children,
        parent: tree_row.parent_key,
        callback: &props.row_event,
        previous_sibling: tree_row.previous_sibling,
    };
    let actions = C::actions(row_context.clone());
    let mut cols = props
        .header
        .iter()
        .flat_map(|header| header.props.children.iter());
    let header_column = cols.next().map(|column| {
        let cell = column.props.index.render_cell(row_context.clone());
        cell.content
    });
    let cells = cols.map(|column| {
        let cell = column.props.index.render_cell(row_context.clone());
        html!(
            <TableData
                data_label={column.props.label.clone().map(AttrValue::from)}
                center={cell.center}
                text_modifier={cell.text_modifier}
            >
                { cell.content }
            </TableData>
        )
    });

    (
        header_column,
        html! {
            <>
                {for cells}
                <RowActions {actions} />
            </>
        },
    )
}

#[derive(PartialEq, Properties)]
struct RowActionsProperties {
    actions: Vec<MenuChildVariant>,
}

#[function_component(RowActions)]
fn row_actions(props: &RowActionsProperties) -> Html {
    html!(
        <>
            if !props.actions.is_empty() {
                <TableData action=true>
                    <Dropdown variant={MenuToggleVariant::Plain} icon={Icon::EllipsisV}>
                        { props.actions.clone() }
                    </Dropdown>
                </TableData>
            }
        </>
    )
}
#[derive(Debug, Clone, PartialEq, Properties)]
pub struct TreeTableRowProperties {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Html,
    #[prop_or_default]
    pub header: Html,
    #[prop_or_default]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or_default]
    pub selected: bool,
    #[prop_or_default]
    pub expandable: bool,
    #[prop_or_default]
    pub toggle_expand: Callback<()>,
    #[prop_or_default]
    pub expanded: bool,
    #[prop_or_default]
    pub control_row: bool,
    #[prop_or_default]
    pub level: u8,
}

#[function_component(TreeTableRow)]
pub fn table_row(props: &TreeTableRowProperties) -> Html {
    let mut class = classes!(
        "pf-v6-c-table__tr",
        "pf-m-tree-view-details-expanded",
        props.class.clone()
    );
    if props.onclick.is_some() {
        class.push("pf-m-clickable");
    }
    if props.selected {
        class.push("pf-m-selected");
    }
    let header_column_content = if props.expandable {
        //class.push("pf-v6-c-table__expandable-row");
        class.push("pf-v6-c-table__toggle");
        let mut button_class = classes!("pf-v6-c-table__td", "pf-v6-c-table__toggle");
        if props.expanded {
            button_class.push("pf-m-expanded");
        }
        let onclick = {
            let ontoggle = props.toggle_expand.clone();
            Callback::from(move |_| ontoggle.emit(()))
        };
        html! {
            <Button
                variant={ButtonVariant::Plain}
                class={button_class}
                {onclick}
                aria_expanded={props.expanded.to_string()}
            >
                <div class="pf-v6-c-table__toggle-icon">{ Icon::AngleDown }</div>
            </Button>
        }
    } else {
        html! {}
    };
    if props.control_row {
        class.push("pf-v6-c-table__control-row");
    }
    html! {
        <tr class={class.clone()} role="row" onclick={props.onclick.clone()} aria-level={props.level.to_string()}>
            <th class="pf-v6-c-table__th pf-v6-c-table__tree-view-title-cell">
                <div class="pf-v6-c-table__tree-view-main">
                    {header_column_content}
                    {props.header.clone()}
                </div>
            </th>
            { props.children.clone() }
        </tr>
    }
}
