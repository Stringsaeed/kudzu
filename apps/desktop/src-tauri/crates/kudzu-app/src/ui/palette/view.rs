//! The palette's view: `palette.tsx`'s JSX, class for class, as the Tauri window drew it over
//! native vibrancy (`.vibrant`: the panel fills the 760×480 window, whose 18 pt rounded corners
//! clip it, with `bg-popover/55` over the popover material and no entrance animation).

use kudzu_core::client::search::PALETTE_SCOPES;
use kudzu_core::client::stats::{format_count, relative_time};
use kudzu_core::client::template::analyze;
use kudzu_core::SnippetRecord;
use rune::{col, el, icon, rich, row, text, text_input, Color, Cursor, Cx, El, Icon, ScrollSpec, Span, Spring, SystemMaterial, TextStyle, Tx};

use crate::ui::icons;
use crate::ui::kit::button::{tween, tw_spring, TW_EASE};
use crate::ui::kit::formatted_text::FormattedText;
use crate::ui::kit::selection_pill::SelectionPill;
use crate::ui::kit::shortcut::{Shortcut, Tone};
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::theme::{radius, weight, Theme, Utilities, RADIUS};
use crate::ui::theme_for;

use super::{highlight_ranges, row_key, Hit, Mode, PaletteUi};

/// `rounded-[18px]`, the window's own corner.
const PANEL_RADIUS: f32 = crate::backend::windows::PALETTE_RADIUS;

/// lucide `star` with `fill-favorite`: the outline filled in its own colour.
static STAR_FILLED: Icon = Icon {
    name: "kudzu-star-filled",
    view: 24.0,
    body: r#"<g fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></g>"#,
};

/// Body text inherits `leading-5` (20 px) from `body`; sizes set with `text-[…px]` keep it.
const fn ty(size: f32, w: f32) -> TextStyle {
    TextStyle::new(size, w).lh(20.0)
}

pub fn view(s: &PaletteUi, cx: &mut Cx<PaletteUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    col()
        .key("palette")
        .role("dialog")
        .label("Quick search")
        .w_full()
        .h_full()
        .radius(PANEL_RADIUS)
        .system_material(SystemMaterial::Popover)
        // `vibrant:bg-popover/55 vibrant:shadow-none`: no border either; the panel's own rounded
        // window edge frames it, as in the Tauri build.
        .fill(t.popover.alpha(0.55))
        .color(t.popover_foreground)
        .child(search_bar(s, th, cx))
        .child(row().grow(1.0).min_h(0.0).w_full().child(results(s, th)).child(preview_pane(s, th, cx)))
        .child(footer(s, th))
        // `onKeyUp`: letting go of ⌘ or ⌃ dims the number hints.
        .on_key_up(|s: &mut PaletteUi, ev, tx| s.on_key(ev, tx) && matches!(ev.key, rune::Key::Modifier { .. }))
}

// MARK: Search

fn search_bar(s: &PaletteUi, th: &Theme, cx: &Cx<PaletteUi>) -> El {
    let t = &th.t;
    let input = text_input(&s.query, TextStyle::new(17.0, weight::NORMAL).lh(20.0).tracking(-0.17))
        .single_line()
        .key("pal-query")
        .grow(1.0)
        .min_w(0.0)
        .color(t.popover_foreground)
        .placeholder("Search snippets…", t.faint)
        .caret(t.popover_foreground, t.iris_soft)
        .spellcheck(false)
        .label("Search snippets")
        .role("combobox")
        .on_edit(|s: &mut PaletteUi| &mut s.query)
        .on_input_change(|s: &mut PaletteUi, tx| s.query_changed(tx))
        .on_input_key(|s: &mut PaletteUi, ev, tx| s.on_key(ev, tx));
    let input = match s.current() {
        Some(_) => input.active_descendant(row_key(s.clamped())),
        None => input,
    };
    // `h-[58px] border-b border-hairline px-4 gap-3`: the border is inside the 58.
    row()
        .h(58.0)
        .pb(1.0)
        .w_full()
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .child(icon(&icons::SEARCH, 18.0).color(t.muted_foreground))
        .child(input)
        .child(scopes(s, th, cx))
}

