//! The virtual clip list: sections, rows, highlights. Generic over the window state `S`: the
//! caller supplies an accessor to a [`ListState`] and the press handlers, so the main window
//! and the picker share it and nothing here names either (`PLAN.md` §1.3).
//!
//! Rows hold no hover state: a row sliding under a still pointer must not commit a view
//! (the §1.7 scroll budget), so hover is painted with `El::hover` only.

use std::borrow::Cow;

use chrono::{DateTime, FixedOffset};
use kudzu_core::clipboard::facts::{clip_time, concealed, describe, secret_reason, spoken_label};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, icon, row, text, El, GrowthSpec, ScrollSpec, Span, TextStyle, Transition, Tx};
use uuid::Uuid;

use super::model::{Click, ClipList, RowKind};
use super::motion::Census;
use super::tile::tile;
use crate::theme::{icons, Theme};
use crate::widgets::shortcut::{Shortcut, Tone};

/// Row, header and tile sizes (`SIZES` in `clip-list.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub const fn row(self) -> f32 {
        match self {
            Self::Comfortable => 58.0,
            Self::Compact => 50.0,
        }
    }
    pub const fn header(self) -> f32 {
        match self {
            Self::Comfortable => 30.0,
            Self::Compact => 28.0,
        }
    }
    pub const fn tile(self) -> f32 {
        match self {
            Self::Comfortable => 36.0,
            Self::Compact => 32.0,
        }
    }
}

/// What the rows draw besides the clips: everything a window decides.
#[derive(Clone, Debug)]
pub struct Look {
    pub density: Density,
    /// Lowercased search terms to mark.
    pub terms: Vec<String>,
    pub show_secrets: bool,
    pub now: DateTime<FixedOffset>,
    /// The clip just copied from here: a check lands next to its time.
    pub copied: Option<Uuid>,
    /// ⌘1–9 hints on the first nine rows (the picker).
    pub numbered: bool,
    pub show_numbers: bool,
    /// Whether the list has keyboard focus (the active row's ring).
    pub focused: bool,
}

impl Look {
    pub fn new(now: DateTime<FixedOffset>) -> Self {
        Self {
            density: Density::Comfortable,
            terms: Vec::new(),
            show_secrets: false,
            now,
            copied: None,
            numbered: false,
            show_numbers: false,
            focused: true,
        }
    }
}

/// A list's model and look, as one value a window state holds.
#[derive(Debug)]
pub struct ListState {
    pub list: ClipList,
    pub look: Look,
    pub census: Census,
}

impl ListState {
    pub fn new(now: DateTime<FixedOffset>) -> Self {
        Self {
            list: ClipList::default(),
            look: Look::new(now),
            census: Census::default(),
        }
    }
}

/// How a window reaches its list and answers the pointer.
pub struct Hooks<S: 'static> {
    pub state: fn(&S) -> &ListState,
    /// A press on a clip with the modifiers it carried.
    pub press: fn(&mut S, Uuid, Click, &mut Tx),
    /// A double press: copy it.
    pub pick: fn(&mut S, Uuid, &mut Tx),
}

impl<S: 'static> Clone for Hooks<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: 'static> Copy for Hooks<S> {}

/// Byte ranges of `text` that match `terms`. Core gives UTF-16 offsets into the *lowercased*
/// text; lowercasing can change the length (İ), so offsets are mapped by character and clamped.
pub fn highlight(text: &str, terms: &[String]) -> Vec<(usize, usize)> {
    if terms.is_empty() {
        return Vec::new();
    }
    let lower = text.to_lowercase();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let lower_utf16: Vec<usize> = lower
        .chars()
        .scan(0, |at, c| {
            let start = *at;
            *at += c.len_utf16();
            Some(start)
        })
        .collect();
    let to_byte = |utf16: usize| -> usize {
        let char_index = lower_utf16.partition_point(|&s| s < utf16);
        chars.get(char_index).map_or(text.len(), |&(b, _)| b)
    };
    kudzu_core::clipboard::query::match_ranges(text, terms)
        .into_iter()
        .map(|(s, e)| (to_byte(s), to_byte(e)))
        .filter(|(s, e)| s < e)
        .collect()
}

