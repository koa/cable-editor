use crate::components::{
    map::Point,
    map_edit::{Layer, MapEditor, marker::HashmapLayer},
};
use leaflet::{TileLayerWms, TileLayerWmsOptions};
use yew::{Html, function_component, html};

#[function_component]
pub fn MapTestPage() -> Html {
    let av_options = TileLayerWmsOptions::default();
    av_options.set_layers("ch.kantone.cadastralwebmap-farbe".to_string());
    av_options.set_format("image/png".to_string());
    //av_options.set_detect_retina(true);
    av_options.set_transparent(true);
    av_options.set_version("1.3.0".to_string());
    av_options.set_max_zoom(20.0);
    av_options.set_min_zoom(17.0);
    let av_layer = TileLayerWms::new_options("https://wms.geo.admin.ch/", &av_options);
    let basemap_options = TileLayerWmsOptions::default();
    basemap_options.set_layers("ch.swisstopo.pixelkarte-farbe".to_string());
    basemap_options.set_format("image/jpeg".to_string());
    basemap_options.set_detect_retina(true);
    basemap_options.set_max_zoom(17.0);
    let topo_layer = TileLayerWms::new_options("https://wms.geo.admin.ch/", &basemap_options);
    let layers = Box::from([
        Layer::PassiveTileLayer(av_layer),
        Layer::PassiveTileLayer(topo_layer),
        Layer::DynamicPointLayer(HashmapLayer::default()),
    ]);
    html!(<MapEditor<HashmapLayer> {layers} center={Point( 47.417986,8.882440)} zoom={15.0}/>)
}
