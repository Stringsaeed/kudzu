//! Breadcrumbs (`components/app/breadcrumbs.tsx` on shadcn's `breadcrumb`): ancestors as
//! buttons in 13 point muted ink, chevrons between (`size-3.5`, faint, `gap-2`), the current
//! page in medium foreground ink, truncating. Past four items the trail collapses to the first
//! and the last two, with a "Show parent groups" menu for the rest. The current page replays
//! `rise` when the trail arrives somewhere new: key it on [`crate::motion::Arrivals`], which
//! skips mount and bursts (a held arrow key).

use rune::{icon, row, text, Color, El};
use rune_kit::{Menu, MenuItem};

use crate::theme::{icons, motion as tokens, radius, Theme};
use crate::widgets::kit_ext::menu;

/// One step of the trail. `id` says what it stands for when the label can change under it (a
/// snippet being renamed), so retyping is not an arrival.
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb {
    pub label: String,
    pub id: Option<String>,
    /// The crumb's element key instead of `crumb<i>` (galleries key parity samples).
    pub sample: Option<String>,
}

impl Crumb {
    pub fn new(label: impl Into<String>) -> Crumb {
        Crumb {
            label: label.into(),
            id: None,
            sample: None,
        }
    }
    pub fn sample(mut self, key: impl Into<String>) -> Self {
        self.sample = Some(key.into());
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    /// What arrivals are counted on.
    pub fn identity(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.label)
    }
}

/// Which items show, and which fold into the menu: past four, the first and the last two.
pub fn fold(count: usize) -> (Vec<usize>, Vec<usize>) {
    if count > 4 {
        (vec![0, count - 2, count - 1], (1..count - 2).collect())
    } else {
        ((0..count).collect(), Vec::new())
    }
}

pub struct Breadcrumbs {
    label: String,
    items: Vec<Crumb>,
    arrivals: u32,
    menu_open: bool,
    hovered: Option<usize>,
    hug: bool,
}

impl Breadcrumbs {
    pub fn new(items: Vec<Crumb>) -> Breadcrumbs {
        Breadcrumbs {
            label: "Breadcrumb".into(),
            items,
            arrivals: 0,
            menu_open: false,
            hovered: None,
            hug: false,
        }
    }
    /// The navigation's name (`aria-label`): the clipboard item's trail is "Clipboard item
    /// breadcrumb".
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }
    /// The arrival count (`Arrivals::observe` on the last item's identity).
    pub fn arrivals(mut self, n: u32) -> Self {
        self.arrivals = n;
        self
    }
    pub fn menu_open(mut self, open: bool) -> Self {
        self.menu_open = open;
        self
    }
    /// A forced hover on the ancestor at `index` (galleries); the runtime drives hover.
    pub fn hovered(mut self, index: usize) -> Self {
        self.hovered = Some(index);
        self
    }
    /// Hug the content instead of filling the row (the clipboard item's trail).
    pub fn hug(mut self) -> Self {
        self.hug = true;
        self
    }

    /// `on_select` gets the index of the chosen ancestor; `on_menu` opens and closes the menu
    /// of folded ancestors.
    pub fn build<S: 'static>(
        self,
        theme: &Theme,
        layer: &mut Vec<El>,
        on_select: impl Fn(&mut S, usize) + Clone + 'static,
        on_menu: impl Fn(&mut S, bool) + Clone + 'static,
    ) -> El {
        let c = &theme.c;
        let (shown, folded) = fold(self.items.len());
        let last = self.items.len().saturating_sub(1);
        let mut nav = row()
            .role("navigation")
            .label(self.label)
            .min_w(0.0)
            .gap(8.0)
            .center_y();
        if !self.hug {
            nav = nav.grow(1.0).shrink();
        }
        let chevron = |i: usize| {
            icon(&icons::CHEVRON_RIGHT, 14.0)
                .key(format!("sep{i}"))
                .color(c.faint)
                .a11y_hidden()
        };
        for (n, &i) in shown.iter().enumerate() {
            if n > 0 {
                nav = nav.child(chevron(i));
            }
            if n == 1 && !folded.is_empty() {
                nav = nav
                    .child(parents(
                        theme,
                        &self.items,
                        &folded,
                        self.menu_open,
                        layer,
                        &on_select,
                        &on_menu,
                    ))
                    .child(chevron(0));
            }
            let item = &self.items[i];
            if i == last {
                let mut page = text(item.label.clone())
                    .key(format!("page-{}", self.arrivals))
                    .role("link")
                    .label(item.label.clone())
                    .help_when_truncated(item.label.clone())
                    .font(theme.ty.body_medium)
                    .color(c.foreground)
                    .truncate()
                    .min_w(0.0);
                if !self.hug {
                    page = page.grow(1.0).shrink();
                }
                if self.arrivals > 0 {
                    page = page.transition(crate::motion::rise());
                }
                nav = nav.child(page);
            } else {
                let pick = on_select.clone();
                let hovered = self.hovered == Some(i);
                nav = nav.child(
                    row()
                        .key(item.sample.clone().unwrap_or_else(|| format!("crumb{i}")))
                        .role("button")
                        .label(item.label.clone())
                        .help_when_truncated(item.label.clone())
                        .focusable(true)
                        .max_w(160.0)
                        .min_w(0.0)
                        .shrink()
                        .radius(radius::SM)
                        .hover_group()
                        .on_press(move |s: &mut S| pick(s, i))
                        .child(
                            text(item.label.clone())
                                .font(theme.ty.body)
                                .color(if hovered {
                                    c.foreground
                                } else {
                                    c.muted_foreground
                                })
                                .group_color(c.foreground)
                                .truncate()
                                .a11y_hidden(),
                        ),
                );
            }
        }
        nav
    }
}