/// The All / Favorites / Recent radio group with its gliding pill.
fn scopes(s: &PaletteUi, th: &Theme, cx: &Cx<PaletteUi>) -> El {
    let t = &th.t;
    let selected = PALETTE_SCOPES.iter().position(|(v, _)| *v == s.scope).map(|i| format!("pal-scope-{i}"));
    let pill = SelectionPill::new("pal-scope", selected).fill(t.surface_raised).radius(radius::MD).shadows(&t.elev_soft).build(cx);
    let buttons = PALETTE_SCOPES.iter().enumerate().map(|(i, (scope, label))| {
        let on = *scope == s.scope;
        let scope = *scope;
        row()
            .key(format!("pal-scope-{i}"))
            .role("radio")
            .label(*label)
            .checked(on)
            .h(24.0)
            .px(8.0)
            .center()
            .radius(radius::MD)
            .child(text(*label).font(ty(11.0, weight::MEDIUM)).color(if on { t.foreground } else { t.muted_foreground }))
            .press_sink(0.97)
            .on_press_tx(move |s: &mut PaletteUi, tx| s.set_scope(scope, tx))
    });
    row()
        .key("pal-scope")
        .role("radiogroup")
        .label("Scope")
        .shrink()
        .padding(2.0)
        .radius(radius::LG)
        .fill(Theme::tint(t.foreground, 0.04))
        .elevate(&t.elev_well)
        .child(pill)
        .children(buttons)
}

// MARK: Results

fn results(s: &PaletteUi, th: &Theme) -> El {
    let t = &th.t;
    let list = col().key("pal-results").role("listbox").label(s.section_label()).w_frac(0.54).shrink().h_full().min_h(0.0);
    if s.total() == 0 {
        let (title, hint) = (
            match s.scope {
                kudzu_core::client::search::PaletteScope::Favorites => "No favorites yet.",
                kudzu_core::client::search::PaletteScope::Recent => "Nothing used yet.",
                _ => "No snippets yet.",
            },
            "Type to create one.",
        );
        return list.padding(6.0).child(
            col()
                .grow(1.0)
                .w_full()
                .center()
                .gap(4.0)
                .px(24.0)
                .child(text(title).font(TextStyle::new(14.0, weight::NORMAL).lh(20.0)).color(t.muted_foreground).align(rune::Align::Center))
                .child(text(hint).font(TextStyle::new(12.0, weight::NORMAL).lh(16.0)).color(t.faint).align(rune::Align::Center)),
        );
    }
    let clamped = s.clamped();
    let mut label = row().px(10.0).pt(6.0).pb(4.0).child(text(s.section_label()).font(ty(11.0, weight::MEDIUM)).color(t.faint));
    if let Some(g) = &s.group_filter {
        let g = if g.is_empty() { "…" } else { g.as_str() };
        label = label.child(text(format!("· #{g}")).font(ty(11.0, weight::MEDIUM)).color(t.iris_foreground).ml(4.0));
    }
    let rows = s.hits.iter().enumerate().map(|(i, h)| result_row(s, th, h, i, i == clamped));
    let mut content = col().w_full().padding(6.0).instant_changes().child(label).children(rows);
    if s.create_row {
        content = content.child(create_row(s, th, clamped == s.hits.len()));
    }
    list.scroll(ScrollSpec { stick_bottom: false }).child(content)
}

/// `KindIcon`.
fn kind_icon(snippet: &SnippetRecord) -> &'static Icon {
    let f = analyze(&snippet.content);
    if f.is_form {
        &icons::TEXT_CURSOR_INPUT
    } else if f.uses_clipboard {
        &icons::CLIPBOARD
    } else if snippet.content.contains("{{date") || snippet.content.contains("{{time") {
        &icons::CALENDAR_DAYS
    } else {
        &icons::FILE_TEXT
    }
}

