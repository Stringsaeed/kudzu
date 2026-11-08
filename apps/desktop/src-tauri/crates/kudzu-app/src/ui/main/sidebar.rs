//! The library sidebar (`components/app/main/sidebar.tsx`): the wordmark, the Snippets and
//! Clipboard sections (folded state remembered on this Mac), the nested group tree with drag and
//! drop, context menus and rename in place, Insights and Settings, and the status footer.
//!
//! Keyboard: the nav is one focus group (Up, Down, Home and End rove between items, clamped at
//! the ends, as `onNavKeyDown` did); a section heading folds on Return, Space, ← and →; a group
//! row opens and closes with → and ←, and renames on F2 or ⌥Return.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use rune::{col, el, icon, row, text, text_input, Cx, El, Icon, Key, Pixels, Side, Spring, TextEdit};
use uuid::Uuid;

use super::library::drag::{self, DragState, Payload, Target, TOP_KEY};
use super::library::{self as lib, rise, tick};
use super::{ClipFilter, MainUi, View};
use crate::ui::icons;
use crate::ui::kit::badge::Badge;
use crate::ui::kit::button::{fast_spring, hover_fast, layer, tween, EASE};
use crate::ui::kit::context_menu::ContextMenu;
use crate::ui::kit::dropdown_menu::{MenuItem, MenuState};
use crate::ui::kit::selection_pill::SelectionPill;
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::kit::wordmark::{wordmark, WordmarkVariant};
use crate::ui::theme::{motion, radius, text as ty, weight, Theme, Utilities, WeightOf, MOTION_MODERATE};
use crate::ui::theme_for;
use kudzu_core::client::groups::{build_tree, recursive_counts, GroupNode};
use kudzu_core::GroupRecord;

pub struct SidebarState {
    /// Folded feature sections (`"snippets"`, `"clipboard"`), remembered on this Mac.
    pub closed: BTreeSet<String>,
    /// Groups whose children are hidden (this session only, as on the web).
    pub collapsed: HashSet<Uuid>,
    /// The group being renamed in place, and its field.
    pub renaming: Option<Uuid>,
    pub rename: TextEdit,
    /// The group rows' context menu (one open at a time) and whose it is.
    pub menu: MenuState,
    pub menu_group: Option<Uuid>,
    /// A snippet or group being dragged onto the sidebar.
    pub drag: Option<DragState>,
    /// The pointer is on the "New group" button (its plus turns).
    pub plus_hover: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        SidebarState {
            closed: BTreeSet::new(),
            collapsed: HashSet::new(),
            renaming: None,
            rename: TextEdit::new(""),
            menu: MenuState::default(),
            menu_group: None,
            drag: None,
            plus_hover: false,
        }
    }
}

impl SidebarState {
    /// The state with the folded sections this Mac remembers.
    pub fn load() -> SidebarState {
        SidebarState { closed: lib::load_sections(), ..Default::default() }
    }

    pub fn toggle_section(&mut self, id: &str) {
        if !self.closed.remove(id) {
            self.closed.insert(id.to_string());
        }
        lib::save_sections(&self.closed);
    }

    pub fn toggle_collapsed(&mut self, id: Uuid) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
    }

    /// `setRenaming(id)`: the field opens with the name selected and focused.
    pub fn start_rename(&mut self, group: &GroupRecord) {
        let mut edit = TextEdit::new(group.name.clone());
        edit.set_selection(0, group.name.len());
        edit.focused = true;
        self.rename = edit;
        self.renaming = Some(group.id);
    }
}

/// `RenameRow`'s `onDone`: a changed, non-empty name saves; `None` (Escape) keeps the old one.
pub fn finish_rename(s: &mut MainUi, commit: bool) {
    let Some(id) = s.sidebar.renaming.take() else { return };
    let name = s.sidebar.rename.text.trim().to_string();
    s.sidebar.rename = TextEdit::new("");
    if !commit || name.is_empty() {
        return;
    }
    if let Some(mut g) = s.group(id).cloned()
        && g.name != name
    {
        g.name = name;
        s.save_group(g);
    }
}

/// `createGroup("New group", parent).then((g) => setRenaming(g.id))`.
pub fn new_group(s: &mut MainUi, parent: Option<Uuid>) {
    let g = s.create_group("New group", parent);
    s.sidebar.start_rename(&g);
}

