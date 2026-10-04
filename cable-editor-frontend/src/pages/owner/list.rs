//! The owners of Schächte and ducts (see docs/stammdaten.md): readable by everyone, changed by
//! admins, as the owner is the Datenherr of the delivery to the Leitungskataster.

use crate::components::load::Load;
use crate::{
    components::{
        dialog::ask_delete,
        menu::popup::{MenuActionItem, MenuGroup, PopupMenu},
        page_layout::PageLayout,
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::{
        current_user::Role,
        owners::{
            NAME_NOT_RELEASED, OwnerInput, OwnerListEntry, create_owner, delete_owner,
            fetch_owner_list, set_default_owner, update_owner,
        },
    },
    util::{get_backdrop, get_credentials, get_role, toast_error, toast_success},
};
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonType, ButtonVariant, Cell, CellContext,
    Checkbox, CheckboxState, Color, ExpansionState, Form, FormGroup, Icon, Label,
    MemoizedTableModel, MenuToggleVariant, Modal, ModalVariant, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableMode, TextInput,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use web_sys::SubmitEvent;
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Name,
    LkName,
    Schaechte,
    Ducts,
    Actions,
}

/// What a row's menu does with its owner.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    Edit,
    SetDefault,
    Delete,
}

/// A row: the owner and, for admins, its menu.
#[derive(Clone, PartialEq)]
struct OwnerRow {
    owner: OwnerListEntry,
    /// Missing for users who may not change owners
    onaction: Option<Callback<(Action, OwnerListEntry)>>,
}

impl TableEntryRenderer<Columns> for OwnerRow {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        let owner = &self.owner;
        match context.column {
            // One element per cell: on phones the cell lays its children out as a grid
            Columns::Name => Cell::new(html! {
                <span>
                    {owner.name.clone()}
                    if owner.is_default {
                        {" "}<Label label="Standard" compact=true color={Color::Blue}/>
                    }
                </span>
            }),
            Columns::LkName => Cell::new(match owner.lk_name.as_deref() {
                Some(NAME_NOT_RELEASED) => html!(<i>{"nicht freigegeben"}</i>),
                Some(name) => html!(name),
                None => html!(<span class="owner-list__default">{"wie Name"}</span>),
            }),
            Columns::Schaechte => Cell::new(owner.schacht_count.into_prop_value()),
            Columns::Ducts => Cell::new(if owner.delivered_duct_count > 0 {
                html!(format!(
                    "{} (geliefert {})",
                    owner.duct_count, owner.delivered_duct_count
                ))
            } else {
                html!(owner.duct_count)
            }),
            Columns::Actions => Cell::new(self.view_actions()),
        }
    }
}

impl OwnerRow {
    fn view_actions(&self) -> Html {
        let Some(onaction) = &self.onaction else {
            return Html::default();
        };
        let item = |action: Action, text: &'static str| {
            let owner = self.owner.clone();
            let onclick = onaction.reform(move |()| (action.clone(), owner.clone()));
            html_nested!(<MenuActionItem {onclick}>{text}</MenuActionItem>)
        };
        let owner = &self.owner;
        let deletable = !owner.is_default && owner.schacht_count == 0 && owner.duct_count == 0;
        html! {
            <PopupMenu
                variant={MenuToggleVariant::Plain}
                icon={Icon::EllipsisV}
                aria_label="Aktionen"
                align_end=true
            >
                <MenuGroup>
                    {item(Action::Edit, "Bearbeiten")}
                    if !owner.is_default {
                        {item(Action::SetDefault, "Als Standard setzen")}
                    }
                    if deletable {
                        {item(Action::Delete, "Löschen")}
                    }
                </MenuGroup>
            </PopupMenu>
        }
    }
}

