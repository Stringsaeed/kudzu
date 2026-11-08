//! The fill-in form shown at the caret when a snippet has fields or dropdowns
//! (`components/app/form/form-panel.tsx`, `app/form/page.tsx`). Tab moves between fields,
//! Return inserts (⌘Return inside multi-line fields), Esc cancels and leaves the trigger exactly
//! as typed.
//!
//! Drawn as the Tauri window drew it over native vibrancy (`.vibrant`): the panel fills the
//! 440 pt window, whose 16 pt corners clip it, with `bg-popover/55` over the popover material.
//! The panel measures itself and hands its height to `windows::resize_form`, which keeps the
//! window under the caret (the web's `ResizeObserver` → `resizeForm`).

use std::collections::HashMap;
use std::sync::Arc;

use kudzu_core::formatting::Formatting;
use kudzu_core::template::{FormFieldKind, RenderResult};
use rune::{col, el, icon, row, text, text_input, Color, Cx, El, Key, PopupMenu, ScrollSpec, SystemMaterial, TextEdit, TextStyle, Tx};

use crate::backend::commands;
use crate::backend::core::{try_core, Event, FormPayload};
use crate::backend::windows;
use crate::ui::icons;
use crate::ui::kit::button::{layer, tw_spring};
use crate::ui::kit::formatted_text::FormattedText;
use crate::ui::kit::shortcut::{Shortcut, Tone};
use crate::ui::theme::{radius, weight, Theme, Utilities};
use crate::ui::theme_for;

/// `rounded-2xl`, the window's own corner.
const PANEL_RADIUS: f32 = windows::FORM_RADIUS;
/// `Math.min(h, 640)`.
const MAX_HEIGHT: f32 = 640.0;

pub struct FormUi {
    pub(crate) request: Option<Arc<FormPayload>>,
    pub(crate) values: HashMap<String, String>,
    /// One editor per field (unused for choices).
    edits: Vec<TextEdit>,
    /// The snippet's formatting, for the preview.
    formatting: Option<Formatting>,
    pub(crate) preview: Option<RenderResult>,
    /// The height last handed to the window.
    height: f32,
}

impl FormUi {
    pub fn init() -> FormUi {
        let mut s = FormUi { request: None, values: HashMap::new(), edits: Vec::new(), formatting: None, preview: None, height: 0.0 };
        // `api.pendingForm().then(load)`: a request that landed before this window existed.
        let pending = try_core().and_then(|_| commands::pending_form()).or_else(demo_payload);
        if let Some(p) = pending {
            s.load(Arc::new(p));
        }
        s
    }

    /// `load(r)`: the request, its remembered values, an empty preview, the first field focused.
    fn load(&mut self, r: Arc<FormPayload>) {
        self.values = r.initial_values.clone();
        self.edits = r
            .fields
            .iter()
            .map(|f| {
                let mut e = TextEdit::new(self.values.get(&f.name).cloned().unwrap_or_default());
                let end = e.text.len();
                e.set_caret(end);
                e
            })
            .collect();
        // `first.current?.focus()`; a dropdown first gets focus from the modal.
        if r.fields.first().is_some_and(|f| f.kind != FormFieldKind::Choice) {
            self.edits[0].focused = true;
        }
        self.formatting = snippet_formatting(r.snippet_id);
        self.request = Some(r);
        self.preview = None;
        self.render();
    }

    /// The live preview with the values typed so far.
    fn render(&mut self) {
        let Some(r) = &self.request else { return };
        if try_core().is_none() {
            return;
        }
        self.preview = Some(commands::render_preview(r.content.clone(), Some(r.snippet_id), self.values.clone(), self.formatting.clone()));
    }

    fn set_value(&mut self, name: String, value: String) {
        self.values.insert(name, value);
        self.render();
    }

    fn submit(&mut self) {
        if let Some(r) = &self.request {
            commands::submit_form(r.token, self.values.clone());
        }
    }

    fn cancel(&mut self) {
        if let Some(r) = &self.request {
            commands::cancel_form(r.token);
        }
    }

    /// The panel measured itself: size the window to it.
    fn measured(&mut self, h: f32) {
        let h = h.ceil().min(MAX_HEIGHT);
        if (h - self.height).abs() > 0.5 {
            self.height = h;
            if try_core().is_some() {
                windows::resize_form(f64::from(h));
            }
        }
    }
}

