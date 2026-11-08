//! `formatting-toolbar.tsx`: Plain text / Formatted, then (formatted) bold, italic, underline,
//! a link, an image, bulleted and numbered lines, and Remove formatting. Every change is one
//! undo step in the editor's history.

use kudzu_core::client::editor_formatting::FormatChange;
use kudzu_core::client::formatting::safe_link;
use kudzu_core::formatting::FormatStyle;
use rune::{col, el, row, text, Align, Cx, El, Icon, TextEdit};

use super::template::now;
use super::{ensure, template_changed, EditorState};
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::input::Input;
use crate::ui::kit::popover::Popover;
use crate::ui::kit::segmented::Segmented;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::main::MainUi;
use crate::ui::theme::{text as ty, weight, WeightOf};
use crate::ui::theme_for;

#[derive(Default, Clone)]
pub struct ToolbarState {
    pub link_open: bool,
    pub url: TextEdit,
    pub error: String,
}

fn kind_matches(a: &FormatStyle, b: &FormatStyle) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Whether a style covers the whole (widened) selection.
fn active(st: &EditorState, kind: &FormatStyle) -> bool {
    let d = &st.template.doc;
    let (from, to) = d.selected_range();
    d.spans().is_some_and(|spans| spans.iter().any(|s| kind_matches(&s.style, kind) && s.start <= from && s.end >= to))
}

fn apply(s: &mut MainUi, change: FormatChange) {
    ensure(s);
    if s.editor.template.composing() {
        return;
    }
    let changed = s.editor.template.doc.change_format(&change, now());
    s.editor.template.focus();
    if changed {
        template_changed(s);
    }
}

fn set_mode(s: &mut MainUi, formatted: bool) {
    ensure(s);
    let t = &mut s.editor.template;
    if t.composing() || formatted == t.doc.formatted() {
        return;
    }
    t.doc.set_spans(if formatted { Some(Vec::new()) } else { None }, now());
    t.focus();
    template_changed(s);
}

fn submit_link(s: &mut MainUi) {
    ensure(s);
    let value = kudzu_core::client::js::trim(&s.editor.toolbar.url.text).to_string();
    if !safe_link(&value) {
        s.editor.toolbar.error = "Use an https://, http://, or mailto: link.".into();
        return;
    }
    let d = &s.editor.template.doc;
    let (from, to) = d.selected_range();
    let same = d.spans().unwrap_or_default().iter().any(|sp| sp.start <= from && sp.end >= to && matches!(&sp.style, FormatStyle::Link { url } if *url == value));
    if !same {
        apply(s, FormatChange::Style(FormatStyle::Link { url: value }));
    }
    s.editor.toolbar.link_open = false;
    s.editor.template.focus();
}

fn open_link(s: &mut MainUi, open: bool) {
    ensure(s);
    if open {
        let d = &s.editor.template.doc;
        let (from, to) = d.selected_range();
        let url = d.spans().unwrap_or_default().iter().find_map(|sp| match &sp.style {
            FormatStyle::Link { url } if sp.start <= from && sp.end >= to => Some(url.clone()),
            _ => None,
        });
        s.editor.toolbar.url = TextEdit::new(url.unwrap_or_default());
        s.editor.toolbar.url.focused = true;
        s.editor.toolbar.error.clear();
    } else {
        s.editor.template.focus();
    }
    s.editor.toolbar.link_open = open;
}

