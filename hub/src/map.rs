//! Meeting-spot pin picker, built on the self-hosted Leaflet in
//! `public/leaflet/` (loaded by index.html). The glue below is the whole
//! Leaflet surface we use: initialise an interactive map in a div, drop/move a
//! single marker on tap, and report the coordinates back through a Rust
//! closure. Tiles come from OpenStreetMap and load ONLY here in the picker —
//! ride cards render plain "open in maps" links instead of embedding a map, so
//! browsing rides never touches a tile server.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const registry = {};

export function bb_map_init(id, lat, lng, onPick) {
  const L = window.L;
  if (!L) return;

  // Leaflet auto-detects its marker images from the script URL, which breaks
  // when vendored; point it at our copies explicitly (once).
  if (!window.__bbIconsSet) {
    window.__bbIconsSet = true;
    L.Icon.Default.mergeOptions({
      iconUrl: '/public/leaflet/marker-icon.png',
      iconRetinaUrl: '/public/leaflet/marker-icon-2x.png',
      shadowUrl: '/public/leaflet/marker-shadow.png',
    });
  }

  if (registry[id] || !document.getElementById(id)) return;

  const map = L.map(id).setView([lat, lng], 13);
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; OpenStreetMap',
  }).addTo(map);

  let marker = null;
  map.on('click', function (e) {
    if (marker) marker.setLatLng(e.latlng);
    else marker = L.marker(e.latlng).addTo(map);
    onPick(e.latlng.lat, e.latlng.lng);
  });

  registry[id] = {
    map,
    clear() { if (marker) { map.removeLayer(marker); marker = null; } },
    destroy() { map.remove(); delete registry[id]; },
  };

  // The div is often laid out after init (inside a card); nudge Leaflet to
  // recompute tile geometry once it has a real size.
  setTimeout(function () { map.invalidateSize(); }, 60);
}

export function bb_map_clear(id) { const m = registry[id]; if (m) m.clear(); }
export function bb_map_destroy(id) { const m = registry[id]; if (m) m.destroy(); }
"#)]
extern "C" {
    /// Initialise a Leaflet map in the div with the given id, centred on
    /// `(lat, lng)`. Tapping the map drops/moves a marker and calls `on_pick`
    /// with the chosen coordinates. No-op if Leaflet is missing or the id is
    /// already initialised.
    #[wasm_bindgen(js_name = bb_map_init)]
    pub fn init(id: &str, lat: f64, lng: f64, on_pick: &Closure<dyn FnMut(f64, f64)>);

    /// Remove the marker (if any) without tearing down the map.
    #[wasm_bindgen(js_name = bb_map_clear)]
    pub fn clear(id: &str);

    /// Tear the map down entirely (used on unmount).
    #[wasm_bindgen(js_name = bb_map_destroy)]
    #[allow(dead_code)]
    pub fn destroy(id: &str);
}
