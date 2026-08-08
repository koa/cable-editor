use patternfly_yew::prelude::*;
use yew::prelude::*; // Passe dies an deine Patternfly-Yew Imports an

#[macro_export]
macro_rules! create_simple_dialog {
    (
        $component_name:ident,
        $props_name:ident,
        $struct_name:ident,
        $( ($field_ident:ident, $field_label:expr) ),* $(,)?
    ) => {
        // 1. Generiere die Resultat-Struct
        #[derive(Default, Clone, PartialEq, Debug)]
        pub struct $struct_name {
            $( pub $field_ident: String, )*
        }

        // 2. Generiere die Properties für den Dialog
        #[derive(Properties, PartialEq)]
        pub struct $props_name {
            pub on_confirm: Callback<$struct_name>,
            pub on_cancel: Callback<()>,
        }

        // 3. Generiere die Yew Function Component
        #[function_component]
        pub fn $component_name(props: &$props_name) -> Html {
            // Lokaler State für das Formular
            let state = use_state(|| $struct_name::default());

            // Submit Handler
            let on_submit = {
                let state = state.clone();
                let on_confirm = props.on_confirm.clone();
                Callback::from(move |e: SubmitEvent| {
                    e.prevent_default(); // Verhindert den Page-Reload
                    on_confirm.emit((*state).clone());
                    state.set($struct_name::default()); // State nach Bestätigung zurücksetzen
                })
            };

            // Cancel Handler
            let on_cancel = {
                let on_cancel = props.on_cancel.clone();
                let state = state.clone();
                Callback::from(move |_| {
                    on_cancel.emit(());
                    state.set($struct_name::default()); // State beim Abbrechen zurücksetzen
                })
            };
            let onclose = {
                let on_cancel = props.on_cancel.clone();
                let state = state.clone();
                Callback::from(move |_| {
                    on_cancel.emit(());
                    state.set($struct_name::default()); // State beim Abbrechen zurücksetzen
                })
            };

            html! {
                <Modal
                    title="Eingabe"
                    {onclose}
                >
                    <Form onsubmit={on_submit}>
                        // Iteriere über alle Felder im Makro
                        $(
                            <FormGroup label={$field_label}>
                                <TextInput
                                    value={state.$field_ident.clone()}
                                    onchange={
                                        let state = state.clone();
                                        Callback::from(move |val: String| {
                                            let mut new_state = (*state).clone();
                                            new_state.$field_ident = val; // Feld updaten
                                            state.set(new_state);
                                        })
                                    }
                                />
                            </FormGroup>
                        )*

                        <ActionGroup>
                            <Button label="Speichern" variant={ButtonVariant::Primary} r#type={ButtonType::Submit}/>
                            <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={on_cancel} />
                        </ActionGroup>
                    </Form>
                </Modal>
            }
        }
    };
}
