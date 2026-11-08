//! The snippet editor (`components/app/editor/`): `snippet-editor.tsx` here, with one module
//! per component file beside it.
//!
//! [`EditorState`] is the component's React state: a local draft of the selected snippet that
//! edits change at once and that saves 350 ms after the last edit (and when the selection
//! moves away, or ⌘S), the save indicator, the preview, and each part's own state. `MainUi`
//! owns the selection, so the editor adopts a newly selected snippet the frame after it shows
//! ([`ensure`]); until then the view draws a fresh state for it, so nothing lags behind.

pub mod behavior;
pub mod formatting_toolbar;
pub mod image_tools;
pub mod tag_tools;
pub mod template;
pub mod trigger_input;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use kudzu_core::client::groups::{build_tree, flatten, group_path, is_group_enabled};
use kudzu_core::client::template::{analyze, build_tag};
use kudzu_core::formatting::Formatting;
use kudzu_core::matching::ConflictKind;
use kudzu_core::template::{AttributeValue, RenderResult};
use kudzu_core::SnippetRecord;
use rune::{col, el, icon, row, text, text_input, Beat, Color, Cx, El, Icon, Key, KeyEvent, ScrollSpec, Sequence, Spring, SpringPair, TextEdit, TextStyle, Transition, Tx};
use uuid::Uuid;