/// `sameView`.
fn same_view(a: &View, b: &View) -> bool {
    a == b
}

/// `CLIP_ICONS`.
fn clip_icon(f: ClipFilter) -> &'static Icon {
    match f {
        ClipFilter::Pinned => &icons::PIN,
        ClipFilter::Image => &icons::IMAGE,
        ClipFilter::Link => &icons::GLOBE,
        ClipFilter::File => &icons::FILE,
        ClipFilter::Code => &icons::BRACES,
        ClipFilter::Color => &icons::PALETTE,
        ClipFilter::All | ClipFilter::Text | ClipFilter::Other => &icons::TEXT_ALIGN_START,
    }
}

/// The folders `FILTERS` lists after History, in order.
const CLIP_FOLDERS: [ClipFilter; 7] = [ClipFilter::Pinned, ClipFilter::Text, ClipFilter::Image, ClipFilter::Link, ClipFilter::File, ClipFilter::Code, ClipFilter::Color];

/// `/icon.png`, decoded once.
fn app_icon() -> &'static Pixels {
    static ICON: OnceLock<Pixels> = OnceLock::new();
    ICON.get_or_init(|| {
        let p = Pixels::new();
        if let Ok(img) = image::load_from_memory(include_bytes!("../../../../../../public/icon.png")) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            p.set(w, h, rgba.into_raw());
        }
        p
    })
}

// MARK: NavItem

/// `NavItem`'s props.
struct Nav {
    key: String,
    icon: &'static Icon,
    label: String,
    count: Option<usize>,
    active: bool,
    depth: usize,
    muted: bool,
    shortcut: Option<&'static str>,
    trailing: Vec<El>,
    drop: bool,
    expanded: Option<bool>,
}

impl Nav {
    fn new(key: impl Into<String>, icon: &'static Icon, label: impl Into<String>, active: bool) -> Nav {
        Nav { key: key.into(), icon, label: label.into(), count: None, active, depth: 0, muted: false, shortcut: None, trailing: Vec::new(), drop: false, expanded: None }
    }
    fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }
    fn shortcut(mut self, s: &'static str) -> Self {
        self.shortcut = Some(s);
        self
    }

    /// `group/nav relative flex h-7 w-full items-center gap-2 rounded-md pr-2 text-[13px] press
    /// [--press-scale:0.985]`, `paddingLeft: 8 + depth * 14`.
    fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let r = radius::MD;
        let rest = if self.drop {
            t.iris_foreground
        } else if self.muted {
            Theme::tint(t.muted_foreground, 0.7)
        } else if self.active {
            t.sidebar_foreground
        } else {
            Theme::tint(t.sidebar_foreground, 0.8)
        };
        let hover_ink = if self.drop { t.iris_foreground } else { t.sidebar_foreground };
        let mut e = hover_fast(row())
            .key(self.key.clone())
            .role("button")
            .label(self.label.clone())
            .h(28.0)
            .w_full()
            .gap(8.0)
            .pl(8.0 + self.depth as f32 * 14.0)
            .pr(8.0)
            .center_y()
            .radius(r)
            .motion(fast_spring());
        // The hover tint is always a child (fully hidden while active or a drop target), and every
        // child is keyed: Rune matches unkeyed siblings by position, so adding or removing one when
        // the selection moved re-paired the icon, label and count with other elements and
        // cross-faded them.
        let hover = layer(0.0).key("hover").radius(r).fill(t.sidebar_accent);
        e = e.child(if self.active || self.drop { hover.group_opacity(0.0, 0.0) } else { hover.group_opacity(0.0, 1.0) });
        // The drop highlight: `scale-[1.02] bg-iris-soft text-iris-foreground ring-1 ring-iris/40`.
        e = if self.drop {
            e.fill(t.iris_soft).outline(1.0, 0.0, Theme::tint(t.iris, 0.4)).scale(1.02)
        } else {
            e.fill(rune::Color::TRANSPARENT).scale(1.0)
        }
        .scale_motion(motion::pop());
        e = e
            .child(icon(self.icon, 16.0).key("icon").color(if self.active { t.iris } else { t.muted_foreground }).a11y_hidden())
            .child(text(self.label).key("label").font(ty::BASE).color(rest).group_color(hover_ink).truncate().grow(1.0).shrink().min_w(0.0).a11y_hidden())
            .children(self.trailing);
        if let Some(n) = self.count.filter(|&n| n > 0) {
            e = e.child(tick(th, &format!("{}-count", self.key), n, lib::tabular(ty::XS2), t.faint).a11y_hidden());
        }
        if let Some(sc) = self.shortcut {
            e = e.child(Shortcut::new(sc).build(th).key("shortcut"));
        }
        if self.active {
            e = e.checked(true);
        }
        if let Some(open) = self.expanded {
            e = e.expanded(open);
        }
        e.press_scale(0.985).focusable(true).focus_ring(2.0, 0.0, t.ring)
    }
}

