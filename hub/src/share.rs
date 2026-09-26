//! Native share sheet for members-only content (gatherings, etc). There's no
//! public per-item URL to share — the whole app sits behind login — so a
//! share just points at the app's list page and lets the recipient (already
//! a member, or prompted to log in) find the thing themselves. Falls back to
//! copying the text to the clipboard on browsers without `navigator.share`
//! (most desktop browsers).

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r##"
export function bb_share(title, text, url, onStatus) {
  if (navigator.share) {
    // User-cancelled shares reject too; no status needed either way.
    navigator.share({ title, text, url }).catch(() => {});
    return;
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text + '\n' + url)
      .then(() => onStatus('Copied to clipboard.'))
      .catch(() => onStatus('Could not copy — share not supported here.'));
    return;
  }
  onStatus('Share not supported on this device.');
}
"##)]
extern "C" {
    /// Open the OS share sheet for (title, text, url); falls back to a
    /// clipboard copy with `on_status` reporting what happened when there's
    /// no native share sheet to give its own feedback.
    #[wasm_bindgen(js_name = bb_share)]
    pub fn share(title: &str, text: &str, url: &str, on_status: &Closure<dyn FnMut(String)>);
}
