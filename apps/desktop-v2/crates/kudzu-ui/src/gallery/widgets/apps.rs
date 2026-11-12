//! `app-icon` and `app-picker`. The picker stays open so the list is in the tree.
//! The dashed control is the same face as AppPicker’s add button, keyed on its own
//! so the sample size does not depend on the live list.

use std::sync::Arc;

use kudzu_core::system::AppInfo;
use rune::{col, row, Cx, El};

use crate::gallery::GalleryState;
use crate::theme::{icons, Theme};
use crate::widgets::app_icon::{AppIcon, IconCache};
use crate::widgets::app_picker::AppPicker;
use crate::widgets::kit_ext::button::{Button, Size, Variant};

/// A 16 pt monogram. The key replaces `app-icon-mono` after `build`.
pub fn app_icon(theme: &Theme) -> El {
    row().items_start().child(
        AppIcon::new("Mail")
            .size(16.0)
            .build(theme)
            .key("spec:app-icon/monogram:16x16"),
    )
}

/// One open picker, plus a copy of its add button at the sm height.
pub fn app_picker(
    theme: &Theme,
    s: &GalleryState,
    cx: &Cx<GalleryState>,
    overlays: &mut Vec<El>,
) -> El {
    let value = vec!["com.apple.mail".to_owned()];
    let apps = vec![AppInfo {
        bundle_id: "com.apple.mail".into(),
        name: "Mail".into(),
        pid: None,
        path: None,
    }];
    let icons = IconCache::new(Arc::new(|| {}));
    let chips = AppPicker::new("gallery-app-picker", &value, &apps, &s.scratch, &icons)
        .open(true)
        .active(0)
        .overlays(overlays)
        .build(
            theme,
            cx,
            |st| &mut st.scratch,
            |_, _| {},
            |_, _| {},
            |_, _| {},
        );
    // same face as AppPicker's add button
    let add = Button::new("Add app")
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .ink(theme.c.muted_foreground)
        .icon(&icons::PLUS)
        .build(theme)
        .key("spec:app-picker/add:_x28")
        .border(1.0, theme.c.border)
        .border_dash(3.0, 3.0);
    col()
        .key("info:app-picker/open")
        .gap(16.0)
        .items_start()
        .child(chips)
        .child(add)
}
