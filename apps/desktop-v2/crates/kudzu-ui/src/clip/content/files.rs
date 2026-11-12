//! Copied files: up to 200 paths with their Finder icons, Open and Show in Finder on each,
//! and a preview above them when the single file is an image.

use kudzu_core::clipboard::facts::{basename, dirname, tildify};
use rune::{col, row, text, El, SurfaceFit, TextStyle};

use super::loader::Want;
use super::{card, checkerboard, divider, well, Body, Hooks, FILE_LIMIT};
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

fn icon_tile(theme: &Theme, b: &Body, index: usize) -> El {
    let slot = row().size(32.0, 32.0).center();
    match b
        .media
        .and_then(|m| m.pixels(b.entry.id, Want::Icon(index)))
    {
        Some(px) => slot.child(
            col()
                .key(format!("clip-file-icon-{index}"))
                .size(32.0, 32.0)
                .pixels_fit(&px, SurfaceFit::Fit),
        ),
        None => slot.child(
            row()
                .size(32.0, 32.0)
                .radius(8.0)
                .fill(theme.c.foreground.alpha(0.06)),
        ),
    }
}

fn file_row<S: 'static>(theme: &Theme, b: &Body, index: usize, path: &str, hooks: Hooks<S>) -> El {
    let c = &theme.c;
    let name = basename(path).to_owned();
    let actions = row()
        .gap(2.0)
        .child(
            Button::new(format!("Open {name}"))
                .variant(Variant::Ghost)
                .size(Size::IconXs)
                .icon(&icons::EXTERNAL_LINK)
                .help("Open")
                .on_press_tx(move |s: &mut S, tx| (hooks.open_file)(s, index, tx))
                .build(theme),
        )
        .child(
            Button::new(format!("Show {name} in Finder"))
                .variant(Variant::Ghost)
                .size(Size::IconXs)
                .icon(&icons::FOLDER_OPEN)
                .help("Show in Finder")
                .on_press_tx(move |s: &mut S, tx| (hooks.reveal_file)(s, index, tx))
                .build(theme),
        )
        .group_opacity(0.0, 1.0);
    row()
        .key(format!("clip-file-{index}"))
        .w_full()
        .px(12.0)
        .py(11.0)
        .gap(12.0)
        .center_y()
        .hover_group_focus()
        .child(icon_tile(theme, b, index))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(
                    text(name)
                        .font(TextStyle {
                            line_height: 19.5,
                            ..TextStyle::new(13.0, 500.0)
                        })
                        .color(c.foreground)
                        .truncate(),
                )
                .child(
                    text(tildify(dirname(path)))
                        .font(TextStyle {
                            line_height: 16.5,
                            ..TextStyle::new(11.0, 400.0).family(theme.ty.mono.family)
                        })
                        .color(c.muted_foreground)
                        .truncate()
                        .selectable(true),
                ),
        )
        .child(actions)
}

pub fn view<S: 'static>(theme: &Theme, b: &Body, hooks: Hooks<S>) -> El {
    let c = &theme.c;
    let files = &b.entry.files;
    let mut column = col().key("clip-files-body").w_full().gap(12.0);
    if let (1, Some(_)) = (files.len(), b.entry.image) {
        let height = if b.large { 560.0 } else { 224.0 };
        // The file itself, else its thumbnail, else nothing (`clip-content.tsx:243`).
        let px = b
            .media
            .and_then(|m| m.first_ready(b.entry.id, &[Want::File(0), Want::Thumbnail]));
        let mut preview = well(theme)
            .role("img")
            .label(format!("Preview of {}", basename(&files[0])))
            .h(height)
            .padding(16.0)
            .center()
            .clip_rounded(14.0);
        if let Some(px) = px {
            preview = preview.child(checkerboard(theme, 920.0, height)).child(
                col()
                    .key("clip-file-preview-image")
                    .w_full()
                    .h_full()
                    .pixels_fit(&px, SurfaceFit::Fit),
            );
        }
        column = column.child(preview);
    }
    let mut list = card(theme).role("list").label("Copied files");
    for (i, path) in files.iter().take(FILE_LIMIT).enumerate() {
        if i > 0 {
            list = list.child(divider(theme));
        }
        list = list.child(file_row(theme, b, i, path, hooks));
    }
    column = column.child(list);
    if files.len() > FILE_LIMIT {
        column = column.child(
            text(format!("…and {} more.", files.len() - FILE_LIMIT))
                .font(theme.ty.control_xs)
                .color(c.faint),
        );
    }
    column
}
