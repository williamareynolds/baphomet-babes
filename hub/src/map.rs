//! Meeting-spot pin picker, built on the self-hosted Leaflet in
//! `public/leaflet/` (loaded by index.html). The glue below is the whole
//! Leaflet surface we use: initialise an interactive map in a div, drop/move a
//! single marker on tap, and report the coordinates back through a Rust
//! closure. Tiles come from OpenStreetMap and load ONLY here in the picker —
//! ride cards render plain "open in maps" links instead of embedding a map, so
//! browsing rides never touches a tile server.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r##"
const registry = {};

// Marker as an inline-SVG divIcon: no <img>, so no separate PNG request that
// could 404, get mis-pathed, or fail the retina swap (WebKit paints a broken
// <img> as a "?" box — icon + shadow gave two of them). The pin is DOM, drawn
// from the theme red, and needs no files from public/leaflet.
function bbPinIcon(L) {
  return L.divIcon({
    className: 'bb-pin',
    html:
      '<svg width="26" height="38" viewBox="0 0 26 38" xmlns="http://www.w3.org/2000/svg">' +
      '<path d="M13 0C5.82 0 0 5.82 0 13c0 9.75 13 25 13 25s13-15.25 13-25C26 5.82 20.18 0 13 0z" ' +
      'fill="#ee4b61" stroke="#0c0a0e" stroke-width="1.5"/>' +
      '<circle cx="13" cy="13" r="4.5" fill="#0c0a0e"/></svg>',
    iconSize: [26, 38],
    iconAnchor: [13, 38],
  });
}

export function bb_map_init(id, lat, lng, seed, onPick) {
  const L = window.L;
  if (!L) return;

  if (registry[id] || !document.getElementById(id)) return;

  const map = L.map(id).setView([lat, lng], 13);
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; OpenStreetMap',
  }).addTo(map);

  // `seed` pre-drops the marker at the centre (editing a ride that already has a
  // pin), so the picker opens showing the current spot instead of empty.
  let marker = seed ? L.marker([lat, lng], { icon: bbPinIcon(L) }).addTo(map) : null;
  map.on('click', function (e) {
    if (marker) marker.setLatLng(e.latlng);
    else marker = L.marker(e.latlng, { icon: bbPinIcon(L) }).addTo(map);
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
"##)]
extern "C" {
    /// Initialise a Leaflet map in the div with the given id, centred on
    /// `(lat, lng)`. Tapping the map drops/moves a marker and calls `on_pick`
    /// with the chosen coordinates. When `seed` is true a marker is pre-dropped
    /// at the centre (for editing a ride whose pin is already set). No-op if
    /// Leaflet is missing or the id is already initialised.
    #[wasm_bindgen(js_name = bb_map_init)]
    pub fn init(id: &str, lat: f64, lng: f64, seed: bool, on_pick: &Closure<dyn FnMut(f64, f64)>);

    /// Remove the marker (if any) without tearing down the map.
    #[wasm_bindgen(js_name = bb_map_clear)]
    pub fn clear(id: &str);

    /// Tear the map down entirely (used on unmount).
    #[wasm_bindgen(js_name = bb_map_destroy)]
    #[allow(dead_code)]
    pub fn destroy(id: &str);
}