/// All owners with their Schächte and ducts; admins create, change and delete them.
pub struct ListOfOwners {
    /// `None` while loading
    owners: Load<Rc<Vec<OwnerRow>>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Load,
    Loaded(Result<Box<[OwnerListEntry]>, FrontendError>),
    Action(Action, OwnerListEntry),
    New,
    /// Stores the dialog's values: a new owner without id
    Save(Option<i32>, OwnerInput),
    /// A change is stored: closes the dialog, reloads and confirms
    Saved(&'static str),
}

impl Component for ListOfOwners {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Load);
        Self {
            owners: Load::Pending,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let scope = ctx.link().clone();
        match msg {
            Msg::Load => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(Msg::Loaded(fetch_owner_list(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(owners) => {
                let onaction = (get_role(ctx.link()) >= Role::Admin).then(|| {
                    ctx.link()
                        .callback(|(action, owner)| Msg::Action(action, owner))
                });
                self.owners = Load::from(owners.map(|owners| {
                    Rc::new(
                        owners
                            .into_vec()
                            .into_iter()
                            .map(|owner| OwnerRow {
                                owner,
                                onaction: onaction.clone(),
                            })
                            .collect(),
                    )
                }));
                true
            }
            Msg::Action(Action::Edit, owner) => {
                self.open_dialog(ctx, Some(owner.id), "Eigentümer bearbeiten", owner.input());
                false
            }
            Msg::Action(Action::SetDefault, owner) => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    match set_default_owner(credentials.as_ref(), owner.id).await {
                        Ok(()) => scope.send_message(Msg::Saved("Standard-Eigentümer gesetzt")),
                        Err(error) => toast_error(
                            &scope,
                            "Standard-Eigentümer konnte nicht geändert werden",
                            error,
                        ),
                    }
                });
                false
            }
            Msg::Action(Action::Delete, owner) => {
                let name = owner.name.to_string();
                let on_confirm = Callback::from(move |()| {
                    let scope = scope.clone();
                    let credentials = get_credentials(&scope);
                    let owner_id = owner.id;
                    spawn_local(async move {
                        match delete_owner(credentials.as_ref(), owner_id).await {
                            Ok(()) => scope.send_message(Msg::Saved("Eigentümer gelöscht")),
                            Err(error) => toast_error(
                                &scope,
                                "Eigentümer konnte nicht gelöscht werden",
                                error,
                            ),
                        }
                    });
                });
                ask_delete(ctx.link(), "Eigentümer", &name, on_confirm);
                false
            }
            Msg::New => {
                self.open_dialog(ctx, None, "Neuer Eigentümer", OwnerInput::default());
                false
            }
            Msg::Save(owner_id, input) => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    let result = match owner_id {
                        Some(owner_id) => update_owner(credentials.as_ref(), owner_id, input).await,
                        None => create_owner(credentials.as_ref(), input).await,
                    };
                    match result {
                        Ok(()) => scope.send_message(Msg::Saved("Eigentümer gespeichert")),
                        // A toast: the dialog stays open with the entered values
                        Err(error) => {
                            toast_error(&scope, "Eigentümer konnte nicht gespeichert werden", error)
                        }
                    }
                });
                false
            }
            Msg::Saved(message) => {
                if let Some(backdrop) = get_backdrop(ctx.link()) {
                    backdrop.close();
                }
                toast_success(ctx.link(), message);
                ctx.link().send_message(Msg::Load);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = self.owners.view(|owners| self.view_owners(ctx, owners));
        html!(<PageLayout title="Eigentümer">{content}</PageLayout>)
    }
}

impl ListOfOwners {
    fn view_owners(&self, ctx: &Context<Self>, owners: &Rc<Vec<OwnerRow>>) -> Html {
        let is_admin = get_role(ctx.link()) >= Role::Admin;
        let header = html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Name" index={Columns::Name}/>
                <TableColumn<Columns> label="Name in der Lieferung" index={Columns::LkName}/>
                <TableColumn<Columns> label="Schächte" index={Columns::Schaechte}/>
                <TableColumn<Columns> label="Trassen" index={Columns::Ducts}/>
                <TableColumn<Columns> index={Columns::Actions}/>
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(owners.clone()),
            self.table_state.clone(),
        );
        html! {
            <>
                <Table<Columns, ListModel<Columns, MemoizedTableModel<OwnerRow>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
                if is_admin {
                    // Not stretched to the width of the page's content
                    <div>
                        <Button
                            label="Neuer Eigentümer"
                            variant={ButtonVariant::Primary}
                            onclick={ctx.link().callback(|_| Msg::New)}
                        />
                    </div>
                }
            </>
        }
    }

    fn open_dialog(
        &self,
        ctx: &Context<Self>,
        owner_id: Option<i32>,
        title: &'static str,
        owner: OwnerInput,
    ) {
        let Some(backdrop) = get_backdrop(ctx.link()) else {
            return;
        };
        let is_new = owner_id.is_none();
        let onsave = ctx.link().callback(move |input| Msg::Save(owner_id, input));
        let oncancel = {
            let backdrop = backdrop.clone();
            Callback::from(move |()| backdrop.close())
        };
        backdrop.open(Backdrop::new(html! {
            <OwnerDialog {title} {is_new} {owner} {onsave} {oncancel}/>
        }));
    }
}