/// A plain view item (`item(v, icon, label, count, shortcut)`).
#[allow(clippy::too_many_arguments)]
fn item(th: &Theme, s: &MainUi, key: &str, v: View, icon: &'static Icon, label: &str, count: Option<usize>, shortcut: Option<&'static str>) -> El {
    let mut n = Nav::new(key, icon, label, same_view(&s.view, &v));
    if let Some(c) = count {
        n = n.count(c);
    }
    if let Some(sc) = shortcut {
        n = n.shortcut(sc);
    }
    n.build(th).on_press(move |s: &mut MainUi| s.set_view(v.clone()))
}

// MARK: Section header

/// `SectionHeader`: a feature's heading. Click, Return or ←/→ fold it; the chevron shows on hover
/// and focus, like Finder's sidebar.
fn section_header(th: &Theme, id: &'static str, label: &str, open: bool, trailing: Option<El>) -> El {
    let t = &th.t;
    let chevron = icon(&icons::CHEVRON_RIGHT, 12.0)
        .color(t.faint)
        .group_color(t.muted_foreground)
        .icon_rotation(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 })
        .motion(motion::moderate())
        .a11y_hidden();
    // `opacity-0 group-hover/sec:opacity-100 group-focus-within/sec:opacity-100`, and always shown
    // while folded.
    let chevron = if open { chevron.group_opacity(0.0, 1.0) } else { chevron };
    let button = hover_fast(row())
        .key(format!("sec-{id}"))
        .role("button")
        .label(label.to_string())
        .expanded(open)
        .h(24.0)
        .ml(-4.0)
        .px(4.0)
        .gap(4.0)
        .grow(1.0)
        .min_w(0.0)
        .center_y()
        .radius(radius::MD)
        .child(text(label.to_string()).font(section_font()).color(t.faint).group_color(t.muted_foreground).truncate().shrink().a11y_hidden())
        .child(chevron)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .on_press(move |s: &mut MainUi| s.sidebar.toggle_section(id))
        .on_key(move |s: &mut MainUi, ev, _| {
            if (ev.key == Key::Left && open) || (ev.key == Key::Right && !open) {
                s.sidebar.toggle_section(id);
                return true;
            }
            false
        });
    let mut r = row().key(format!("sec-row-{id}")).hover_group().hover_group_focus().h(28.0).gap(4.0).pr(4.0).pl(8.0).center_y().child(button);
    if let Some(tr) = trailing {
        r = r.child(tr);
    }
    r
}

/// `text-[11px] font-semibold tracking-wide`.
fn section_font() -> rune::TextStyle {
    rune::TextStyle { tracking: 11.0 * 0.025, ..ty::XS2.weight_of(weight::SEMIBOLD) }
}

// MARK: Groups

