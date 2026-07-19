use crate::{
    components::map_edit::{NoData, ReferencedData},
    util::render_component,
};
use leaflet::{DragEvents, LatLng, LatLngBounds, Marker, MarkerOptions, MouseEvents};
use log::info;
use patternfly_yew::prelude::Spinner;
use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, hash_map},
    fmt::Debug,
    hash::{Hash, Hasher},
    rc::Rc,
};
use tokio::sync::mpsc;
use wasm_bindgen::JsValue;
use yew::{Html, function_component, html, platform::spawn_local};

pub trait MarkerLoader: Clone + PartialEq + Debug + Hash + Eq {
    type Data: ReferencedData;

    async fn list_points(
        &self,
        bounds: &LatLngBounds,
    ) -> Box<[<<Self as MarkerLoader>::Data as ReferencedData>::Key]>;
    async fn create_entry(&self, point: &LatLng) -> Self::Data;
    async fn fetch_data(
        &self,
        key: &<<Self as MarkerLoader>::Data as ReferencedData>::Key,
    ) -> Option<Self::Data>;
    fn render(&self, data: &Self::Data) -> Marker;

    fn move_marker(&self, _key: &Self::Data, point: &LatLng, marker: Marker) {
        marker.set_lat_lng(point);
    }
}

#[derive(Clone, PartialEq, Copy, Debug, Hash, Eq)]
pub enum NoDynamicMarkerLayer {}

impl MarkerLoader for NoDynamicMarkerLayer {
    type Data = NoData;

    async fn list_points(
        &self,
        bounds: &LatLngBounds,
    ) -> Box<[<<Self as MarkerLoader>::Data as ReferencedData>::Key]> {
        match *self {}
    }

    async fn create_entry(&self, point: &LatLng) -> Self::Data {
        match *self {}
    }

    async fn fetch_data(
        &self,
        key: &<<Self as MarkerLoader>::Data as ReferencedData>::Key,
    ) -> Option<Self::Data> {
        match *self {}
    }

    fn render(&self, _data: &Self::Data) -> Marker {
        match *self {}
    }
}

#[derive(Clone, Debug)]
pub struct HashmapLayer {
    inner: Rc<RefCell<InnerHashmapLayer>>,
    sender: mpsc::Sender<Msg>,
}
#[derive(Debug)]
enum Msg {
    Moved { id: u32, new_pos: LatLng },
    Clicked { id: u32 },
}
impl Default for HashmapLayer {
    fn default() -> Self {
        let (sender, mut receiver) = mpsc::channel::<Msg>(1);

        let inner = Rc::new(RefCell::new(InnerHashmapLayer::default()));
        {
            let inner = inner.clone();
            spawn_local(async move {
                while let Some(msg) = receiver.recv().await {
                    match msg {
                        Msg::Moved { id, new_pos } => {
                            let mut map = inner.borrow_mut();
                            if let Some(e) = map.entries.get_mut(&id) {
                                e.position = new_pos;
                            }
                        }
                        Msg::Clicked { id } => {
                            info!("Clicked on {id}");
                        }
                    }
                }
            });
        }
        HashmapLayer { inner, sender }
    }
}

impl PartialEq for HashmapLayer {
    fn eq(&self, other: &HashmapLayer) -> bool {
        RefCell::as_ptr(&self.inner) == RefCell::as_ptr(&other.inner)
    }
}
impl Hash for HashmapLayer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        RefCell::<InnerHashmapLayer>::as_ptr(&self.inner).hash(state);
    }
}
impl Eq for HashmapLayer {}

#[derive(Debug, Default)]
struct InnerHashmapLayer {
    next_id: u32,
    entries: HashMap<u32, HashmapLayerEntry>,
}
#[derive(Clone, Debug)]
pub struct HashmapLayerEntry {
    pub id: u32,
    pub position: LatLng,
}
impl PartialEq<HashmapLayerEntry> for HashmapLayerEntry {
    fn eq(&self, other: &HashmapLayerEntry) -> bool {
        self.id == other.id
    }
}