/// The row shell both kinds of row share: `h-11 gap-3 rounded-[10px] px-2.5` inside Item's
/// 1 px transparent border.
fn row_shell(i: usize) -> El {
    row()
        .key(row_key(i))
        .role("option")
        .h(44.0)
        .w_full()
        .px(11.0)
        .gap(12.0)
        .center_y()
        .radius(RADIUS)
        .cursor(Cursor::Default)
        // `transition-colors duration-75`.
        .motion(Spring::new(0.075, 0.0))
        // `onMouseMove`: the row under a moving pointer is the active one.
        .on_pointer_move_local(move |s: &mut PaletteUi, _, tx: &mut Tx| {
            if s.clamped() == i {
                tx.live_only();
            } else {
                s.set_active(i, tx);
            }
        })
}

fn result_row(s: &PaletteUi, th: &Theme, hit: &Hit, i: usize, active: bool) -> El {
    let t = &th.t;
    let snippet = s.snippet(hit);
    let name = if snippet.name.is_empty() { "Untitled" } else { snippet.name.as_str() };
    let hits = if snippet.name.is_empty() { &[][..] } else { &hit.name_hits[..] };
    let spans: Vec<Span> = highlight_ranges(name, hits).into_iter().map(|r| Span::new(r.start, r.end).color(t.iris_foreground)).collect();
    let mut title = row()
        .w_full()
        .min_w(0.0)
        .gap(6.0)
        .center_y()
        .child(rich(name.to_string(), spans).key("title").font(TextStyle::new(13.5, weight::MEDIUM).lh(18.5625)).color(t.foreground).truncate().min_w(0.0).shrink());
    if snippet.is_favorite {
        title = title.child(icon(&STAR_FILLED, 12.0).key("star").color(t.favorite));
    }
    let path = snippet.group_id.and_then(|g| s.paths.get(&g)).filter(|p| !p.is_empty());
    let mut content = col().key("content").grow(1.0).min_w(0.0).child(title.key("title-line"));
    if let Some(p) = path {
        content = content.child(text(p.clone()).key("path").font(TextStyle::new(11.0, weight::NORMAL).lh(15.7143)).color(t.faint).truncate());
    }
    let media = row()
        .size(28.0, 28.0)
        
        .center()
        .radius(radius::LG)
        .motion(tw_spring())
        .fill(if active { t.iris } else { Theme::tint(t.foreground, 0.06) });
    let media = if active { media.elevate(&t.elev_accent).sheen(true) } else { media.elevate(&no_shadows(t.elev_accent.len())) };
    let media = media.child(icon(kind_icon(snippet), 14.0).color(if active { t.iris_contrast } else { t.muted_foreground }));
    let mut e = row_shell(i)
        .label(name.to_string())
        .checked(active)
        .fill(if active { Theme::tint(t.foreground, 0.08) } else { Color::TRANSPARENT })
        .child(media.key("media"))
        .child(content);
    if active {
        e = super::css_shadows(e, &t.elev_selected, RADIUS);
    } else {
        let tw = tween(0.075, TW_EASE);
        e = e.hover(Theme::tint(t.foreground, 0.03)).hover_motion(tw, tw);
    }
    if let Some(trigger) = snippet.triggers.first() {
        e = e.child(trigger_chip(th, trigger, !active).key("trigger"));
    }
    // Always shown; lights up while the modifier is held, when the number picks the row.
    if i < 9 {
        let tone = if s.show_numbers { Tone::Accent } else { Tone::Default };
        e = e.child(Shortcut::new(format!("mod+{}", i + 1)).tone(tone).build(th).key("shortcut"));
    }
    if !snippet.is_enabled {
        e = e.opacity(0.6);
    }
    e.on_press(move |s: &mut PaletteUi| s.pick(i, Mode::Insert))
}