fn snippet_formatting(id: uuid::Uuid) -> Option<Formatting> {
    try_core()?.store.lock().library.snippets.iter().find(|s| s.id == id).and_then(|s| s.formatting.clone())
}

/// Debug builds with `KUDZU_FORM_DEMO=1` open on what the browser preview's `/form?demo`
/// shows: the first snippet with a `{{field}}`, inserting into Mail, nothing remembered.
/// `KUDZU_FORM_DEMO=<snippet name>` opens that snippet's form instead.
fn demo_payload() -> Option<FormPayload> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let which = std::env::var("KUDZU_FORM_DEMO").ok()?;
    let core = try_core()?;
    let store = core.store.lock();
    let s = if which == "1" {
        store.library.snippets.iter().find(|s| s.content.contains("{{field"))?
    } else {
        store.library.snippets.iter().find(|s| s.name == which)?
    };
    let snippets = &store.library.snippets;
    let resolve = |id: uuid::Uuid| snippets.iter().find(|x| x.id == id).map(|x| x.content.clone());
    Some(FormPayload {
        token: 1,
        snippet_id: s.id,
        snippet_name: s.name.clone(),
        fields: kudzu_core::template::form_fields(&s.content, &resolve),
        initial_values: HashMap::new(),
        content: s.content.clone(),
        target_app: Some(kudzu_engine::AppInfo { bundle_id: "com.apple.mail".into(), name: "Mail".into(), pid: None, path: None }),
    })
}

