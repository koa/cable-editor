use crate::components::panel::port_editor::PortEditor;
use yew::{Html, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct EditPanelProps {
    pub plan_id: i32,
    pub panel_id: i32,
}

#[function_component]
pub fn EditPanel(props: &EditPanelProps) -> Html {
    let panel_id = props.panel_id;
    html! {
        <PortEditor {panel_id}/>
    }
}
