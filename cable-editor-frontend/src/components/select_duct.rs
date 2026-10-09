use crate::components::load::Load;
use crate::{
    components::table::ListTable,
    error::FrontendError,
    graphql::authenticated::list_ducts::{DuctListEntry, fetch_duct_list},
    util::get_credentials,
};
use patternfly_yew::prelude::{Cell, CellContext, TableColumn, TableEntryRenderer, TableHeader};
use std::rc::Rc;
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

#[derive(Debug, Default)]
pub struct SelectDuct {
    found_ducts: Load<Rc<Vec<DuctListEntry>>>,
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

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.found_ducts = Load::Loaded(Rc::new(data.into_vec()));
                true
            }
            Msg::Error(error) => {
                self.found_ducts = Load::Failed(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        self.found_ducts.view(|table| {
            let header = html_nested! {
                <TableHeader<Columns>>
                    <TableColumn<Columns> label="Schacht" index={Columns::SchachtA}/>
                    <TableColumn<Columns> label="Länge" index={Columns::Length}/>
                    <TableColumn<Columns> label="Schacht" index={Columns::SchachtZ}/>
                </TableHeader<Columns>>
            };
            let onrowclick = ctx.props().on_select.clone();
            html! {
                <ListTable<Columns, DuctListEntry>
                    {header}
                    rows={table.clone()}
                    empty="Keine Trassen."
                    caption="Trassen"
                    {onrowclick}
                />
            }
        })
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            let credentials = get_credentials(&scope);
            spawn_local(async move {
                scope.send_message(match fetch_duct_list(credentials.as_ref()).await {
                    Ok(data) => Msg::Data(data),
                    Err(error) => Msg::Error(error),
                });
            });
        }
    }
}
