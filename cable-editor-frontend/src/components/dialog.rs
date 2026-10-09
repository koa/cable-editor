use crate::util::get_backdrop;
use patternfly_yew::prelude::{
    Backdrop, Backdropper, Bullseye, Button, ButtonVariant, Modal, ModalVariant,
};
use web_sys::MouseEvent;
use yew::{
    AttrValue, BaseComponent, Callback, Html, Properties, function_component, html, html::Scope,
};

/// The click handler of a delete button: asks in the page's backdrop and calls `on_confirm`
/// only when confirmed. `kind` is the kind of object ("Schacht"), `name` what it is called.
pub fn confirm_delete(
    scope: &Scope<impl BaseComponent>,
    kind: &'static str,
    name: &str,
    on_confirm: Callback<()>,
) -> Callback<MouseEvent> {
    let Some(backdropper) = get_backdrop(scope) else {
        return Callback::noop();
    };
    let name = AttrValue::from(name.to_string());
    Callback::from(move |_| {
        open_delete_confirmation(&backdropper, kind, name.clone(), on_confirm.clone());
    })
}

/// Like `confirm_delete`, for several objects at once: "<count> <kinds> werden gelöscht".
pub fn confirm_delete_all(
    scope: &Scope<impl BaseComponent>,
    kinds: &'static str,
    count: usize,
    on_confirm: Callback<()>,
) -> Callback<MouseEvent> {
    let Some(backdropper) = get_backdrop(scope) else {
        return Callback::noop();
    };
    let text = AttrValue::from(format!(
        "{count} {kinds} werden gelöscht. Das lässt sich nicht rückgängig machen."
    ));
    Callback::from(move |_| {
        open_confirmation(&backdropper, kinds, text.clone(), on_confirm.clone());
    })
}

/// Like `confirm_delete`, for a deletion started elsewhere, e.g. from a menu item.
pub fn ask_delete(
    scope: &Scope<impl BaseComponent>,
    kind: &'static str,
    name: &str,
    on_confirm: Callback<()>,
) {
    if let Some(backdropper) = get_backdrop(scope) {
        open_delete_confirmation(&backdropper, kind, name.to_string().into(), on_confirm);
    }
}

fn open_delete_confirmation(
    backdropper: &Backdropper,
    kind: &'static str,
    name: AttrValue,
    on_confirm: Callback<()>,
) {
    let text = format!("{kind} „{name}“ wird gelöscht. Das lässt sich nicht rückgängig machen.");
    open_confirmation(backdropper, kind, text.into(), on_confirm);
}

/// `kind` names what is deleted in the title, `text` says what happens.
fn open_confirmation(
    backdropper: &Backdropper,
    kind: &'static str,
    text: AttrValue,
    on_confirm: Callback<()>,
) {
    let on_confirm = {
        let backdropper = backdropper.clone();
        Callback::from(move |()| {
            backdropper.close();
            on_confirm.emit(());
        })
    };
    let on_cancel = {
        let backdropper = backdropper.clone();
        Callback::from(move |()| backdropper.close())
    };
    backdropper.open(Backdrop::new(html! {
        <DeleteConfirmationDialog {kind} {text} {on_confirm} {on_cancel}/>
    }));
}

/// Asks before deleting, opened by `confirm_delete`.
#[derive(Debug, Clone, PartialEq, Properties)]
pub struct DeleteConfirmationDialogProperties {
    pub kind: &'static str,
    /// What happens, naming what is deleted
    pub text: AttrValue,
    #[prop_or_default]
    pub on_confirm: Callback<()>,
    #[prop_or_default]
    pub on_cancel: Callback<()>,
}

#[function_component]
pub fn DeleteConfirmationDialog(props: &DeleteConfirmationDialogProperties) -> Html {
    let footer = html! {
        <>
            <Button
                label="Löschen"
                onclick={props.on_confirm.reform(|_| ())}
                variant={ButtonVariant::Danger}
            />
            <Button
                label="Abbrechen"
                onclick={props.on_cancel.reform(|_| ())}
                variant={ButtonVariant::Link}
            />
        </>
    };
    html! {
        <Bullseye>
            <Modal
                title={format!("{} löschen?", props.kind)}
                variant={ModalVariant::Small}
                {footer}
            >
                <p>{props.text.clone()}</p>
            </Modal>
        </Bullseye>
    }
}