pub fn view(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let d = &st.template.doc;
    let formatted = d.formatted();
    let (from, to) = d.selected_range();
    let selected = from != to;
    let seg = Segmented::new("editor-format", "Snippet format")
        .option("plain", "Plain text")
        .option("formatted", "Formatted")
        .selected(if formatted { "formatted" } else { "plain" })
        .on_change(|s: &mut MainUi, v: &str| set_mode(s, v == "formatted"))
        .build(th, cx);
    let mut bar = row()
        .key("format-toolbar")
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(2.0)
        .px(8.0)
        .py(6.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .border_layout()
        .role("group")
        .label("Text formatting")
        .child(el().mr(8.0).child(seg));
    if !formatted {
        return bar;
    }
    let styled = |icon: &'static Icon, label: &str, style: FormatStyle, id: &str| {
        let on = active(st, &style);
        let mut b = Button::icon_only(icon, label.to_string()).variant(Variant::Ghost).size(Size::IconSm).disabled(!selected).id(id.to_string());
        b = if on { b.color(t.foreground) } else { b.color(t.muted_foreground) };
        let el = b.on_press(move |s: &mut MainUi| apply(s, FormatChange::Style(style.clone()))).build(th).checked(on);
        if on { el.fill(t.accent) } else { el }
    };
    for (icon, label, style, combo) in [
        (&icons::LETTER_B, "Bold", FormatStyle::Bold, "mod+b"),
        (&icons::ITALIC, "Italic", FormatStyle::Italic, "mod+i"),
        (&icons::UNDERLINE, "Underline", FormatStyle::Underline, "mod+u"),
    ] {
        let b = styled(icon, label, style, &format!("format-{label}"));
        bar = bar.child(Tooltip::new(format!("format-{label}-tip")).text(label).shortcut(combo).build(cx, b, layer));
    }

    // Link.
    let link_on = active(st, &FormatStyle::Link { url: String::new() });
    let mut link_btn = Button::icon_only(&icons::LINK, "Add or edit link").variant(Variant::Ghost).size(Size::IconSm).disabled(!selected).id("format-link");
    link_btn = if link_on { link_btn.color(t.foreground) } else { link_btn.color(t.muted_foreground) };
    let link_btn = link_btn.build(th).checked(link_on);
    let link_btn = if link_on { link_btn.fill(t.accent) } else { link_btn };
    let mut link = Popover::new("format-link-pop")
        .open(st.toolbar.link_open && selected)
        .on_open_change(|s: &mut MainUi, open, _| open_link(s, open))
        .trigger(link_btn)
        .align(Align::Start)
        .width(288.0)
        .label("Link address");
    if st.toolbar.link_open {
        let input = Input::new(&st.toolbar.url)
            .placeholder("https://example.com")
            .label("Link address")
            .invalid(!st.toolbar.error.is_empty())
            .on_edit(|s: &mut MainUi| {
                ensure(s);
                &mut s.editor.toolbar.url
            })
            .on_submit(|s: &mut MainUi, _| submit_link(s))
            .build(th);
        let mut form = col().gap(8.0).child(text("Link address").font(ty::XS.weight_of(weight::MEDIUM)).color(t.foreground)).child(input);
        if !st.toolbar.error.is_empty() {
            form = form.child(text(st.toolbar.error.clone()).key("error").font(ty::XS).color(t.destructive).wrap().role("alert"));
        }
        form = form.child(Button::new("Apply link").size(Size::Sm).full_width().id("format-link-apply").on_press(submit_link).build(th).key("apply"));
        link = link.child(form);
    }
    bar = bar.child(link.build(cx, layer));

    // Image.
    bar = bar.child(super::image_tools::view(s, st, cx, layer));

    bar = bar.child(el().mx(4.0).size(1.0, 16.0).fill(t.border));
    for (icon, label, ordered) in [(&icons::LIST, "Bulleted list", false), (&icons::LIST_ORDERED, "Numbered list", true)] {
        let b = Button::icon_only(icon, label)
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .color(t.muted_foreground)
            .id(format!("format-{label}"))
            .on_press(move |s: &mut MainUi| {
                ensure(s);
                if s.editor.template.composing() {
                    return;
                }
                s.editor.template.doc.toggle_list(ordered, now());
                s.editor.template.focus();
                template_changed(s);
            })
            .build(th);
        bar = bar.child(Tooltip::new(format!("format-{label}-tip")).text(label).build(cx, b, layer));
    }
    let clear = Button::icon_only(&icons::REMOVE_FORMATTING, "Remove formatting")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .color(t.muted_foreground)
        .disabled(!selected)
        .id("format-clear")
        .on_press(|s: &mut MainUi| apply(s, FormatChange::Clear))
        .build(th);
    bar.child(Tooltip::new("format-clear-tip").text("Remove formatting").build(cx, clear, layer))
}
