use crate::components::table::ListModel;
use crate::error::FrontendError;
use crate::graphql::authenticated::select_duct::{DuctListEntry, list_all_ducts};
use log::info;
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableMode, UseTableData,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html_nested};
use yew_oauth2::prelude::OAuth2Context;

#[derive(Debug, Default)]
pub struct SelectDuct {
    found_ducts: Option<Rc<Vec<DuctListEntry>>>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
    error: Option<FrontendError>,
}
pub enum Msg {
    Data(Box<[DuctListEntry]>),
    Error(FrontendError),
}

#[derive(PartialEq, Properties)]
pub struct SelectDuctProps {
    #[prop_or_default]
    pub on_select: Callback<DuctListEntry>,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    SchachtA,
    SchachtZ,
    Length,
}
impl TableEntryRenderer<Columns> for DuctListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::SchachtA => Cell::new(self.schacht_a.name.as_str().into_prop_value()),
            Columns::SchachtZ => Cell::new(self.schacht_z.name.as_str().into_prop_value()),
            Columns::Length => self
                .length
                .map(|l| Cell::new(format!("{l:.1} m").into_prop_value()))
                .unwrap_or_default(),
        }
    }
}

impl Component for SelectDuct {
    type Message = Msg;
    type Properties = SelectDuctProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.found_ducts = Some(Rc::new(data.into_vec()));
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(table) = &self.found_ducts {
            let entries = ListModel::new(
                MemoizedTableModel::new(table.clone()),
                self.table_state.clone(),
            );
            let header = html_nested! {
                <TableHeader<Columns>>
                    <TableColumn<Columns> label="Schacht" index={Columns::SchachtA}/>
                    <TableColumn<Columns> label="Länge" index={Columns::Length}/>
                    <TableColumn<Columns> label="Schacht" index={Columns::SchachtZ}/>
                </TableHeader<Columns>>
            };
            let onrowclick = ctx.props().on_select.clone();
            html! {
                <>
                    <Table<Columns, ListModel<Columns, MemoizedTableModel<DuctListEntry>>>
                        mode={TableMode::Compact}
                        grid={TableGridMode::Medium}
                        caption="Trassen"
                        {onrowclick}
                        {header}
                        {entries}
                    />
                </>
            }
        } else {
            html!(<Spinner/>)
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            if let Some((credentials, _)) = scope.context::<OAuth2Context>(Callback::noop()) {
                spawn_local(async move {
                    scope.send_message(match list_all_ducts(Some(&credentials)).await {
                        Ok(data) => Msg::Data(data),
                        Err(error) => Msg::Error(error),
                    });
                });
            }
        }
    }
}