impl MarkerLoader for HashmapLayer {
    type Data = HashmapLayerEntry;

    async fn list_points(
        &self,
        bounds: &LatLngBounds,
    ) -> Box<[<<Self as MarkerLoader>::Data as ReferencedData>::Key]> {
        self.inner
            .borrow()
            .entries
            .iter()
            .filter(|(_, e)| bounds.contains(&e.position))
            .map(|(id, e)| *id)
            .collect()
    }

    async fn create_entry(&self, point: &LatLng) -> Self::Data {
        let mut ref_mut = self.inner.borrow_mut();
        let next_id = ref_mut.next_id;
        let entry = HashmapLayerEntry {
            id: next_id,
            position: point.clone(),
        };
        ref_mut.entries.insert(next_id, entry.clone());
        ref_mut.next_id += 1;
        entry
    }

    async fn fetch_data(
        &self,
        key: &<<Self as MarkerLoader>::Data as ReferencedData>::Key,
    ) -> Option<Self::Data> {
        self.inner.borrow().entries.get(key).cloned()
    }

    fn render(&self, key: &Self::Data) -> Marker {
        let marker_options = MarkerOptions::new();
        marker_options.set_draggable(true);
        let marker = Marker::new_with_options(&key.position, &marker_options);
        let id = key.id;
        let sender = self.sender.clone();
        {
            let marker_ref = marker.clone();
            marker.on_move_end(Box::from(move |_| {
                let new_pos = marker_ref.get_lat_lng();
                let sender = sender.clone();
                spawn_local(async move {
                    sender
                        .send(Msg::Moved { id, new_pos })
                        .await
                        .expect("receiver unavailable");
                });
            }));
        }
        let sender = self.sender.clone();
        let marker_ref = marker.clone();
        let marker_ref = marker.clone();
        marker.on_click(Box::from(move |_| {
            let (handle, element) = render_component::<MarkerPopup>(());
            info!("Created element: {:?}", element.outer_html());
            marker_ref.bind_popup_with_options(&element, &JsValue::null());

            let sender = sender.clone();
            let result = marker_ref.open_popup();
            info!("Result: {result:?}");

            spawn_local(async move {
                sender
                    .send(Msg::Clicked { id })
                    .await
                    .expect("receiver unavailable");
            });
            //info!("Handle: {handle:?}");
        }));
        marker
    }
}
impl ReferencedData for HashmapLayerEntry {
    type Key = u32;

    fn key_of(&self) -> Self::Key {
        self.id
    }
}
impl HashmapLayer {
    pub fn with_entry<F, R>(&self, id: u32, f: F) -> R
    where
        F: FnOnce(hash_map::Entry<u32, HashmapLayerEntry>) -> R,
    {
        let mut inner = self.inner.borrow_mut();
        let entry = inner.entries.entry(id);
        f(entry)
    }
}

#[ouroboros::self_referencing]
pub struct HashmapLayerKeys<'a> {
    guard: Ref<'a, InnerHashmapLayer>,

    #[borrows(guard)]
    #[covariant]
    iter: hash_map::Keys<'this, u32, HashmapLayerEntry>,
}

// Ouroboros generiert Code, sodass du das Struct als Iterator nutzen kannst:
impl<'a> Iterator for HashmapLayerKeys<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        // Über eine generierte Methode greifen wir auf den Iterator zu
        self.with_iter_mut(|iter| iter.next().copied())
    }
}

impl HashmapLayer {
    pub fn keys(&self) -> HashmapLayerKeys<'_> {
        HashmapLayerKeysBuilder {
            guard: self.inner.borrow(),
            iter_builder: |guard| guard.entries.keys(),
        }
        .build()
    }
}

#[function_component]
fn MarkerPopup() -> Html {
    html! {
        <>
        {"Hello popup"}
        <Spinner/>
        </>
    }
}
