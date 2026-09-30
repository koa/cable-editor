//! Tells installations apart: with `instance_name` set (every installation but the productive
//! one) the installed app gets another name, colour and icon (`assets/icons/dev`), so it can
//! be told apart on a phone at first sight.

use std::borrow::Cow;

const APP_NAME: &str = "Cable Editor";
const THEME_COLOR: &str = "#c2410c";
/// The icons with a variant in `icons/dev`, by the file name.
const ICONS: [&str; 6] = [
    "icon.svg",
    "icon-192.png",
    "icon-512.png",
    "favicon.svg",
    "favicon-32.png",
    "apple-touch-icon.png",
];

/// Where the icon at `path` of an instance lies, `None` for anything else.
pub fn icon_path(path: &str) -> Option<String> {
    let name = path.strip_prefix("icons/")?;
    ICONS.contains(&name).then(|| format!("icons/dev/{name}"))
}

/// The manifest with the name of the instance and its colour.
pub fn manifest(manifest: &[u8], instance: &str) -> Option<Box<[u8]>> {
    let mut manifest: serde_json::Value = serde_json::from_slice(manifest).ok()?;
    let object = manifest.as_object_mut()?;
    object.insert("name".into(), format!("{APP_NAME} ({instance})").into());
    object.insert("short_name".into(), format!("Cable {instance}").into());
    object.insert("theme_color".into(), THEME_COLOR.into());
    object.insert("background_color".into(), THEME_COLOR.into());
    serde_json::to_vec_pretty(&manifest).ok().map(Box::from)
}

/// The page with the name of the instance in its title.
pub fn index(index: &[u8], instance: &str) -> Option<Box<[u8]>> {
    let index = std::str::from_utf8(index).ok()?;
    let title = format!("<title>{APP_NAME}</title>");
    index.contains(&title).then(|| {
        let name = format!("{APP_NAME} ({instance})");
        let title_with_name = format!("<title>{}</title>", escape_html(&name));
        index
            .replacen(&title, &title_with_name, 1)
            .into_bytes()
            .into()
    })
}

/// The file changed for the instance, `None` for those that stay as they are.
pub fn adapt(path: &str, data: &[u8], instance: &str) -> Option<Box<[u8]>> {
    match path {
        "manifest.webmanifest" => manifest(data, instance),
        "index.html" => index(data, instance),
        _ => None,
    }
}

fn escape_html(text: &str) -> Cow<'_, str> {
    if !text.contains(['&', '<', '>']) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_icons() {
        assert_eq!(
            icon_path("icons/icon-192.png").as_deref(),
            Some("icons/dev/icon-192.png")
        );
        assert_eq!(icon_path("icons/dev/icon.svg"), None);
        assert_eq!(icon_path("icon.svg"), None);
        assert_eq!(icon_path("index.html"), None);
    }

    #[test]
    fn names_the_instance_in_the_manifest() {
        let manifest = manifest(
            br##"{"name": "Cable Editor", "short_name": "Cable Editor", "theme_color": "#12324f", "icons": []}"##,
            "Dev",
        )
        .expect("valid manifest");
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).expect("JSON");
        assert_eq!(manifest["name"], "Cable Editor (Dev)");
        assert_eq!(manifest["short_name"], "Cable Dev");
        assert_eq!(manifest["theme_color"], THEME_COLOR);
        assert_eq!(manifest["icons"], serde_json::json!([]));
    }

    #[test]
    fn names_the_instance_in_the_title() {
        let page = index(b"<head><title>Cable Editor</title></head>", "T&st").expect("title");
        assert_eq!(
            String::from_utf8(page.into_vec()).expect("UTF-8"),
            "<head><title>Cable Editor (T&amp;st)</title></head>"
        );
        assert!(index(b"<head></head>", "Dev").is_none());
    }
}