/// The folded ancestors: a `size-6` ghost trigger and a menu of their names.
fn parents<S: 'static>(
    theme: &Theme,
    items: &[Crumb],
    folded: &[usize],
    open: bool,
    layer: &mut Vec<El>,
    on_select: &(impl Fn(&mut S, usize) + Clone + 'static),
    on_menu: &(impl Fn(&mut S, bool) + Clone + 'static),
) -> El {
    let c = &theme.c;
    let toggle = on_menu.clone();
    let trigger = row()
        .key("parents")
        .role("button")
        .label("Show parent groups")
        .focusable(true)
        .size(24.0, 24.0)
        .center()
        .radius(radius::MD)
        .fill(if open { c.accent } else { Color::TRANSPARENT })
        .hover(c.accent)
        .press_scale(tokens::press::SCALE)
        .on_press(move |s: &mut S| toggle(s, !open))
        .child(icon(&icons::MORE_HORIZONTAL, 16.0).color(c.muted_foreground));
    let close = on_menu.clone();
    let mut m = Menu::new("breadcrumb-parents")
        .trigger(trigger)
        .open(open)
        .on_close(move |s: &mut S| close(s, false));
    for &i in folded {
        let (pick, close) = (on_select.clone(), on_menu.clone());
        m = m.item(
            MenuItem::new(items[i].label.clone()).on_select(move |s: &mut S| {
                close(s, false);
                pick(s, i);
            }),
        );
    }
    menu::build(m, theme, layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime};

    #[test]
    fn past_four_items_the_trail_keeps_the_first_and_last_two() {
        assert_eq!(fold(4), (vec![0, 1, 2, 3], vec![]));
        assert_eq!(fold(5), (vec![0, 3, 4], vec![1, 2]));
        assert_eq!(fold(7), (vec![0, 5, 6], vec![1, 2, 3, 4]));
        assert_eq!(fold(1), (vec![0], vec![]));
    }

    fn trail(n: usize) -> String {
        fn view(n: &usize, _: &mut Cx<usize>) -> El {
            let items = (0..*n).map(|i| Crumb::new(format!("Level {i}"))).collect();
            let mut layer = Vec::new();
            let nav = Breadcrumbs::new(items).build(
                Theme::light(),
                &mut layer,
                |_: &mut usize, _| {},
                |_: &mut usize, _| {},
            );
            rune_kit::root(rune::col().child(nav), layer)
        }
        let mut rt = Runtime::new(n, view, (900.0, 60.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.accessibility_json().to_string()
    }

    #[test]
    fn the_tree_shows_the_folded_trail() {
        let t = trail(6);
        for shown in [
            "Level 0",
            "Level 4",
            "Level 5",
            "Show parent groups",
            "Breadcrumb",
        ] {
            assert!(t.contains(shown), "{shown} missing: {t}");
        }
        for folded in ["Level 1", "Level 2", "Level 3"] {
            assert!(!t.contains(folded), "{folded} shows: {t}");
        }
        let t = trail(4);
        assert!(t.contains("Level 1") && !t.contains("Show parent groups"));
    }

    /// The reference's spacing: each crumb starts 8 + 14 + 8 points after the one before
    /// (`gap-2` either side of a `size-3.5` chevron), as "Snippets" → "Work" measures.
    #[test]
    fn crumbs_sit_a_chevron_and_two_gaps_apart() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            let items = ["Snippets", "Work", "Email", "Page"]
                .map(Crumb::new)
                .to_vec();
            let nav = Breadcrumbs::new(items).build(
                Theme::light(),
                &mut Vec::new(),
                |_: &mut (), _| {},
                |_: &mut (), _| {},
            );
            rune::col().child(rune::row().w(600.0).child(nav))
        }
        let mut rt = Runtime::new((), view, (700.0, 60.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let els = rt.elements();
        let at = |k: &str| {
            els.iter()
                .find(|e| e.key.as_deref() == Some(k))
                .unwrap()
                .rect
        };
        let (a, b, c) = (at("crumb0"), at("crumb1"), at("crumb2"));
        assert!((b[0] - (a[0] + a[2]) - 30.0).abs() < 0.01, "{a:?} {b:?}");
        assert!((c[0] - (b[0] + b[2]) - 30.0).abs() < 0.01);
        assert_eq!((a[3], b[3]), (20.0, 20.0));
    }
}
