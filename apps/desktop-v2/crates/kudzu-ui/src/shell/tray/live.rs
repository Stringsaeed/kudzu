//! The live menu-bar item over `kudzu_appkit::status_item` (RUNE-GAP(#141)). Owned by `main`'s
//! state on the main thread; `Drop` removes it. Item callbacks only queue shell work.
use super::{Line, Model, Row};
use crate::shell::{Route, SettingsPane, Shell};

use kudzu_appkit::status_item::{Entry, Item, ItemId, StatusItem};
use objc2_foundation::MainThreadMarker;
use std::rc::Rc;

pub(crate) struct Tray {
    item: StatusItem,
    shown: Model,
}

impl Tray {
    pub(crate) fn install(
        mtm: MainThreadMarker,
        model: &Model,
        shell: &Shell,
    ) -> Result<Self, &'static str> {
        let entries = model
            .lines
            .iter()
            .map(|line| match line {
                Line::Separator => Entry::Separator,
                Line::Item(row) => Entry::Item(Item {
                    id: ItemId(row.row.id()),
                    title: row.title.clone(),
                    enabled: row.enabled,
                    tooltip: None,
                    shortcut: row.shortcut.clone(),
                    action: action(row.row, shell.clone()),
                }),
            })
            .collect();
        let item = StatusItem::new(
            mtm,
            &model.title,
            Some(&model.tooltip),
            model.glyph,
            entries,
        )?;
        Ok(Self {
            item,
            shown: model.clone(),
        })
    }

    /// Apply only what changed since the last refresh.
    pub(crate) fn refresh(&mut self, model: Model) {
        if model == self.shown {
            return;
        }
        if model.title != self.shown.title {
            self.item.set_status_title(&model.title);
        }
        if model.tooltip != self.shown.tooltip {
            self.item.set_status_tooltip(Some(&model.tooltip));
        }
        if model.glyph != self.shown.glyph {
            if let Err(error) = self.item.set_glyph(model.glyph) {
                log::warn!("the menu-bar glyph was not updated: {error}");
            }
        }
        for (new, old) in model.lines.iter().zip(&self.shown.lines) {
            let (Line::Item(new), Line::Item(old)) = (new, old) else {
                continue;
            };
            let id = ItemId(new.row.id());
            if new.title != old.title {
                self.item.set_title(id, &new.title);
            }
            if new.enabled != old.enabled {
                self.item.set_enabled(id, new.enabled);
            }
        }
        self.shown = model;
    }
}

fn action(row: Row, shell: Shell) -> Rc<dyn Fn()> {
    Rc::new(move || match row {
        Row::Status => {}
        Row::QuickSearch => shell.toggle_palette(),
        Row::Clipboard => shell.toggle_picker(),
        Row::FromSelection => shell.new_snippet_from_selection(),
        Row::Pause => shell.toggle_pause(),
        Row::Open => shell.show_main(),
        Row::Settings => shell.open_main(Route::Settings(SettingsPane::General)),
        Row::Updates => shell.update_row(),
        Row::Quit => shell.quit(),
    })
}