pub fn apply(s: &mut FormUi, ev: &Event, tx: &mut Tx) -> bool {
    match ev {
        Event::FormRequest(r) => {
            tx.instant();
            s.load(r.clone());
            true
        }
        Event::Library(lib) => {
            // The preview resolves nested snippets and formatting from the library.
            if let Some(r) = &s.request {
                s.formatting = lib.snippets.iter().find(|x| x.id == r.snippet_id).and_then(|x| x.formatting.clone());
                s.render();
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Body text inherits `leading-5` (20 px); `text-[…px]` sizes keep it.
const fn ty(size: f32, w: f32) -> TextStyle {
    TextStyle::new(size, w).lh(20.0)
}

pub fn view(s: &FormUi, cx: &mut Cx<FormUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let Some(r) = &s.request else { return el() };
    let panel = col()
        .key("form-panel")
        .role("dialog")
        .label(format!("Fill in {}", r.snippet_name))
        .w_full()
        .radius(PANEL_RADIUS)
        .system_material(SystemMaterial::Popover)
        .fill(t.popover.alpha(0.55))
        .border(1.0, t.hairline)
        .color(t.popover_foreground)
        .child(header(r, th))
        .child(fields(s, r, th))
        .child(preview(s, th))
        .child(footer(th))
        // Esc cancels from any control; Tab stays inside the form.
        .modal(false, |s: &mut FormUi| s.cancel())
        .on_layout_tx(|s: &mut FormUi, rect, tx| {
            s.measured(rect[3]);
            tx.live_only();
        });
    // The panel sizes to its content; the window follows it.
    col().w_full().items_start().child(panel)
}

fn header(r: &FormPayload, th: &Theme) -> El {
    let t = &th.t;
    let name = if r.snippet_name.is_empty() { "Fill in snippet".to_string() } else { r.snippet_name.clone() };
    let mut e = row()
        .w_full()
        .gap(10.0)
        .center_y()
        .px(16.0)
        .pt(12.0)
        .pb(13.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .window_drag()
        .child(row().size(24.0, 24.0).center().radius(radius::MD).fill(t.iris_soft).child(icon(&icons::TEXT_CURSOR_INPUT, 14.0).color(t.iris_foreground)))
        .child(text(name).font(ty(13.0, weight::SEMIBOLD)).color(t.popover_foreground).truncate().grow(1.0).min_w(0.0).shrink());
    if let Some(app) = &r.target_app {
        e = e.child(text(format!("into {}", app.name)).font(ty(11.0, weight::NORMAL)).color(t.faint).truncate().min_w(0.0).shrink());
    }
    e
}

/// The look every control shares: `rounded-lg bg-foreground/[0.03] px-3 shadow-well`, and on
/// focus `bg-transparent shadow-none ring-2 ring-ring` (`transition-shadow`).
fn control(th: &Theme, e: El, focused: bool) -> El {
    let t = &th.t;
    let e = e.w_full().radius(radius::LG).px(12.0).motion(tw_spring());
    if focused {
        crate::ui::kit::button::shadows(e.fill(Color::TRANSPARENT), &[], t.elev_well.len()).outline(2.0, 0.0, t.ring)
    } else {
        e.fill(Theme::tint(t.foreground, 0.03)).elevate(&t.elev_well).outline(2.0, 0.0, Color::TRANSPARENT)
    }
}

const FIELD_FONT: TextStyle = TextStyle::new(13.5, weight::NORMAL).lh(20.0);
/// `leading-relaxed` in the multi-line field.
const AREA_FONT: TextStyle = TextStyle::new(13.5, weight::NORMAL).lh(21.9375);

fn fields(s: &FormUi, r: &FormPayload, th: &Theme) -> El {
    let t = &th.t;
    let items = r.fields.iter().enumerate().map(|(i, f)| {
        let key = format!("form-field-{i}");
        let name = f.name.clone();
        let label = text(f.name.clone()).font(TextStyle::new(12.0, weight::MEDIUM).lh(16.0)).color(t.muted_foreground);
        let edit = &s.edits[i];
        let input = match f.kind {
            FormFieldKind::Choice => {
                let value = s.values.get(&f.name).cloned().filter(|v| f.options.contains(v));
                let shown = value.clone().or_else(|| f.options.first().cloned()).unwrap_or_default();
                let selected = f.options.iter().position(|o| Some(o) == value.as_ref()).unwrap_or(0).to_string();
                let options = f.options.clone();
                let k = key.clone();
                let e = row()
                    .key(key.clone())
                    .role("combobox")
                    .label(f.name.clone())
                    .a11y_value(shown.clone())
                    .focusable(true)
                    .focus_ring(2.0, 0.0, t.ring)
                    .h(36.0)
                    .center_y()
                    .child(text(shown).font(FIELD_FONT).color(t.popover_foreground).truncate().grow(1.0).min_w(0.0).shrink().mr(20.0))
                    .child(icon(&icons::CHEVRON_DOWN, 16.0).color(t.muted_foreground).pin(None, Some(10.0), Some(10.0), None))
                    .on_press_tx(move |_: &mut FormUi, tx| {
                        let menu = PopupMenu::picker(options.iter().enumerate().map(|(j, o)| (j.to_string(), o.clone())), &selected);
                        let (options, name) = (options.clone(), name.clone());
                        tx.popup_menu(menu, k.clone(), move |s: &mut FormUi, picked, _| {
                            if let Some(o) = picked.and_then(|p| p.parse::<usize>().ok()).and_then(|j| options.get(j)) {
                                s.set_value(name, o.clone());
                            }
                        });
                    });
                control(th, e, false)
            }
            FormFieldKind::Multiline => {
                let field = text_input(edit, AREA_FONT)
                    .key(key.clone())
                    .label(f.name.clone())
                    .w_full()
                    .h(3.0 * 21.9375)
                    .color(t.popover_foreground)
                    .caret(t.popover_foreground, t.iris_soft)
                    .on_edit(move |s: &mut FormUi| &mut s.edits[i])
                    .on_input_change(move |s: &mut FormUi, _| {
                        let (n, v) = (s.request.as_ref().map(|r| r.fields[i].name.clone()).unwrap_or_default(), s.edits[i].text.clone());
                        s.set_value(n, v);
                    })
                    // Return types a newline here; ⌘Return inserts.
                    .on_input_key(move |s: &mut FormUi, ev, _| {
                        if ev.key != Key::Enter {
                            return false;
                        }
                        if ev.mods.cmd || ev.mods.ctrl {
                            s.submit();
                        } else {
                            s.edits[i].insert("\n");
                            let (n, v) = (s.request.as_ref().map(|r| r.fields[i].name.clone()).unwrap_or_default(), s.edits[i].text.clone());
                            s.set_value(n, v);
                        }
                        true
                    });
                control(th, col().py(8.0).child(field), edit.focused)
            }
            FormFieldKind::Text => {
                let field = text_input(edit, FIELD_FONT)
                    .single_line()
                    .key(key.clone())
                    .label(f.name.clone())
                    .grow(1.0)
                    .min_w(0.0)
                    .spellcheck(false)
                    .color(t.popover_foreground)
                    .caret(t.popover_foreground, t.iris_soft)
                    .on_edit(move |s: &mut FormUi| &mut s.edits[i])
                    .on_input_change(move |s: &mut FormUi, _| {
                        let (n, v) = (s.request.as_ref().map(|r| r.fields[i].name.clone()).unwrap_or_default(), s.edits[i].text.clone());
                        s.set_value(n, v);
                    })
                    .on_submit(|s: &mut FormUi, _| s.submit());
                control(th, row().h(36.0).center_y().child(field), edit.focused)
            }
        };
        col().w_full().gap(6.0).child(label).child(input)
    });
    col().w_full().gap(12.0).px(16.0).pt(14.0).pb(14.0).children(items)
}

fn preview(s: &FormUi, th: &Theme) -> El {
    let t = &th.t;
    let font = TextStyle::new(12.5, weight::NORMAL).lh(20.3125);
    let ink = Theme::tint(t.foreground, 0.85);
    let body = match &s.preview {
        Some(r) => FormattedText::new(r).font(font).color(ink).build(th).selectable(true).selection_color(t.iris_soft),
        None => text("…").font(font).color(ink),
    };
    // `mx-4 mb-3.5 max-h-40 overflow-y-auto rounded-lg … px-3 py-2.5 shadow-well`.
    let content = col().w_full().px(12.0).py(10.0).child(text("Preview").font(ty(10.5, weight::MEDIUM)).color(t.faint).mb(4.0)).child(body);
    row()
        .px(16.0)
        .pb(14.0)
        .w_full()
        .child(col().key("form-preview").w_full().max_h(160.0).radius(radius::LG).fill(Theme::tint(t.foreground, 0.03)).elevate(&t.elev_well).scroll(ScrollSpec { stick_bottom: false }).child(content))
}

/// `brightness(1.1)`, as CSS filters do it: on sRGB-encoded channels.
fn brighten(c: Color, k: f32) -> Color {
    let (enc, dec) = (rune::theme::srgb_encode, rune::theme::srgb_decode);
    let f = |x: f32| dec((enc(x) * k).min(1.0));
    Color { r: f(c.r), g: f(c.g), b: f(c.b), a: c.a }
}

fn footer(th: &Theme) -> El {
    let t = &th.t;
    let small = ty(11.5, weight::NORMAL);
    let hint = |combo: &str, what: &str| row().gap(6.0).center_y().child(Shortcut::new(combo).build(th)).child(text(what.to_string()).font(small).color(t.muted_foreground));
    // `h-7 rounded-lg bg-iris px-3 text-xs font-medium text-iris-contrast shadow-accent
    // sheen-strong press hover:brightness-110`.
    let insert = row()
        .key("form-insert")
        .role("button")
        .label("Insert")
        .focusable(true)
        .focus_ring(2.0, 1.0, t.ring)
        .default_button()
        .h(28.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(radius::LG)
        .fill(t.iris)
        .elevate(&t.elev_accent)
        .sheen(true)
        .hover_group()
        .hover_motion(crate::ui::kit::button::tween(crate::ui::theme::MOTION_FAST, crate::ui::kit::button::EASE), crate::ui::kit::button::tween(crate::ui::theme::MOTION_FAST, crate::ui::kit::button::EASE))
        .child(layer(0.0).radius(radius::LG).fill(brighten(t.iris, 1.1)).sheen(true).group_opacity(0.0, 1.0))
        .child(text("Insert").font(TextStyle::new(12.0, weight::MEDIUM).lh(16.0)).color(t.iris_contrast))
        .child(Shortcut::new("enter").tone(Tone::OnAccent).color(t.iris_contrast).build(th))
        .press_sink(0.97)
        .on_press(|s: &mut FormUi| s.submit());
    row()
        .w_full()
        .gap(16.0)
        .center_y()
        .px(16.0)
        .pt(11.0)
        .pb(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .child(hint("tab", "Next field"))
        .child(hint("escape", "Cancel"))
        .child(row().grow(1.0).justify_end().child(insert))
}