/// `renderNodes`: each group in a `role="none"` box holding its row and, unless folded, its
/// children's boxes (so nested rows sit flush; only top-level siblings get the list's `gap-px`).
fn group_rows(th: &Theme, s: &MainUi, cx: &mut Cx<MainUi>, nodes: &[GroupNode<'_>], counts: &HashMap<Uuid, usize>, layer_out: &mut Vec<El>, out: &mut Vec<El>) {
    for n in nodes {
        let mut boxed = col().key(format!("group-box-{}", n.group.id)).w_full().child(group_row(th, s, cx, n, counts, layer_out));
        if !s.sidebar.collapsed.contains(&n.group.id) {
            let mut kids = Vec::new();
            group_rows(th, s, cx, &n.children, counts, layer_out, &mut kids);
            boxed = boxed.children(kids);
        }
        out.push(boxed);
    }
}

/// `RenameRow`.
fn rename_row(th: &Theme, s: &MainUi, depth: usize) -> El {
    let t = &th.t;
    let field = text_input(&s.sidebar.rename, ty::BASE)
        .single_line()
        .key("group-rename-input")
        .label("Group name")
        .h(24.0)
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .color(t.foreground)
        .caret(t.foreground, t.iris_soft)
        .on_edit(|s: &mut MainUi| &mut s.sidebar.rename)
        .on_input_key(|s: &mut MainUi, ev, _| match ev.key {
            Key::Enter => {
                finish_rename(s, true);
                true
            }
            Key::Escape => {
                finish_rename(s, false);
                true
            }
            _ => false,
        });
    let mut r = row()
        .key("group-rename")
        .h(28.0)
        .w_full()
        .gap(8.0)
        .pl(8.0 + depth as f32 * 14.0)
        .pr(4.0)
        .center_y()
        .radius(radius::MD)
        .fill(t.sidebar_accent)
        .child(icon(&icons::FOLDER, 16.0).color(t.iris).a11y_hidden())
        .child(field);
    // `onBlur={() => onDone(name.trim())}`: the field lost focus without Return or Escape.
    if !s.sidebar.rename.focused {
        r = r.child(lib::on_mount("group-rename-blur", |s: &mut MainUi, _| finish_rename(s, true)));
    }
    r
}

fn group_row(th: &Theme, s: &MainUi, cx: &mut Cx<MainUi>, node: &GroupNode<'_>, counts: &HashMap<Uuid, usize>, layer_out: &mut Vec<El>) -> El {
    let t = &th.t;
    let g = node.group;
    let id = g.id;
    if s.sidebar.renaming == Some(id) {
        return rename_row(th, s, node.depth);
    }
    let has_children = !node.children.is_empty();
    let collapsed = s.sidebar.collapsed.contains(&id);
    let active = s.view == View::Group(id);
    let mut trailing = Vec::new();
    if !g.is_enabled {
        trailing.push(icon(&icons::CIRCLE_OFF, 12.0).color(t.faint).label("Off"));
    }
    if has_children {
        // `-mr-1 flex size-5 items-center justify-center rounded text-faint hover:text-foreground`.
        trailing.push(
            row()
                .key(format!("group-chevron-{id}"))
                .hover_group()
                .hover_motion(tween(0.08, EASE), tween(0.08, EASE))
                .size(20.0, 20.0)
                .mr(-4.0)
                .center()
                .radius(radius::SM)
                .role("presentation")
                .child(
                    icon(&icons::CHEVRON_RIGHT, 12.0)
                        .color(t.faint)
                        .group_color(t.foreground)
                        .icon_rotation(if collapsed { 0.0 } else { std::f32::consts::FRAC_PI_2 })
                        .motion(Spring::new(MOTION_MODERATE, 0.05)),
                )
                .on_press(move |s: &mut MainUi| s.sidebar.toggle_collapsed(id))
                .a11y_hidden(),
        );
    }
    let nav = Nav {
        key: drag::group_key(id),
        icon: if collapsed || !has_children { &icons::FOLDER } else { &icons::FOLDER_OPEN },
        label: g.name.clone(),
        count: counts.get(&id).copied(),
        active,
        depth: node.depth,
        muted: !g.is_enabled,
        shortcut: None,
        trailing,
        drop: drag::is_over(s, Target::Group(id)),
        expanded: has_children.then_some(!collapsed),
    };
    let name = g.name.clone();
    let row_el = nav
        .build(th)
        .on_press(move |s: &mut MainUi| s.set_view(View::Group(id)))
        .on_double_press(move |s: &mut MainUi| {
            if let Some(g) = s.group(id).cloned() {
                s.sidebar.start_rename(&g);
            }
        })
        .on_drag(move |s: &mut MainUi, d, tx| drag::track(s, d, tx, |_| Some((Payload::Group(id), name.clone()))))
        .on_key(move |s: &mut MainUi, ev, _| {
            let rename = ev.key == Key::Function(2) || (ev.key == Key::Enter && ev.mods.alt);
            if rename {
                if let Some(g) = s.group(id).cloned() {
                    s.sidebar.start_rename(&g);
                }
                return true;
            }
            if (ev.key == Key::Right && has_children && collapsed) || (ev.key == Key::Left && has_children && !collapsed) {
                s.sidebar.toggle_collapsed(id);
                return true;
            }
            false
        });

    let enabled = g.is_enabled;
    let parent = g.parent_id;
    // The menu keys its trigger `{key}-area`: that is the row's (and its drop zone's) key.
    let mut menu = ContextMenu::new(drag::group_menu_key(id), &s.sidebar.menu, |s: &mut MainUi| &mut s.sidebar.menu)
        .open(s.sidebar.menu_group == Some(id))
        .trigger(row_el)
        .width(208.0)
        .item(MenuItem::new("Rename").icon(&icons::PENCIL).on_select(move |s: &mut MainUi, _| {
            if let Some(g) = s.group(id).cloned() {
                s.sidebar.start_rename(&g);
            }
        }))
        .item(MenuItem::new("New subgroup").icon(&icons::FOLDER_PLUS).on_select(move |s: &mut MainUi, _| new_group(s, Some(id))))
        .item(MenuItem::new("Group settings…").icon(&icons::SLIDERS_HORIZONTAL).on_select(move |s: &mut MainUi, _| super::group_settings::open(s, id)))
        .item(MenuItem::new(if enabled { "Turn off group" } else { "Turn on group" }).icon(if enabled { &icons::CIRCLE_OFF } else { &icons::PLAY }).on_select(move |s: &mut MainUi, _| {
            if let Some(mut g) = s.group(id).cloned() {
                g.is_enabled = !g.is_enabled;
                s.save_group(g);
            }
        }));
    if parent.is_some() {
        menu = menu.item(MenuItem::new("Move to top level").icon(&icons::LAYERS).on_select(move |s: &mut MainUi, _| {
            if let Some(mut g) = s.group(id).cloned() {
                g.parent_id = None;
                s.save_group(g);
            }
        }));
    }
    menu = menu
        .separator()
        .item(MenuItem::new("Delete group, keep snippets").icon(&icons::TRASH).destructive().on_select(move |s: &mut MainUi, _| s.delete_group(id, false)))
        .item(MenuItem::new("Delete group and snippets").icon(&icons::TRASH).destructive().on_select(move |s: &mut MainUi, _| s.delete_group(id, true)));
    menu.build_with(cx, layer_out, move |s: &mut MainUi| s.sidebar.menu_group = Some(id))
}

// MARK: Sidebar

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer_out: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let lib_ = &s.library;
    let tree = build_tree(&lib_.groups);
    let counts = recursive_counts(&lib_.groups, &lib_.snippets);
    let favorites = lib_.snippets.iter().filter(|x| x.is_favorite).count();
    let disabled = lib_.snippets.iter().filter(|x| !x.is_enabled).count();
    let recent = s.usage.values().filter(|u| u.last != 0).count();
    let clip_counts = crate::ui::clipboard::folder_counts(s);
    let clip_on = s.settings.clipboard_enabled;
    let active_clip = match s.view {
        View::Clipboard(f) => Some(f),
        _ => None,
    };

    // Header: the app icon and wordmark (all snippets), and the Dev badge.
    let mut header = row()
        .key("sidebar-header")
        .h(72.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .child(
            hover_fast(row())
                .key("sidebar-home")
                .role("button")
                .label("Kudzu — all snippets")
                .min_w(0.0)
                .gap(10.0)
                .padding(4.0)
                .center_y()
                .radius(radius::LG)
                .child(el().size(36.0, 36.0).pixels(app_icon()).a11y_hidden())
                .child(
                    wordmark(WordmarkVariant::Small, 76.0, Theme::tint(t.foreground, 0.9))
                        .group_color(t.foreground)
                        .a11y_hidden(),
                )
                .focusable(true)
                .focus_ring(2.0, 0.0, t.ring)
                .on_press(|s: &mut MainUi| s.set_view(View::All)),
        );
    if s.dev {
        header = header.child(el().grow(1.0)).child(Badge::new("Dev").build(th));
    }

    // Snippets.
    let snippets_open = !s.sidebar.closed.contains("snippets");
    let mut snippets = col().key("sec-snippets-wrap").role("group").label("Snippets").gap(1.0).child(section_header(th, "snippets", "Snippets", snippets_open, None));
    if snippets_open {
        let mut views = col()
            .gap(1.0)
            .child(item(th, s, "nav-all", View::All, &icons::LAYERS, "All snippets", Some(lib_.snippets.len()), Some("mod+1")))
            .child(item(th, s, "nav-favorites", View::Favorites, &icons::STAR, "Favorites", Some(favorites), Some("mod+2")))
            .child(item(th, s, "nav-recent", View::Recent, &icons::CLOCK_3, "Recently used", Some(recent), Some("mod+3")));
        if disabled > 0 {
            views = views.child(item(th, s, "nav-disabled", View::Disabled, &icons::CIRCLE_OFF, "Turned off", Some(disabled), None));
        }

        // The "Groups" heading: a drop zone for the top level, and the "New group" button.
        // `size-5 rounded text-faint opacity-60 press hover:bg-sidebar-accent hover:text-foreground
        // hover:opacity-100`, its plus turning a quarter on hover.
        let turned = s.sidebar.plus_hover;
        let plus = row()
            .key("new-group")
            .role("button")
            .label("New group")
            .size(20.0, 20.0)
            .center()
            .radius(radius::SM)
            .hover_group()
            .hover_motion(tween(0.08, EASE), tween(0.08, EASE))
            .child(layer(0.0).radius(radius::SM).fill(t.sidebar_accent).group_opacity(0.0, 1.0))
            .child(
                icon(&icons::PLUS, 14.0)
                    .color(t.faint)
                    .group_color(t.foreground)
                    .icon_rotation(if turned { std::f32::consts::FRAC_PI_2 } else { 0.0 })
                    .motion(motion::pop())
                    .a11y_hidden(),
            )
            .layer_opacity(if turned { 1.0 } else { 0.6 })
            .press_sink(0.97)
            .focusable(true)
            .focus_ring(2.0, 0.0, t.ring)
            .on_hover_change(|s: &mut MainUi, inside, _| s.sidebar.plus_hover = inside)
            .on_press(|s: &mut MainUi| new_group(s, None));
        let plus = Tooltip::new("new-group-tip").side(Side::Right).text("New group").shortcut("mod+shift+n").build(cx, plus, layer_out);
        let mut groups = col().gap(1.0).child(
            row()
                .h(24.0)
                .pr(4.0)
                .pl(8.0)
                .center_y()
                .justify_between()
                .child(row().key(TOP_KEY).child(text("Groups").font(rune::TextStyle { tracking: 11.0 * 0.025, ..ty::XS2.weight_of(weight::MEDIUM) }).color(t.faint)))
                .child(plus),
        );
        if tree.is_empty() {
            groups = groups.child(
                hover_fast(row())
                    .key("groups-empty")
                    .role("button")
                    .mx(4.0)
                    .px(8.0)
                    .py(8.0)
                    .radius(radius::MD)
                    .border(1.0, t.border)
                    .border_dash(4.0, 3.0)
                    .child(text("Organize snippets into groups — like Work, Email or Code.").font(ty::XS).color(t.muted_foreground).group_color(t.foreground).wrap())
                    .focusable(true)
                    .focus_ring(2.0, 0.0, t.ring)
                    .on_press(|s: &mut MainUi| new_group(s, None)),
            );
        } else {
            let mut rows = Vec::new();
            group_rows(th, s, cx, &tree, &counts, layer_out, &mut rows);
            groups = groups.children(rows);
        }
        snippets = snippets.child(col().key("sec-snippets").gap(12.0).transition(rise()).child(views).child(groups));
    }

    // Clipboard.
    let clip_open = !s.sidebar.closed.contains("clipboard");
    let paused = (!clip_on).then(|| {
        row()
            .mr(4.0)
            .px(6.0)
            .radius(radius::FULL)
            .fill(Theme::tint(t.warning, 0.15))
            .child(text("Paused").font(rune::TextStyle::new(10.0, weight::MEDIUM).lh(15.0)).color(t.warning))
    });
    let mut clipboard = col().key("sec-clipboard-wrap").role("group").label("Clipboard").gap(1.0).child(section_header(th, "clipboard", "Clipboard", clip_open, paused));
    if clip_open {
        let count = |f: ClipFilter| clip_counts.get(&f).copied().unwrap_or(0);
        let mut items = col().key("sec-clipboard").gap(1.0).transition(rise()).child(
            Nav::new("nav-clip-all", &icons::CLIPBOARD_LIST, "History", active_clip == Some(ClipFilter::All))
                .count(count(ClipFilter::All))
                .shortcut("mod+5")
                .build(th)
                .on_press(|s: &mut MainUi| s.set_view(View::Clipboard(ClipFilter::All))),
        );
        for f in CLIP_FOLDERS {
            if !(f == ClipFilter::Pinned || count(f) > 0 || active_clip == Some(f)) {
                continue;
            }
            items = items.child(
                Nav::new(format!("nav-clip-{f:?}"), clip_icon(f), super::toolbar::clip_label(f), active_clip == Some(f))
                    .count(count(f))
                    .build(th)
                    .on_press(move |s: &mut MainUi| s.set_view(View::Clipboard(f))),
            );
        }
        clipboard = clipboard.child(items);
    }

    // The nav: one scroll area, one focus group, one sliding selection pill.
    let active_key = active_nav_key(s);
    // Each group's pill only follows its own items.
    let in_footer = matches!(s.view, View::Insights | View::Settings(_));
    let (nav_key, footer_key) = if in_footer { (None, active_key) } else { (active_key, None) };
    let content = col()
        .key("nav-content")
        .w_full()
        .gap(12.0)
        .px(10.0)
        .pb(12.0)
        .child(pill_slot(SelectionPill::new("nav-content", nav_key).fill(t.sidebar_accent).radius(radius::MD).shadows(&t.elev_selected).build(cx)))
        .child(snippets)
        .child(clipboard);
    let nav = col()
        .key("nav")
        .role("tree")
        .label("Library")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(rune::ScrollSpec { stick_bottom: false })
        .focus_group()
        .child(content);

    let footer = col()
        .key("nav-footer")
        .role("tree")
        .w_full()
        .gap(1.0)
        .px(10.0)
        .py(8.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .focus_group()
        .child(pill_slot(SelectionPill::new("nav-footer", footer_key).fill(t.sidebar_accent).radius(radius::MD).shadows(&t.elev_selected).build(cx)))
        .child(item(th, s, "nav-insights", View::Insights, &icons::CHART_COLUMN, "Insights", None, Some("mod+4")))
        .child(Nav::new("nav-settings", &icons::SETTINGS, "Settings", matches!(s.view, View::Settings(_))).shortcut("mod+,").build(th).on_press(|s: &mut MainUi| s.set_view(View::Settings(None))));

    col()
        .key("sidebar")
        .role("complementary")
        .label("Sidebar")
        .w_full()
        .h_full()
        .min_h(0.0)
        .motion(Spring::NONE)
        .child(header)
        .child(nav)
        .child(footer)
        .child(super::status::view(s, cx, layer_out))
}

/// Keeps the pill out of the group's flow (and its gaps), whether or not anything is selected:
/// a zero-size box at the group's origin, which the pill's own offsets start from.
fn pill_slot(pill: El) -> El {
    el().key("pill-slot").absolute(0.0, 0.0).size(0.0, 0.0).child(pill)
}

/// The key of the item `aria-current="page"` marks, for the selection pill.
fn active_nav_key(s: &MainUi) -> Option<String> {
    Some(match &s.view {
        View::All => "nav-all".into(),
        View::Favorites => "nav-favorites".into(),
        View::Recent => "nav-recent".into(),
        View::Disabled => "nav-disabled".into(),
        View::Group(id) => drag::group_key(*id),
        View::Insights => "nav-insights".into(),
        View::Settings(_) => "nav-settings".into(),
        View::Clipboard(ClipFilter::All) => "nav-clip-all".into(),
        View::Clipboard(f) => format!("nav-clip-{f:?}"),
    })
}
