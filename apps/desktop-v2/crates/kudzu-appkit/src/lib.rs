//! RUNE-GAP(#141, #561, #562, #563, #146, #574): delete each adapter with its upstream fix.
//! Isolated, main-thread AppKit adapters. No framework or application dependencies.
pub mod app_bundle;
pub mod app_menu;
pub mod appearance;
pub mod current_event;
pub mod geometry;
pub mod guide_panel;
mod menu_action;
pub mod panel_tuning;
pub mod screens;
pub mod settings_window;
pub mod sidebar_material;
pub mod status_item;
pub mod window;

#[cfg(test)]
mod model_tests;