fn marked(
    theme: &Theme,
    shown: String,
    terms: &[String],
    style: TextStyle,
    ink: rune::Color,
) -> El {
    let spans: Vec<Span> = highlight(&shown, terms)
        .into_iter()
        .map(|(s, e)| Span::new(s, e).color(theme.c.iris_foreground))
        .collect();
    let mut el = text(shown).font(style).color(ink).single_line();
    if !spans.is_empty() {
        el = el.spans(spans);
    }
    el
}

fn dots(len: usize) -> String {
    "•".repeat(len.clamp(8, 18))
}

fn truncate(s: &str, chars: usize) -> String {
    s.chars().take(chars).collect()
}

/// Where a clip sits in the list and how the model marks it.
struct Place {
    position: usize,
    dated: bool,
    active: bool,
    selected: bool,
    arriving: bool,
}

/// The element for clip `e`.
fn clip_row<S: 'static>(
    theme: &Theme,
    hooks: Hooks<S>,
    e: &ClipEntry,
    look: &Look,
    at: Place,
) -> El {
    let Place {
        position,
        dated,
        active,
        selected,
        arriving,
    } = at;
    let c = &theme.c;
    let ty = &theme.ty;
    let hidden = concealed(e, look.show_secrets);
    let d = describe(e);
    let mono = matches!(e.kind, ClipKind::Code | ClipKind::Color) && !hidden;
    let (title, subtitle) = if hidden {
        (
            dots(d.title.chars().count()),
            format!("{} · hover to show", secret_reason(e)),
        )
    } else {
        (truncate(&d.title, 200), truncate(&d.subtitle, 200))
    };
    let terms: &[String] = if hidden { &[] } else { &look.terms };
    let title_style = if mono {
        TextStyle::new(12.5, 400.0).family(ty.mono.family)
    } else {
        TextStyle::new(13.0, 400.0)
    };
    let sub_style = if e.kind == ClipKind::Link {
        TextStyle::new(11.0, 400.0).family(ty.mono.family)
    } else {
        ty.meta_11_5
    };
    let id = e.id;
    let time = clip_time(e.copied_at, look.now, dated);
    let mut marks = row().h(14.0).gap(4.0).center_y();
    if e.copy_count > 1 {
        marks = marks.child(
            text(format!("×{}", e.copy_count))
                .font(ty.meta_10_5)
                .color(c.faint),
        );
    }
    if look.copied == Some(id) {
        marks = marks.child(icon(&icons::CHECK, 12.0).color(c.success));
    }
    if e.pinned {
        marks = marks.child(icon(&icons::PIN, 12.0).color(c.iris_foreground));
    }
    let mut line = row()
        .w_full()
        .h(look.density.row() - 2.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(10.0)
        .hover(c.foreground.alpha(0.035))
        .child(tile(
            theme,
            e,
            hidden,
            active && look.focused,
            look.density.tile(),
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(marked(theme, title, terms, title_style, c.foreground))
                .child(marked(
                    theme,
                    subtitle,
                    terms,
                    sub_style,
                    c.muted_foreground,
                )),
        )
        .child(
            col()
                .items_end()
                .gap(4.0)
                .shrink()
                .child(text(time).font(ty.meta_11).color(c.faint))
                .child(marks),
        );
    if look.numbered && position < 9 {
        let tone = if look.show_numbers {
            Tone::Accent
        } else {
            Tone::Ghost
        };
        line = line.child(
            Shortcut::combo(&format!("mod+{}", position + 1))
                .tone(tone)
                .build(theme),
        );
    }
    if selected {
        line = line.fill(c.foreground.alpha(0.06));
    }
    if active {
        line = line.fill(if look.focused {
            c.iris_soft
        } else {
            c.foreground.alpha(0.07)
        });
    }
    let line = line
        .key(format!("clip-{id}"))
        .role("option")
        .label(spoken_label(e, look.show_secrets, look.now))
        .on_press_tx(move |s: &mut S, tx: &mut Tx| {
            let mods = tx.mods();
            (hooks.press)(
                s,
                id,
                Click {
                    command: mods.cmd || mods.ctrl,
                    shift: mods.shift,
                },
                tx,
            );
        })
        .on_double_press_tx(move |s: &mut S, tx: &mut Tx| (hooks.pick)(s, id, tx));
    let line = if active || selected {
        line.a11y_value("selected")
    } else {
        line
    };
    let line = if arriving {
        line.transition(Transition::offset(0.0, -6.0).combined(Transition::opacity()))
    } else {
        line
    };
    col()
        .w_full()
        .h(look.density.row())
        .px(6.0)
        .py(1.0)
        .child(line)
}

fn header_row(theme: &Theme, density: Density, label: &str, count: usize) -> El {
    row()
        .w_full()
        .h(density.header())
        .px(16.0)
        .pb(6.0)
        .items_end()
        .gap(6.0)
        .role("presentation")
        .child(
            text(label.to_owned())
                .font(theme.ty.meta_11)
                .color(theme.c.faint),
        )
        .child(
            text(count.to_string())
                .font(theme.ty.meta_11)
                .color(theme.c.faint.alpha(0.7)),
        )
}

fn build_row<S: 'static>(theme: &'static Theme, hooks: Hooks<S>, s: &S, i: usize) -> El {
    let st = (hooks.state)(s);
    let Some(row) = st.list.rows().get(i) else {
        return col();
    };
    match &row.kind {
        RowKind::Header { label, count } => header_row(theme, st.look.density, label, *count),
        RowKind::Entry {
            id,
            index,
            dated,
            position,
        } => {
            let Some(e) = st.list.history().entries.get(*index) else {
                return col();
            };
            let place = Place {
                position: *position,
                dated: *dated,
                active: st.list.active() == Some(*id),
                selected: st.list.in_multi(*id),
                arriving: st.census.is_fresh(*id),
            };
            clip_row(theme, hooks, e, &st.look, place)
        }
    }
}

/// The scrolling list over `st`. `key` names the scroll viewport (one key per viewport); rows
/// are keyed by clip id, so handlers find their clip by id, never by index.
pub fn view<S: 'static>(
    theme: &'static Theme,
    hooks: Hooks<S>,
    key: &'static str,
    st: &ListState,
) -> El {
    let rows = col()
        .key(format!("{key}-rows"))
        .growth(GrowthSpec::default())
        .w_full()
        .pt(2.0)
        .pb(8.0)
        .virtual_rows(
            st.list.rows().len(),
            st.look.density.row(),
            move |s: &S, i| {
                Cow::Borrowed(
                    (hooks.state)(s)
                        .list
                        .rows()
                        .get(i)
                        .map_or("", |r| r.key.as_str()),
                )
            },
            move |s: &S, i| build_row(theme, hooks, s, i),
        )
        .virtual_rows_version(st.list.version());
    col()
        .key(key)
        .role("listbox")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(rows)
}

#[cfg(test)]
mod tests {
    use super::highlight;

    fn terms(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_match_maps_to_byte_offsets_in_the_shown_text() {
        assert_eq!(
            highlight("Invoice meeting", &terms(&["meeting"])),
            vec![(8, 15)]
        );
        assert_eq!(
            highlight("Invoice", &terms(&["inv", "ice"])),
            vec![(0, 3), (4, 7)]
        );
    }

    #[test]
    fn offsets_count_utf16_units_not_bytes() {
        // "é" is 2 bytes and 1 unit; the emoji is 4 bytes and 2 units.
        assert_eq!(highlight("é😀 note", &terms(&["note"])), vec![(7, 11)]);
    }

    #[test]
    fn lowercasing_that_changes_length_never_slices_inside_a_character() {
        // "İ" lowercases to two characters, so core's offsets drift; they are clamped.
        for (s, e) in highlight("İstanbul İzmir", &terms(&["zmir", "stanbul"])) {
            let text = "İstanbul İzmir";
            assert!(text.is_char_boundary(s) && text.is_char_boundary(e) && e <= text.len());
        }
    }

    #[test]
    fn no_terms_mark_nothing() {
        assert!(highlight("anything", &[]).is_empty());
    }
}