use crate::backend::commands;
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::dropdown_menu::{DropdownMenu, MenuItem, MenuState, SubMenu};
use crate::ui::kit::formatted_text::FormattedText;
use crate::ui::kit::shortcut::key_labels;
use crate::ui::kit::switch::{Switch, SwitchSize};
use crate::ui::kit::toast::Toast;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::main::MainUi;
use crate::ui::theme::{motion, radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

use self::template::TemplateState;

/// `SAVE_DELAY`.
const SAVE_DELAY: Duration = Duration::from_millis(350);
/// `useDebounced(draft, 120)` for the preview.
const PREVIEW_DELAY: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Saved {
    #[default]
    Idle,
    Saving,
    Saved,
}

#[derive(Default)]
pub struct EditorState {
    /// The snippet this state edits.
    pub id: Option<Uuid>,
    pub draft: Option<SnippetRecord>,
    pub dirty: bool,
    save_gen: u64,
    pub saved: Saved,
    pub name: TextEdit,
    pub triggers: trigger_input::TriggerState,
    pub template: TemplateState,
    pub preview: Option<RenderResult>,
    /// The content and formatting the preview shows.
    preview_of: Option<(String, Option<Formatting>)>,
    preview_gen: u64,
    pub group_menu: MenuState,
    pub more_menu: MenuState,
    pub tags: tag_tools::TagToolsState,
    pub toolbar: formatting_toolbar::ToolbarState,
    pub image: image_tools::ImageState,
    pub behavior: behavior::BehaviorState,
    /// The favourite star bursts only when turned on (`FavoriteStar`), counted to replay.
    star_burst: u32,
}

impl EditorState {
    fn fresh(s: &MainUi, snippet: &SnippetRecord) -> EditorState {
        let mut st = EditorState {
            id: Some(snippet.id),
            draft: Some(snippet.clone()),
            name: TextEdit::new(snippet.name.clone()),
            template: TemplateState::new(&snippet.content, kudzu_core::client::formatting::source_spans(&snippet.content, snippet.formatting.as_ref())),
            ..Default::default()
        };
        st.preview = Some(render(s, snippet));
        st.preview_of = Some((snippet.content.clone(), snippet.formatting.clone()));
        st
    }
}

fn render(_s: &MainUi, d: &SnippetRecord) -> RenderResult {
    commands::render_preview(d.content.clone(), Some(d.id), HashMap::new(), d.formatting.clone())
}

// MARK: Draft lifecycle

/// Adopts the selected snippet: a new selection (saving the old draft first), or an outside
/// change to this one while it has no pending edits.
pub fn ensure(s: &mut MainUi) {
    let Some(id) = s.selected.filter(|id| s.snippet(*id).is_some()) else {
        flush(s);
        s.editor.id = None;
        return;
    };
    if s.editor.id != Some(id) {
        flush(s);
        let Some(snippet) = s.snippet(id).cloned() else { return };
        s.editor = EditorState::fresh(s, &snippet);
    } else if !s.editor.dirty {
        let Some(store) = s.snippet(id).cloned() else { return };
        let stale = s.editor.draft.as_ref().is_none_or(|d| d.updated_at != store.updated_at);
        if stale {
            adopt(s, store);
        }
    }
    if s.focus_name_of == Some(id) {
        s.editor.name.focused = true;
        s.focus_name_of = None;
    }
}

/// An outside update (another window, an import, a restore, our own save's timestamp).
fn adopt(s: &mut MainUi, store: SnippetRecord) {
    let e = &mut s.editor;
    if e.name.text != store.name {
        e.name.replace_text(store.name.clone());
    }
    let spans = kudzu_core::client::formatting::source_spans(&store.content, store.formatting.as_ref());
    let doc = &e.template.doc;
    if doc.text() != store.content || doc.spans().map(<[_]>::to_vec) != spans {
        e.template.doc.set_external(&store.content, spans);
        e.template.saved_revision = e.template.doc.revision;
    }
    e.draft = Some(store);
    refresh_preview_now(s);
}

/// Whether the view should draw `s.editor` or a fresh state for the selection.
fn current(s: &MainUi, snippet: &SnippetRecord) -> bool {
    s.editor.id == Some(snippet.id) && (s.editor.dirty || s.editor.draft.as_ref().is_some_and(|d| d.updated_at == snippet.updated_at))
}

/// `update(patch)`: changes the draft and saves it after a pause.
pub fn update(s: &mut MainUi, patch: impl FnOnce(&mut SnippetRecord)) {
    ensure(s);
    let Some(d) = s.editor.draft.as_mut() else { return };
    patch(d);
    s.editor.dirty = true;
    s.editor.saved = Saved::Saving;
    s.editor.save_gen += 1;
    let generation = s.editor.save_gen;
    s.spawn(move || std::thread::sleep(SAVE_DELAY), move |s, (), _| {
        if s.editor.save_gen == generation {
            flush(s);
        }
    });
    schedule_preview(s);
}

/// Saves pending edits now (`flush`).
pub fn flush(s: &mut MainUi) {
    if !s.editor.dirty {
        return;
    }
    s.editor.dirty = false;
    s.editor.save_gen += 1;
    let Some(d) = s.editor.draft.clone() else { return };
    s.upsert_local(d.clone());
    match commands::save_snippet(d) {
        Ok(saved) => {
            s.upsert_local(saved.clone());
            if s.editor.id == Some(saved.id) {
                s.editor.saved = Saved::Saved;
                if let Some(d) = s.editor.draft.as_mut() {
                    d.updated_at = saved.updated_at;
                }
            }
        }
        Err(e) => s.toasts.push(Toast::error("Couldn't save").description(e)),
    }
    s.refresh_conflicts();
}

fn schedule_preview(s: &mut MainUi) {
    s.editor.preview_gen += 1;
    let generation = s.editor.preview_gen;
    s.spawn(move || std::thread::sleep(PREVIEW_DELAY), move |s, (), _| {
        if s.editor.preview_gen == generation {
            refresh_preview_now(s);
        }
    });
}

fn refresh_preview_now(s: &mut MainUi) {
    let Some(d) = s.editor.draft.clone() else { return };
    let key = (d.content.clone(), d.formatting.clone());
    if s.editor.preview_of.as_ref() == Some(&key) && s.editor.preview.is_some() {
        return;
    }
    s.editor.preview = Some(render(s, &d));
    s.editor.preview_of = Some(key);
}

/// The template editor changed: copy its document into the draft.
fn template_changed(s: &mut MainUi) {
    let doc = &s.editor.template.doc;
    let content = doc.text().to_string();
    let formatting = doc.spans().map(|spans| Formatting { source: content.clone(), spans: spans.to_vec() });
    s.editor.template.saved_revision = doc.revision;
    update(s, move |d| {
        d.content = content;
        d.formatting = formatting;
    });
}

/// What the template editor's edits ask for.
fn on_template(s: &mut MainUi, fx: template::Effects, tx: &mut Tx) {
    if fx.changed || s.editor.template.doc.revision != s.editor.template.saved_revision {
        template_changed(s);
        // Keep the caret in view as the editor grows.
        tx.scroll_into_view(format!("kz-caret-{}", s.editor.template.caret_gen));
    }
    if let Some(at) = fx.slash {
        tag_tools::open_insert(s, Some(at));
    }
    if let Some(from) = fx.chip {
        tag_tools::open_chip(s, from);
    }
    if fx.escape {
        escape_to_list(s, tx);
    }
}

/// Escape from the editor returns to the list (main-app.tsx's keyboard round trip), unless a
/// popover or menu is open (it takes Escape itself).
pub fn escape_to_list(s: &mut MainUi, tx: &mut Tx) -> bool {
    let e = &s.editor;
    let open = e.tags.insert.is_some() || e.tags.chip.is_some() || e.toolbar.link_open || e.image.open || e.group_menu.is_open() || e.more_menu.is_open() || e.behavior.apps.open;
    if open {
        return false;
    }
    s.editor.template.proxy.focused = false;
    s.editor.name.focused = false;
    s.editor.triggers.input.focused = false;
    s.list.focus_list(tx);
    true
}

fn template_access(s: &mut MainUi) -> &mut TemplateState {
    ensure(s);
    &mut s.editor.template
}

/// ⌘E: focus the template editor.
pub fn focus(s: &mut MainUi, _tx: &mut rune::Tx) {
    ensure(s);
    s.editor.template.focus();
}

/// `pick`: inserts a menu choice (replacing the typed `/`), then configures it if asked.
pub fn pick(s: &mut MainUi, text: &str, configure: bool, slash: Option<usize>) {
    ensure(s);
    let t = &mut s.editor.template;
    let range = match slash {
        Some(at) if t.doc.slice(at, at + 1) == "/" => t.replace(at, at + 1, text),
        _ => t.insert(text),
    };
    t.focus();
    template_changed(s);
    if configure {
        tag_tools::open_chip(s, range.0);
    }
}

// MARK: View

/// The snippet's draft as the view draws it.
fn draft(st: &EditorState) -> &SnippetRecord {
    st.draft.as_ref().expect("draft")
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let Some(snippet) = s.selected_snippet() else { return empty(s, cx) };
    let fresh;
    let st: &EditorState = if current(s, snippet) {
        &s.editor
    } else {
        fresh = EditorState::fresh(s, snippet);
        &fresh
    };
    let th = theme_for(cx);
    let t = &th.t;
    let d = draft(st);
    let mut layer: Vec<El> = Vec::new();

    // Adopt the selection on the first layout (the key changes with the snippet).
    let sync = el().key(format!("editor-sync-{}-{}", d.id, s.selected_snippet().map_or(0, |x| x.updated_at.timestamp_millis())))
        .absolute(0.0, 0.0)
        .size(0.0, 0.0)
        .on_layout_tx(|s: &mut MainUi, _, _| ensure(s));

    let header = header(s, st, cx, &mut layer);
    let body = body(s, st, cx, &mut layer);
    col()
        .key("snippet-editor")
        .h_full()
        .w_full()
        .min_h(0.0)
        .on_key(|s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx| save_key(s, ev) || (matches!(ev.key, Key::Escape) && escape_to_list(s, tx)))
        .child(sync)
        .child(header)
        .child(body)
        .child(el().key("editor-layer").absolute(0.0, 0.0).size(0.0, 0.0).children(layer))
        .fill(Color::TRANSPARENT)
        .color(t.foreground)
}

/// ⌘S saves now (`onKeyDownCapture`).
fn save_key(s: &mut MainUi, ev: &KeyEvent) -> bool {
    if ev.mods.cmd && matches!(ev.key, Key::Command('s')) {
        flush(s);
        return true;
    }
    false
}

/// A filled star (`fill-current`).
static STAR_FILLED: Icon = Icon {
    name: "kudzu-star-filled",
    view: 24.0,
    body: r#"<g fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></g>"#,
};

/// `animate-star` (`star-burst`, 420 ms ease-out): 0.5 → 1.25 → 1.
const STAR: &[Beat] = &[Beat::new(0.0).scale(0.5), Beat::new(0.231).scale(1.25).easing(rune::Easing::EASE_OUT), Beat::new(0.42).easing(rune::Easing::EASE_OUT)];
/// `animate-check` (`check-in`, `--motion-slow` on `--spring-pop`).
use crate::ui::kit::checkbox::CHECK_IN;

fn header(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let d = draft(st);
    let groups = &s.library.groups;
    let tree = build_tree(groups);
    let flat = flatten(&tree, None);
    let path = group_path(groups, d.group_id);

    // The group breadcrumb, a menu to move the snippet.
    let mut crumb = row()
        .key("editor-group")
        .min_w(0.0)
        .shrink()
        .center_y()
        .gap(4.0)
        .px(6.0)
        .py(4.0)
        .radius(radius::MD)
        .role("button")
        .label("Move to group")
        .hover(t.accent)
        .child(icon(&icons::FOLDER, 14.0).color(t.muted_foreground).group_color(t.foreground).a11y_hidden());
    if path.is_empty() {
        crumb = crumb.child(text("No group").key("no-group").font(ty::XS).color(t.muted_foreground).group_color(t.foreground));
    } else {
        for (i, p) in path.iter().enumerate() {
            let mut part = row().key(format!("path-{i}")).min_w(0.0).shrink().center_y().gap(4.0);
            if i > 0 {
                part = part.child(icon(&icons::CHEVRON_RIGHT, 12.0).key("separator").color(t.muted_foreground).group_color(t.foreground).opacity(0.5).a11y_hidden());
            }
            part = part.child(text(p.clone()).key("label").font(ty::XS).color(t.muted_foreground).group_color(t.foreground).truncate().min_w(0.0).shrink());
            crumb = crumb.child(part);
        }
    }
    let crumb = crate::ui::kit::button::hover_fast(crumb);
    let mut move_menu = DropdownMenu::new("editor-group-menu", &st.group_menu, |s: &mut MainUi| &mut s.editor.group_menu)
        .trigger(crumb)
        .align(rune::Align::Start)
        .width(224.0)
        .label("Move to")
        .item(MenuItem::checkbox("No group", d.group_id.is_none()).on_select(|s: &mut MainUi, _| update(s, |d| d.group_id = None)));
    for n in &flat {
        let id = n.group.id;
        move_menu = move_menu.item(
            MenuItem::checkbox(n.group.name.clone(), d.group_id == Some(id))
                .icon(&icons::FOLDER)
                .pad_left(8.0 + n.depth as f32 * 14.0)
                .on_select(move |s: &mut MainUi, _| update(s, |d| d.group_id = Some(id))),
        );
    }
    let move_menu = move_menu.build(cx, layer);

    // Saved indicator: `text-[11px] text-faint`, fading in and out.
    let saved = {
        let mut r = row().key("editor-saved").mr(8.0).center_y().gap(4.0).opacity(if st.saved == Saved::Idle { 0.0 } else { 1.0 }).motion(motion::moderate()).live_region(rune::LiveRegion::Polite);
        if st.saved == Saved::Saved {
            r = r.child(el().key(format!("editor-saved-check-{}", st.save_gen)).child(icon(&icons::CHECK, 12.0).color(t.success)).sequence(Sequence::new(CHECK_IN)).a11y_hidden());
        }
        r.child(text(if st.saved == Saved::Saving { "Saving…" } else { "Saved" }).key("label").font(ty::XS2).color(t.faint))
    };

    // Favourite.
    let fav_on = d.is_favorite;
    let star = el()
        .key(format!("editor-star-{}-{}", d.id, st.star_burst))
        .ml(-4.0)
        .child(icon(if fav_on { &STAR_FILLED } else { &icons::STAR }, 16.0).color(if fav_on { t.favorite } else { t.foreground }))
        .a11y_hidden();
    let star = if fav_on && st.star_burst > 0 { star.sequence(Sequence::new(STAR)) } else { star };
    let fav_label = if fav_on { "Remove from favorites" } else { "Add to favorites" };
    let mut fav = Button::new("")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .child(star)
        .id("editor-fav")
        .on_press(|s: &mut MainUi| {
            ensure(s);
            let on = !draft(&s.editor).is_favorite;
            if on {
                s.editor.star_burst += 1;
            }
            update(s, |d| d.is_favorite = on);
        });
    if fav_on {
        fav = fav.color(t.favorite).hover_color(t.favorite);
    }
    let fav = Tooltip::new("editor-fav-tip").text("Favorite").shortcut("mod+shift+f").build(cx, fav.build(th).label(fav_label).checked(fav_on), layer);

    // Enabled.
    let enabled = d.is_enabled;
    let toggle = |s: &mut MainUi| {
        ensure(s);
        let on = !draft(&s.editor).is_enabled;
        update(s, |d| d.is_enabled = on);
    };
    let enabled_row = row()
        .key("editor-enabled")
        .h(28.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(radius::MD)
        .on_press(toggle)
        .child(Switch::new(enabled).size(SwitchSize::Sm).label("Enabled").id("editor-enabled-switch").on_toggle(toggle).build(th))
        .child(el().w(48.0).child(text(if enabled { "Enabled" } else { "Off" }).font(ty::XS).color(t.muted_foreground)));
    let enabled_row = Tooltip::new("editor-enabled-tip").text("Turn this snippet on or off").shortcut("mod+shift+e").build(cx, enabled_row, layer);

    // More actions.
    let more_btn = Button::icon_only(&icons::ELLIPSIS, "More actions").variant(Variant::Ghost).size(Size::IconSm).id("editor-more").build(th);
    let id = d.id;
    let mut sub = SubMenu::new("Move to group").icon(&icons::FOLDER_INPUT).width(208.0).item(MenuItem::new("No group").on_select(move |s: &mut MainUi, _| {
        flush(s);
        s.move_snippets(&[id], None);
    }));
    for n in &flat {
        let gid = n.group.id;
        sub = sub.item(MenuItem::new(n.group.name.clone()).pad_left(8.0 + n.depth as f32 * 14.0).on_select(move |s: &mut MainUi, _| {
            flush(s);
            s.move_snippets(&[id], Some(gid));
        }));
    }
    let more = DropdownMenu::new("editor-more-menu", &st.more_menu, |s: &mut MainUi| &mut s.editor.more_menu)
        .trigger(more_btn)
        .align(rune::Align::End)
        .width(240.0)
        .item(MenuItem::new("Copy expanded snippet").icon(&icons::CLIPBOARD).on_select(|s: &mut MainUi, _| copy_text(s)))
        .item(MenuItem::new("Duplicate").icon(&icons::COPY).shortcut(key_labels("mod+d").join("")).on_select(move |s: &mut MainUi, _| {
            flush(s);
            s.duplicate(id);
        }))
        .sub(sub)
        .separator()
        .item(MenuItem::new("Delete").icon(&icons::TRASH).destructive().shortcut(key_labels("mod+backspace").join("")).on_select(move |s: &mut MainUi, tx| {
            flush(s);
            s.request_delete(vec![id], tx);
        }))
        .build(cx, layer);

    row()
        .key("editor-header")
        .h(48.0)
        .center_y()
        .gap(8.0)
        .px(16.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .window_drag()
        .child(move_menu.no_window_drag())
        .child(rune::spacer())
        .child(row().center_y().gap(4.0).no_window_drag().child(saved).child(fav).child(enabled_row).child(more))
}

fn copy_text(s: &mut MainUi) {
    ensure(s);
    let d = draft(&s.editor).clone();
    commands::copy_preview(d.content, Some(d.id), d.formatting);
    s.toasts.push(Toast::success("Copied expanded snippet"));
}

/// `animate-rise`.
fn rise() -> Transition {
    Transition::offset(0.0, 4.0).combined(Transition::opacity()).springs(SpringPair::new(motion::soft(), Spring::NONE))
}

fn label_xs(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::XS.weight_of(weight::MEDIUM)).color(th.t.muted_foreground)
}

fn body(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let d = draft(st);
    let groups = &s.library.groups;
    let path = group_path(groups, d.group_id);
    let group_disabled = !is_group_enabled(groups, d.group_id);
    let mut page = col().key(format!("editor-page-{}", d.id)).w_full().max_w(760.0).gap(24.0).px(32.0).pt(28.0).pb(64.0).transition(rise());

    // Turned off.
    if !d.is_enabled || group_disabled {
        let msg = if d.is_enabled {
            format!("This snippet's group “{}” is turned off, so it won't expand.", path.join(" › "))
        } else {
            "This snippet is turned off. It still shows in search but won't expand.".to_string()
        };
        page = page.child(
            row()
                .key("editor-off")
                .gap(8.0)
                .center_y()
                .radius(radius::LG)
                .fill(Theme::tint(t.foreground, 0.04))
                .px(12.0)
                .py(8.0)
                .child(el().size(6.0, 6.0).radius(radius::FULL).fill(t.faint))
                .child(text(msg).font(ty::XS).color(t.muted_foreground).wrap().shrink()),
        );
    }

    // Name and triggers.
    let name_font = TextStyle::new(22.0, weight::SEMIBOLD).lh(27.5).tracking(-0.33);
    let name = text_input(&st.name, name_font)
        .single_line()
        .key("editor-name")
        .w_full()
        .label("Snippet name")
        .placeholder("Untitled snippet", Theme::tint(t.faint, 0.7))
        .color(t.foreground)
        .caret(t.foreground, t.iris_soft)
        .on_edit(|s: &mut MainUi| {
            ensure(s);
            &mut s.editor.name
        })
        .on_input_change(|s: &mut MainUi, _| {
            let n = s.editor.name.text.clone();
            update(s, move |d| d.name = n);
        })
        .on_input_key(|s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx| {
            if save_key(s, ev) {
                return true;
            }
            if matches!(ev.key, Key::Escape) {
                return escape_to_list(s, tx);
            }
            if matches!(ev.key, Key::Enter) && !ev.mods.cmd {
                s.editor.name.focused = false;
                s.editor.triggers.input.focused = true;
                return true;
            }
            false
        });

    let mine: Vec<_> = s.conflicts.iter().filter(|c| c.snippet_id == d.id || c.other_snippet_id == d.id).collect();
    let conflicted: HashSet<String> = mine.iter().map(|c| if c.snippet_id == d.id { c.trigger.to_lowercase() } else { c.other_trigger.to_lowercase() }).collect();
    let mut triggers = col().gap(6.0).child(
        row()
            .gap(4.0)
            .child(label_xs(th, "Triggers"))
            .child(text("— type any of these to expand").font(ty::XS).color(t.faint)),
    );
    triggers = triggers.child(trigger_input::view(s, st, &conflicted, cx));
    if d.triggers.is_empty() && !d.content.is_empty() {
        triggers = triggers.child(
            text("Add a trigger so this snippet can expand as you type. You can still insert it from search.").key("trigger-hint").font(ty::XS).color(t.muted_foreground).wrap(),
        );
    }
    for (i, c) in mine.iter().enumerate() {
        let is_mine = c.snippet_id == d.id;
        let other_id = if is_mine { c.other_snippet_id } else { c.snippet_id };
        let other = s.snippet(other_id).map(|x| x.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "another snippet".into());
        let message = match c.kind {
            ConflictKind::Duplicate => format!("“{}” is also used by", if is_mine { &c.trigger } else { &c.other_trigger }),
            ConflictKind::Shadowing if is_mine => format!("“{}” fires before “{}” can be finished in", c.trigger, c.other_trigger),
            ConflictKind::Shadowing => format!("“{}” can't be typed in full — “{}” fires first in", c.other_trigger, c.trigger),
        };
        let n = other.len();
        let link = rune::rich(other, vec![rune::Span::new(0, n).underline(t.warning)])
            .font(ty::XS.weight_of(weight::MEDIUM))
            .color(t.warning)
            .role("button")
            .key(format!("editor-conflict-link-{i}"))
            .on_press(move |s: &mut MainUi| {
                flush(s);
                s.selected = Some(other_id);
            });
        triggers = triggers.child(
            row()
                .key(format!("editor-conflict-{i}"))
                .flex_wrap()
                .center_y()
                .gap(4.0)
                .child(icon(&icons::TRIANGLE_ALERT, 14.0).color(t.warning).a11y_hidden())
                .child(text(message).font(ty::XS).color(t.warning))
                .child(link),
        );
    }
    page = page.child(col().key("editor-fields").gap(16.0).child(name).child(triggers));

    // Content.
    page = page.child(content_section(s, st, cx, layer));

    // Preview.
    let facts = analyze(&d.content);
    let mut prev_head = row().gap(8.0).center_y().child(label_xs(th, "Preview"));
    let badge = |label: String, c: Color| {
        el().radius(radius::SM * 0.6).fill(Theme::tint(c, 0.12)).px(6.0).py(1.0).child(text(label).font(TextStyle::new(10.5, weight::MEDIUM).lh(14.0)).color(c))
    };
    if facts.is_form {
        prev_head = prev_head.child(badge(format!("Asks for {}", facts.field_names.join(", ")), t.warning).key("form"));
    }
    if facts.uses_clipboard {
        prev_head = prev_head.child(badge("Uses clipboard".into(), t.success).key("clipboard"));
    }
    let preview_font = TextStyle::new(13.5, weight::NORMAL).lh(21.9375);
    let preview_body = match &st.preview {
        Some(r) if !r.text.is_empty() => FormattedText::new(r).font(preview_font).build(th).selectable(true),
        _ => text("Nothing to show yet.").font(preview_font).color(t.faint),
    };
    page = page.child(
        col().key("editor-preview").gap(6.0).label("Preview").child(prev_head).child(
            el().min_h(48.0)
                .radius(radius::XL)
                .fill(Theme::tint(t.foreground, 0.025))
                .elevate(&t.elev_well)
                .px(16.0)
                .py(12.0)
                .child(preview_body),
        ),
    );

    // Behaviour.
    page = page.child(behavior::view(s, st, cx, layer));

    // Footer.
    let usage = s.usage.get(&d.id).cloned().unwrap_or_default();
    let now = chrono::Utc::now().timestamp_millis();
    let tz = chrono::Local;
    let mut foot = row().key("editor-footer").flex_wrap().gaps(4.0, 16.0);
    let ft = |s: String| text(s).font(ty::XS2.lh(20.0).features("'tnum' 1")).color(t.faint);
    foot = foot.child(ft(format!("Used {} {}", usage.count, if usage.count == 1 { "time" } else { "times" })).key("used"));
    if usage.last != 0 {
        foot = foot.child(ft(format!("Last used {}", kudzu_core::client::stats::relative_time(usage.last, now, &tz).to_lowercase())).key("last-used"));
    }
    foot = foot.child(ft(format!("Edited {}", kudzu_core::client::stats::relative_time(d.updated_at.timestamp_millis(), now, &tz).to_lowercase())).key("edited"));
    foot = foot.child(ft(format!("Created {}", kudzu_core::client::js::date_medium(&d.created_at.with_timezone(&tz)))).key("created"));
    page = page.child(foot);

    col().key("editor-scroll").grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(col().w_full().items_center().child(page))
}

/// "Expands to": the quick-insert buttons, the editor card and the tag popovers.
fn content_section(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let quick: [(&'static Icon, &str, String, bool); 4] = [
        (&icons::CALENDAR_DAYS, "Date", build_tag("date", [("format", Some(AttributeValue::String("MMMM d, yyyy".into())))]), false),
        (&icons::CLIPBOARD, "Clipboard", build_tag::<&str>("clipboard", []), false),
        (&icons::TEXT_CURSOR, "Cursor", build_tag::<&str>("cursor", []), false),
        (&icons::TEXT_CURSOR_INPUT, "Fill-in", build_tag("field", [("name", Some(AttributeValue::String("Name".into())))]), true),
    ];
    let mut tools = row().center_y().gap(2.0);
    for (ic, label, tag, configure) in quick {
        let b = Button::icon_only(ic, format!("Insert {label}"))
            .variant(Variant::Ghost)
            .size(Size::IconXs)
            .color(t.muted_foreground)
            .id(format!("editor-insert-{label}"))
            .on_press(move |s: &mut MainUi| pick(s, &tag, configure, None))
            .build(th);
        tools = tools.child(Tooltip::new(format!("editor-insert-{label}-tip")).text(format!("Insert {}", label.to_lowercase())).build(cx, b, layer));
    }
    let insert = Button::new("Insert")
        .variant(Variant::Ghost)
        .size(Size::Xs)
        .color(t.muted_foreground)
        .icon(&icons::PLUS)
        .shortcut("/")
        .id("editor-insert")
        .on_press(|s: &mut MainUi| tag_tools::open_insert(s, None))
        .build(th);
    tools = tools.child(tag_tools::insert_button(s, st, cx, layer, insert));
    let head = row().center_y().gap(8.0).child(label_xs(th, "Expands to")).child(rune::spacer()).child(tools);

    // The card: `rounded-xl bg-surface shadow-card`, ringed while the editor has focus.
    let focused = st.template.focused();
    let width = cx.layout_size("kz-template").map_or_else(|| fallback_width(cx), |sz| sz[0]);
    let names = |id: &str| -> Option<String> { id.parse::<Uuid>().ok().and_then(|id| s.snippet(id)).map(|x| x.name.clone()).filter(|n| !n.is_empty()) };
    let parts = template::view::<MainUi>(
        th,
        &st.template,
        width,
        168.0,
        "Type the text to expand. Press / to add a date, clipboard, fill-in field…",
        "Snippet content",
        &names,
        template_access,
        on_template,
    );
    // Popover anchors in the editor (the typed `/`, a clicked chip).
    let anchors = tag_tools::anchors(s, st, cx, layer);
    let formatted = st.template.doc.formatted();
    let hint = if formatted { "Select text to apply styles or add a link. Use Image to add a logo or picture." } else { "Choose Formatted to add styles, links, and images." };
    let ring = |c: Color, spread: f32| rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread, color: c, inset: false };
    let shadows: Vec<rune::Shadow> = if focused { vec![ring(t.ring, 1.0), ring(t.iris_soft, 4.0)] } else { t.elev_card.clone() };
    let card = crate::ui::kit::button::shadows(
        col()
            .key("editor-card")
            .w_full()
            .radius(radius::XL)
            .fill(t.surface)
            .child(formatting_toolbar::view(s, st, cx, layer))
            .child(el().key("editor-host").w_full().child(parts.el.child(anchors)))
            .child(
                el().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.hairline).border_layout().px(12.0).py(8.0).child(text(hint).font(ty::XS).color(t.muted_foreground).wrap()),
            ),
        &shadows,
        shadows.len().max(t.elev_card.len()).max(2),
    )
    .motion(Spring::new(0.15, 0.0));

    col().key("editor-content").label("Content").gap(6.0).child(head).child(card)
}

/// The editor's width before its first layout: the pane, less the page's padding.
fn fallback_width(cx: &Cx<MainUi>) -> f32 {
    let pane = cx.layout_size("editor-pane").map_or(1180.0 - 224.0 - 8.0 - 300.0, |s| s[0]);
    (pane - 64.0).clamp(200.0, 760.0 - 64.0)
}

/// `EditorEmpty`: nothing selected.
pub fn empty(_s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let new = Button::new("New snippet")
        .size(Size::Sm)
        .icon(&icons::PLUS)
        .shortcut("mod+n")
        .id("editor-empty-new")
        .on_press(|s: &mut MainUi| {
            s.new_snippet(Default::default());
        })
        .build(th);
    col()
        .key("editor-empty")
        .w_full()
        .h_full()
        .center()
        .gap(16.0)
        .padding(32.0)
        .window_drag()
        .child(
            el().size(48.0, 48.0)
                .center()
                .radius(radius::XXL)
                .fill(Theme::tint(t.foreground, 0.04))
                .box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: t.hairline, inset: true })
                .child(icon(&icons::CORNER_DOWN_LEFT, 20.0).color(t.muted_foreground)),
        )
        .child(
            col().gap(4.0).items_center()
                .child(text("No snippet selected").font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground))
                .child(text("Pick one from the list, or start a new one.").font(ty::XS).color(t.muted_foreground)),
        )
        .child(new.no_window_drag())
}