#[derive(Properties, PartialEq)]
pub struct OwnerDialogProps {
    pub title: &'static str,
    /// Labels the button "Anlegen" instead of "Speichern"
    pub is_new: bool,
    /// The stored values, empty for a new owner
    pub owner: OwnerInput,
    pub onsave: Callback<OwnerInput>,
    pub oncancel: Callback<()>,
}

/// Name and name in the delivery of an owner; the page stores them and closes the dialog,
/// which stays open with the entered values if that fails.
pub struct OwnerDialog {
    name: String,
    /// Empty: the name
    lk_name: String,
    /// `Eigentuemer` is `Keine_Angabe` in the delivery
    not_released: bool,
}

pub enum DialogMsg {
    Name(String),
    LkName(String),
    NotReleased(bool),
    Save,
}

impl OwnerDialog {
    fn input(&self) -> OwnerInput {
        let optional = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        };
        OwnerInput {
            name: self.name.trim().to_string(),
            lk_name: if self.not_released {
                Some(NAME_NOT_RELEASED.to_string())
            } else {
                optional(&self.lk_name)
            },
        }
    }

    fn can_save(&self, ctx: &Context<Self>) -> bool {
        let input = self.input();
        !input.name.is_empty() && input != ctx.props().owner
    }
}

impl Component for OwnerDialog {
    type Message = DialogMsg;
    type Properties = OwnerDialogProps;

    fn create(ctx: &Context<Self>) -> Self {
        let owner = &ctx.props().owner;
        let not_released = owner.lk_name.as_deref() == Some(NAME_NOT_RELEASED);
        Self {
            name: owner.name.clone(),
            lk_name: if not_released {
                String::new()
            } else {
                owner.lk_name.clone().unwrap_or_default()
            },
            not_released,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            DialogMsg::Name(name) => self.name = name,
            DialogMsg::LkName(lk_name) => self.lk_name = lk_name,
            DialogMsg::NotReleased(not_released) => self.not_released = not_released,
            DialogMsg::Save => {
                if self.can_save(ctx) {
                    ctx.props().onsave.emit(self.input());
                }
                return false;
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let onsubmit = link.callback(|e: SubmitEvent| {
            e.prevent_default();
            DialogMsg::Save
        });
        let oncancel = ctx.props().oncancel.reform(|_| ());
        let checked = if self.not_released {
            CheckboxState::Checked
        } else {
            CheckboxState::Unchecked
        };
        html! {
            <Bullseye>
                <Modal
                    title={ctx.props().title}
                    variant={ModalVariant::Medium}
                    onclose={ctx.props().oncancel.reform(|_| ())}
                >
                    <Form {onsubmit}>
                        <FormGroup label="Name" required=true>
                            <TextInput
                                value={self.name.clone()}
                                onchange={link.callback(DialogMsg::Name)}
                                required=true
                                autofocus=true
                            />
                        </FormGroup>
                        <FormGroup label="Name in der Lieferung an den Leitungskataster">
                            <TextInput
                                value={if self.not_released { NAME_NOT_RELEASED.to_string() } else { self.lk_name.clone() }}
                                onchange={link.callback(DialogMsg::LkName)}
                                placeholder="wie der Name"
                                disabled={self.not_released}
                            />
                            <Checkbox
                                label="Name nicht freigeben"
                                {checked}
                                onchange={link.callback(|state: CheckboxState| DialogMsg::NotReleased(state == CheckboxState::Checked))}
                            />
                        </FormGroup>
                        <ActionGroup>
                            <Button label={if ctx.props().is_new { "Anlegen" } else { "Speichern" }} variant={ButtonVariant::Primary} r#type={ButtonType::Submit} disabled={!self.can_save(ctx)}/>
                            <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={oncancel}/>
                        </ActionGroup>
                    </Form>
                </Modal>
            </Bullseye>
        }
    }
}