fn no_shadows(n: usize) -> Vec<rune::Shadow> {
    vec![rune::Shadow { color: Color::TRANSPARENT, ..rune::Shadow::default() }; n]
}

fn create_row(s: &PaletteUi, th: &Theme, active: bool) -> El {
    let t = &th.t;
    let q = kudzu_core::client::js::trim(&s.query.text).to_string();
    let lead = "Create snippet ";
    let label = format!("{lead}“{q}”");
    let ink = if active { t.popover_foreground } else { t.muted_foreground };
    let i = s.hits.len();
    row_shell(i)
        .label(label.clone())
        .checked(active)
        .fill(if active { Theme::tint(t.foreground, 0.08) } else { Color::TRANSPARENT })
        .child(row().size(28.0, 28.0).center().radius(radius::LG).fill(Theme::tint(t.foreground, 0.06)).child(icon(&icons::PLUS, 14.0).color(ink)))
        .child(
            rich(label.clone(), vec![Span::new(lead.len(), label.len()).color(t.foreground)])
                .font(TextStyle::new(13.5, weight::NORMAL).lh(20.0))
                .color(ink)
                .truncate()
                .min_w(0.0)
                .shrink(),
        )
        .on_press(move |s: &mut PaletteUi| s.pick(i, Mode::Insert))
}

// MARK: Preview

fn preview_pane(s: &PaletteUi, th: &Theme, cx: &Cx<PaletteUi>) -> El {
    let t = &th.t;
    let aside = col()
        .key("pal-preview")
        .role("complementary")
        .label("Preview")
        .live_region(rune::LiveRegion::Polite)
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .pl(1.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.hairline)
        .fill(Theme::tint(t.foreground, 0.015));
    let Some(current) = s.current() else {
        let note = if s.create_row { "Opens Kudzu with a new snippet" } else { "" };
        return aside.center().child(text(note).font(TextStyle::new(12.0, weight::NORMAL).lh(16.0)).color(t.faint));
    };
    let facts = analyze(&current.content);
    let preview_font = TextStyle::new(13.5, weight::NORMAL).lh(21.9375);
    let body = match s.preview() {
        Some(r) if !r.text.is_empty() => FormattedText::new(r).font(preview_font).color(Theme::tint(t.foreground, 0.9)).build(th).selectable(true).selection_color(t.iris_soft),
        Some(_) => text("Empty snippet").font(preview_font).color(t.faint),
        // `animate-shimmer` while the preview renders.
        None => el().h(16.0).w_frac(2.0 / 3.0).radius(4.0).mt(2.75).linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, Color::TRANSPARENT), (0.5, Theme::tint(t.foreground, 0.08)), (1.0, Color::TRANSPARENT)]).cycle(rune::Cycle {
            period: 1.6,
            phase: 0.0,
            stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)],
            still: 1.0,
            fps: 60.0,
        }),
    };
    let mut top = col()
        .w_full()
        .px(20.0)
        .pt(16.0)
        .pb(12.0)
        .child(text("Will insert").font(ty(11.0, weight::MEDIUM)).color(t.faint).mb(8.0))
        .child(col().w_full().child(body));
    if facts.is_form {
        top = top.child(
            row()
                .mt(12.0)
                .self_start()
                .gap(6.0)
                .center_y()
                .radius(radius::MD)
                .fill(Theme::tint(t.warning, 0.12))
                .px(8.0)
                .py(4.0)
                .child(icon(&icons::TEXT_CURSOR_INPUT, 12.0).color(t.warning))
                .child(text(format!("Asks for {}", facts.field_names.join(", "))).font(ty(11.0, weight::MEDIUM)).color(t.warning)),
        );
    }
    let u = s.usage.get(&current.id).copied().unwrap_or_default();
    let used = if u.count != 0 {
        format!("{}× · {}", format_count(u.count), relative_time(u.last, chrono::Utc::now().timestamp_millis(), &chrono::Local).to_lowercase())
    } else {
        "Never".to_string()
    };
    let small = ty(11.5, weight::NORMAL);
    let mut slots = vec!["triggers"];
    let mut terms = vec![text("Triggers").font(small).color(t.faint)];
    let triggers = if current.triggers.is_empty() {
        row().child(text("None").font(small).color(t.muted_foreground))
    } else {
        row().flex_wrap().gap(4.0).children(current.triggers.iter().map(|tr| trigger_chip(th, tr, true)))
    };
    let mut defs = vec![triggers.min_w(0.0).min_h(20.0).center_y()];
    if let Some(g) = current.group_id {
        slots.push("group");
        terms.push(text("Group").font(small).color(t.faint));
        defs.push(
            row()
                .min_w(0.0)
                .gap(4.0)
                .center_y()
                .child(icon(&icons::FOLDER, 12.0).color(t.muted_foreground))
                .child(text(s.paths.get(&g).cloned().unwrap_or_default()).font(small).color(t.muted_foreground).truncate().min_w(0.0).shrink()),
        );
    }
    slots.push("used");
    terms.push(text("Used").font(small).color(t.faint));
    defs.push(row().child(text(used).font(small.features("'tnum' 1")).color(t.muted_foreground)));
    // `grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5`: each row as tall as its taller cell, the
    // terms' column as wide as the widest term (read back from layout).
    let dt_w = (0..terms.len()).filter_map(|i| cx.layout_size(&format!("pal-dt-{i}"))).map(|sz| sz[0]).fold(0.0_f32, f32::max);
    let rows = terms.into_iter().zip(defs).enumerate().map(|(i, (dt, dd))| row().key(slots[i]).w_full().gap(16.0).child(row().min_w(dt_w).child(dt.key(format!("pal-dt-{i}")))).child(dd.grow(1.0).min_w(0.0)));
    let dl = col()
        .w_full()
        
        .pt(13.0)
        .pb(12.0)
        .px(20.0)
        .gap(6.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .children(rows);
    aside.child(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(top)).child(dl)
}

