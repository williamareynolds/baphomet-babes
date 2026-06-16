//! The Baphomet Babes look, expressed as a Thaw theme.
//!
//! Instead of fighting Thaw's Fluent defaults per-component, we override its
//! design tokens once: a blood-red brand ramp over a near-black gothic neutral
//! palette, plus our serif/mono font pairing and sharper corners. Every Thaw
//! component (Button, Input, Card, Field, …) then inherits the aesthetic. Set
//! once at the root via `ConfigProvider` in [`crate::app`].

use std::collections::HashMap;
use thaw::Theme;

/// 16-step red ramp filling Fluent brand-ramp slots 10..=160 (dark → light),
/// centred so slot 70 is our signature `#c41e3a`. `Theme::custom_dark` reads
/// specific slots for the background / hover / foreground / link roles, so the
/// whole ramp must be present and monotonic.
const RED_RAMP: [(i32, &str); 16] = [
    (10, "#1c0407"),
    (20, "#2a0609"),
    (30, "#460b11"),
    (40, "#6c121b"), // brand_background_pressed
    (50, "#891626"),
    (60, "#a81a30"),
    (70, "#c41e3a"), // brand_background — the signature red
    (80, "#d62a40"), // brand_background_hover
    (90, "#dd3a4d"),
    (100, "#e44f5f"), // links / focus stroke
    (110, "#e9636f"), // brand_foreground_1
    (120, "#ed7782"),
    (130, "#f08c95"),
    (140, "#f4a0a8"),
    (150, "#f7b4bb"),
    (160, "#facace"),
];

/// Build the gothic dark theme used across the app.
pub fn gothic_theme() -> Theme {
    let ramp: HashMap<i32, &str> = RED_RAMP.into_iter().collect();
    let mut theme = Theme::custom_dark(&ramp);

    // --- Neutral surfaces: gothic near-black, not Thaw's default greys. ---
    let c = &mut theme.color;
    // Primary surface used by Card, Input, Select and the Button base.
    c.color_neutral_background_1 = "#130e18".into();
    c.color_neutral_background_1_hover = "#1a1320".into();
    c.color_neutral_background_1_pressed = "#0e0a13".into();
    // Deeper, page-level surfaces.
    c.color_neutral_background_3 = "#0c0a0e".into();
    c.color_neutral_background_3_hover = "#130e18".into();
    c.color_neutral_background_3_pressed = "#090709".into();
    c.color_neutral_background_static = "#0c0a0e".into();

    // --- Text. ---
    c.color_neutral_foreground_1 = "#e2d8d0".into(); // primary copy
    c.color_neutral_foreground_1_hover = "#f3ece6".into();
    c.color_neutral_foreground_1_pressed = "#f3ece6".into();
    c.color_neutral_foreground_2 = "#cabcc0".into();
    c.color_neutral_foreground_3 = "#8a7a7a".into(); // field labels / hints
    c.color_neutral_foreground_4 = "#6a5a6a".into();
    c.color_neutral_foreground_on_brand = "#ffffff".into();

    // --- Strokes / borders: faint, purple-tinted to read as "old". ---
    c.color_neutral_stroke_1 = "#2a2035".into();
    c.color_neutral_stroke_1_hover = "#4a3a5e".into();
    c.color_neutral_stroke_1_pressed = "#3a2c4a".into();
    c.color_neutral_stroke_2 = "#251e2c".into();
    c.color_neutral_stroke_accessible = "#6a5a6a".into();

    // --- Common: our font pairing + sharper, less "app-y" corners. ---
    let k = &mut theme.common;
    k.font_family_base = "'Crimson Pro', Georgia, 'Times New Roman', serif".into();
    k.font_family_monospace = "'IBM Plex Mono', ui-monospace, monospace".into();
    k.border_radius_small = "2px".into();
    k.border_radius_medium = "3px".into();
    k.border_radius_large = "4px".into();

    theme
}