#[cfg(test)]
mod tests {
    use super::template::doc::Doc;
    use kudzu_core::client::formatting::source_spans;
    use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};

    /// Source and formatting go into the editor model and come back out unchanged.
    #[test]
    fn source_and_formatting_round_trip_through_the_editor() {
        let cases = [
            ("Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}", None),
            ("Hi {{field name=\"Name\"}},\n\nThanks — {{cursor}}\n", None),
            (r"\{{literal}} \\ {{unknown thing}} {{snippet id=0A1B2C3D-0000-4000-8000-00000000000F}}", None),
            ("Bold link [Logo]", Some(vec![
                FormatSpan { start: 0, end: 4, style: FormatStyle::Bold },
                FormatSpan { start: 5, end: 9, style: FormatStyle::Link { url: "https://kudzu.site".into() } },
            ])),
        ];
        for (src, spans) in cases {
            let formatting = spans.map(|spans| Formatting { source: src.to_string(), spans });
            let doc = Doc::new(src, source_spans(src, formatting.as_ref()));
            assert_eq!(doc.text(), src);
            let back = doc.spans().map(|s| Formatting { source: doc.text().to_string(), spans: s.to_vec() });
            assert_eq!(back, formatting);
        }
        // Stale formatting (its source no longer matches) is dropped, as the web did.
        let stale = Formatting { source: "old".into(), spans: vec![] };
        assert!(source_spans("new", Some(&stale)).is_none());
    }
}