// MARK: Footer

fn footer(s: &PaletteUi, th: &Theme) -> El {
    let t = &th.t;
    let small = ty(11.5, weight::NORMAL);
    let label = s.flash.clone().unwrap_or_else(|| match &s.target {
        Some(app) => format!("Insert into {}", app.name),
        None => "Insert".into(),
    });
    let hint = |combo: &str, what: &str| row().gap(6.0).center_y().child(Shortcut::new(combo).build(th)).child(text(what.to_string()).font(small).color(t.muted_foreground));
    let insert = row()
        .min_w(0.0)
        .gap(6.0)
        .center_y()
        .child(
            row()
                .size(18.0, 18.0)
                
                .center()
                .radius(5.0)
                .fill(t.iris)
                .elevate(&t.elev_accent)
                .sheen(true)
                .child(icon(&icons::CORNER_DOWN_LEFT, 12.0).color(t.iris_contrast)),
        )
        .child(text(label).font(small).color(t.muted_foreground).truncate().min_w(0.0).shrink());
    // `<span className="font-mono">#</span>group`, flex items `gap-1.5` apart.
    let group = row()
        .gap(6.0)
        .center_y()
        .child(text("#").font(small.family(rune::Family::Mono)).color(t.faint))
        .child(text("group").font(small).color(t.faint));
    row()
        .key("pal-footer")
        .h(40.0)
        .pt(1.0)
        .w_full()
        
        .px(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .child(insert)
        .child(hint("mod+enter", "Copy"))
        .child(hint("mod+e", "Edit"))
        .child(hint("tab", "Scope"))
        // `ml-auto` on the group hint: the last two sit at the end.
        .child(row().grow(1.0).justify_end().gap(16.0).center_y().child(group).child(Shortcut::new("escape").build(th)))
}
