//! Gallery page: the overlays in kit/, in every variant, size and state.
//!
//! `KUDZU_GALLERY=overlays` draws the menus, selects, tooltips, popovers, tabs, collapsibles and
//! scroll areas, all open at once. `KUDZU_OVERLAYS=dialog|alert|command|toasts` draws one modal
//! scene instead (each covers the window): the group-settings-style dialog, the clear-history
//! alert (and its `sm` size beside it), the ⌘K palette, and a toast stack (pushed by the
//! "Show toasts" button, which scripts press).

use rune::{Align, Cx, El, Side, col, el, row, text};

use crate::ui::icons;
use crate::ui::kit::alert_dialog::{AlertDialog, AlertSize};
use crate::ui::kit::app_picker::{AppPicker, AppPickerState};
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::collapsible::Collapsible;
use crate::ui::kit::command::{Command, CommandItem, CommandState, CommandStyle};
use crate::ui::kit::context_menu::ContextMenu;
use crate::ui::kit::dialog::{self, Dialog};
use crate::ui::kit::dropdown_menu::{DropdownMenu, MenuItem, MenuState, SubMenu};
use crate::ui::kit::popover::{self, Popover};
use crate::ui::kit::scroll_area;
use crate::ui::kit::select::{Select, SelectSize, SelectState};
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::tabs::{Tabs, TabsVariant};
use crate::ui::kit::toast::Toast;
use crate::ui::kit::tooltip::{self, Tooltip};
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};
use crate::ui::theme_for;

pub struct State {
    pub more: MenuState,
    pub sort: MenuState,
    pub moving: MenuState,
    pub context: MenuState,
    pub sort_by: String,
    pub show_disabled: bool,
    pub keep: SelectState,
    pub keep_value: String,
    pub keep_closed: SelectState,
    pub popover: bool,
    pub picker: AppPickerState,
    pub apps: Vec<String>,
    pub tab: String,
    pub tab_line: String,
    pub collapsed: bool,
    pub dialog: bool,
    pub alert: bool,
    pub alert_sm: bool,
    pub command: CommandState,
    pub command_open: bool,
}

impl Default for State {
    fn default() -> Self {
        let mut more = MenuState::default();
        more.open_with(None);
        debug_assert!(more.is_open());
        let mut sort = MenuState::default();
        sort.open_with(Some(1));
        let mut moving = MenuState::default();
        moving.open_with(Some(2));
        moving.show_submenu(2, true);
        let mut context = MenuState::default();
        context.open_with(None);
        context.point = Some((40.0, 470.0));
        let mut keep = SelectState::default();
        keep.open_at(Some(1));
        let scene = scene();
        let menus = scene.is_empty();
        if !menus {
            for m in [&mut more, &mut sort, &mut moving, &mut context] {
                m.close();
            }
            keep.close();
        }
        State {
            more,
            sort,
            moving,
            context,
            sort_by: "edited".into(),
            show_disabled: true,
            keep,
            keep_value: "7".into(),
            keep_closed: SelectState::default(),
            popover: menus,
            picker: AppPickerState { open: menus, ..Default::default() },
            apps: vec!["com.apple.Terminal".into(), "com.1password.1password".into()],
            tab: "general".into(),
            tab_line: "week".into(),
            collapsed: false,
            dialog: scene == "dialog",
            alert: scene == "alert",
            alert_sm: scene == "alert-sm",
            command: CommandState::default(),
            command_open: scene == "command",
        }
    }
}

fn scene() -> String {
    std::env::var("KUDZU_OVERLAYS").unwrap_or_default()
}

fn g(s: &mut MainUi) -> &mut State {
    &mut s.gallery.overlays
}

fn label(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::XS.weight_of(weight::MEDIUM)).color(th.t.muted_foreground)
}

/// A labelled cell of `h` points, its content top-left.
fn cell(th: &Theme, title: &str, w: f32, h: f32, body: El) -> El {
    col().w(w).h(h).gap(8.0).items_start().child(label(th, title)).child(body)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.gallery.overlays;
    let mut layer: Vec<El> = Vec::new();

    // Row 1: dropdown menus.
    let more = DropdownMenu::new("g-more", &st.more, |s: &mut MainUi| &mut g(s).more)
        .trigger(Button::icon_only(&icons::ELLIPSIS, "More actions").variant(Variant::Ghost).size(Size::IconSm).id("g-more-b").build(th))
        .align(Align::End)
        .width(240.0)
        .item(MenuItem::new("Copy expanded snippet").icon(&icons::CLIPBOARD))
        .item(MenuItem::new("Duplicate").icon(&icons::COPY).shortcut("⌘D"))
        .sub(SubMenu::new("Move to group").icon(&icons::FOLDER_INPUT).width(208.0).item(MenuItem::new("No group")))
        .separator()
        .item(MenuItem::new("Delete").icon(&icons::TRASH).destructive().shortcut("⌘⌫"))
        .build(cx, &mut layer);
    let sort = DropdownMenu::new("g-sort", &st.sort, |s: &mut MainUi| &mut g(s).sort)
        .trigger(Button::icon_only(&icons::ARROW_DOWN_UP, "Sort").variant(Variant::Ghost).size(Size::IconSm).id("g-sort-b").build(th))
        .width(176.0)
        .side(Side::Bottom)
        .side_offset(4.0)
        .entries(Vec::new())
        .label("Sort by")
        .item(MenuItem::radio("Best match", st.sort_by == "relevance").on_select(|s: &mut MainUi, _| g(s).sort_by = "relevance".into()))
        .item(MenuItem::radio("Recently edited", st.sort_by == "edited").on_select(|s: &mut MainUi, _| g(s).sort_by = "edited".into()))
        .item(MenuItem::radio("Most used", st.sort_by == "used").on_select(|s: &mut MainUi, _| g(s).sort_by = "used".into()))
        .item(MenuItem::radio("Name", st.sort_by == "name").on_select(|s: &mut MainUi, _| g(s).sort_by = "name".into()))
        .separator()
        .item(MenuItem::checkbox("Show turned-off", st.show_disabled).on_select(|s: &mut MainUi, _| g(s).show_disabled ^= true))
        .item(MenuItem::new("Disabled item").disabled(true))
        .label_inset("Inset label")
        .item(MenuItem::new("Inset item").inset())
        .build(cx, &mut layer);
    let moving = DropdownMenu::new("g-move", &st.moving, |s: &mut MainUi| &mut g(s).moving)
        .trigger(Button::new("Move to").variant(Variant::Outline).size(Size::Sm).icon(&icons::FOLDER_INPUT).id("g-move-b").build(th))
        .width(208.0)
        .label("Move 3 to")
        .item(MenuItem::new("No group"))
        .sub(
            SubMenu::new("Work")
                .icon(&icons::FOLDER)
                .width(208.0)
                .item(MenuItem::new("Work"))
                .item(MenuItem::new("Email").pad_left(22.0))
                .item(MenuItem::new("Calendar").pad_left(22.0)),
        )
        .item(MenuItem::new("Developer").icon(&icons::FOLDER))
        .item(MenuItem::new("Personal").icon(&icons::FOLDER))
        .build(cx, &mut layer);
    let menus = row()
        .gap(24.0)
        .child(cell(th, "Dropdown · pointer-opened, align end", 300.0, 290.0, row().w_full().justify_end().child(more)))
        .child(cell(th, "Radio, checkbox, label, disabled, inset", 260.0, 290.0, sort))
        .child(cell(th, "Submenu · opened from the keyboard", 480.0, 290.0, moving));

    // Row 2: context menu, selects.
    let ctx_area = el()
        .w(220.0)
        .h(60.0)
        .radius(radius::LG)
        .fill(t.muted)
        .center()
        .child(text("Right-click a group").font(ty::BASE).color(t.muted_foreground));
    let mut ctx_state = st.context.clone();
    if ctx_state.point.is_some()
        && let Some([x, y, w, h]) = cx.layout("g-context-area")
    {
        ctx_state.point = Some((x + w * 0.55, y + h * 0.6));
    }
    let context = ContextMenu::new("g-context", &ctx_state, |s: &mut MainUi| &mut g(s).context)
        .trigger(ctx_area)
        .open(true)
        .width(208.0)
        .label("Group")
        .item(MenuItem::new("New subgroup"))
        .item(MenuItem::new("Rename"))
        .item(MenuItem::new("Group settings…"))
        .item(MenuItem::new("Turn off"))
        .sub(
            SubMenu::new("Move to")
                .inset()
                .disabled(false)
                .label("Groups")
                .item(MenuItem::new("Top level"))
                .separator()
                .sub(SubMenu::new("Work").item(MenuItem::new("Email"))),
        )
        .separator()
        .item(MenuItem::new("Delete group").destructive())
        .item(MenuItem::new("Delete group and snippets").destructive())
        .build(cx, &mut layer);
    let retention = |sel: Select<MainUi>| {
        sel.option("1", "1 day").option("7", "1 week").option("30", "1 month").option("90", "3 months").option("365", "1 year").option("0", "Forever").option("custom", "Custom…")
    };
    let keep = retention(Select::new("g-keep", &st.keep, |s: &mut MainUi| &mut g(s).keep))
        .value(Some(&st.keep_value))
        .width(128.0)
        .label("Keep history for")
        .on_value_change(|s: &mut MainUi, v, _| g(s).keep_value = v.into())
        .build(cx, &mut layer);
    let closed = |key: &str, size: SelectSize, placeholder: bool, invalid: bool, disabled: bool, cx: &mut Cx<MainUi>, layer: &mut Vec<El>| {
        retention(Select::new(key.to_string(), &st.keep_closed, |s: &mut MainUi| &mut g(s).keep_closed))
            .value(if placeholder { None } else { Some("30") })
            .placeholder("Pick a duration")
            .size(size)
            .invalid(invalid)
            .disabled(disabled)
            .build(cx, layer)
    };
    let selects = col()
        .gap(10.0)
        .child(closed("g-sel-a", SelectSize::Default, false, false, false, cx, &mut layer))
        .child(closed("g-sel-b", SelectSize::Sm, true, false, false, cx, &mut layer))
        .child(closed("g-sel-c", SelectSize::Default, false, true, false, cx, &mut layer))
        .child(closed("g-sel-d", SelectSize::Default, false, false, true, cx, &mut layer))
        .child(
            Select::new("g-sel-e", &st.keep_closed, |s: &mut MainUi| &mut g(s).keep_closed)
                .popper()
                .align(Align::Start)
                .group_label("Recent")
                .option("1", "1 day")
                .option_disabled("2", "2 days")
                .separator()
                .option("0", "Forever")
                .value(Some("1"))
                .size(SelectSize::Sm)
                .build(cx, &mut layer),
        );
    let row2 = row()
        .gap(24.0)
        .child(cell(th, "Context menu at the pointer", 460.0, 300.0, context))
        .child(cell(th, "Select · item-aligned, open", 260.0, 300.0, el().pt(110.0).pl(20.0).child(keep)))
        .child(cell(th, "Select · default, sm + placeholder, invalid, disabled", 300.0, 300.0, selects));

    // Row 3: tooltips, popover, app picker.
    if scene().is_empty() {
        for k in ["g-tip-top", "g-tip-bottom", "g-tip-right", "g-tip-wide"] {
            tooltip::force_open(k);
        }
    }
    let tip_btn = |icon_: &'static rune::Icon, l: &str, id: &str| Button::icon_only(icon_, l.to_string()).variant(Variant::Ghost).size(Size::IconSm).id(id.to_string()).build(th);
    let tips = row()
        .gap(56.0)
        .pt(48.0)
        .child(Tooltip::new("g-tip-top").text("Favorite").shortcut("mod+shift+f").build(cx, tip_btn(&icons::STAR, "Favorite", "g-t1"), &mut layer))
        .child(Tooltip::new("g-tip-bottom").side(Side::Bottom).text("Sort").build(cx, tip_btn(&icons::ARROW_DOWN_UP, "Sort", "g-t2"), &mut layer))
        .child(Tooltip::new("g-tip-right").side(Side::Right).text("Hide sidebar").build(cx, tip_btn(&icons::PANEL_LEFT, "Sidebar", "g-t3"), &mut layer));
    let wide = Tooltip::new("g-tip-wide")
        .max_width(256.0)
        .align(Align::Center)
        .delay(std::time::Duration::from_millis(tooltip::DELAY_MS))
        .child(el())
        .text("Expansion is paused. Snippets won't expand until you resume, from here or the menu bar.")
        .build(cx, tip_btn(&icons::PAUSE, "Paused", "g-t4"), &mut layer);
    let pop = Popover::new("g-pop")
        .open(st.popover)
        .on_open_change(|s: &mut MainUi, open, _| g(s).popover = open)
        .trigger(Button::new("Popover").variant(Variant::Outline).id("g-pop-b").build(th))
        .align(Align::Start)
        .side(Side::Bottom)
        .side_offset(4.0)
        .float()
        .backdrop_blur(0.0)
        .children(Vec::new())
        .child(popover::header([popover::title(cx, "Insert a variable"), popover::description(cx, "Variables fill in when the snippet expands.")]))
        .build(cx, &mut layer);
    let picker = AppPicker::new("g-apps", &st.apps, &st.picker, |s: &mut MainUi| &mut g(s).picker)
        .add_label("Never record from an app")
        .on_change(|s: &mut MainUi, ids, _| g(s).apps = ids)
        .build(cx, &mut layer);
    let row3 = row()
        .gap(24.0)
        .child(cell(th, "Tooltips · top + keys, bottom, right", 340.0, 150.0, tips))
        .child(cell(th, "Tooltip · max-w-64", 300.0, 150.0, el().pt(90.0).pl(130.0).child(wide)))
        .child(cell(th, "Popover", 400.0, 150.0, pop));
    let row35 = cell(th, "App picker · chips, popover open", 1100.0, 300.0, picker);

    // Row 4 (below the picker's popover): tabs, collapsible, scroll area.
    let tabs = col()
        .gap(14.0)
        .child(
            Tabs::new("g-tabs")
                .value(&st.tab)
                .tab("general", "General")
                .tab("expansion", "Expansion")
                .tab("apps", "Apps")
                .tab_disabled("data", "Data")
                .on_value_change(|s: &mut MainUi, v, _| g(s).tab = v.into())
                .build(th),
        )
        .child(
            Tabs::new("g-tabs-line")
                .variant(TabsVariant::Line)
                .value(&st.tab_line)
                .tab("day", "Day")
                .tab("week", "Week")
                .tab("month", "Month")
                .on_value_change(|s: &mut MainUi, v, _| g(s).tab_line = v.into())
                .build(th),
        )
        .child(
            Tabs::new("g-tabs-v")
                .vertical()
                .value(&st.tab)
                .tab_icon("general", "General", &icons::SETTINGS)
                .tab_icon("apps", "Apps", &icons::LAYERS)
                .on_value_change(|s: &mut MainUi, v, _| g(s).tab = v.into())
                .build(th),
        );
    let coll = Collapsible::new("g-coll")
        .disabled(false)
        .open(!st.collapsed)
        .on_open_change(|s: &mut MainUi, open, _| g(s).collapsed = !open)
        .trigger(Button::new("Advanced").variant(Variant::Ghost).size(Size::Sm).icon(&icons::CHEVRON_DOWN).id("g-coll-b").build(th))
        .content(col().gap(4.0).pl(10.0).child(text("Case-sensitive triggers").font(ty::BASE).color(t.foreground)).child(text("Expand inside words").font(ty::BASE).color(t.foreground)))
        .build();
    let chips = row().gap(6.0).padding(6.0).children((1..=12).map(|i| text(format!("chip {i}")).font(ty::XS).color(t.muted_foreground)));
    let lines = col().gap(6.0).padding(10.0).children((1..=24).map(|i| text(format!("Clipboard item {i}")).font(ty::BASE).color(t.foreground)));
    let scroll = el().w(220.0).h(150.0).radius(radius::LG).border(1.0, t.border).child(scroll_area::vertical(th, "g-scroll", lines));
    let row4 = row()
        .gap(24.0)
        .child(cell(th, "Tabs · default, line, vertical", 380.0, 230.0, tabs))
        .child(cell(th, "Collapsible", 200.0, 230.0, coll))
        .child(cell(th, "Scroll area (hover to show the bar), horizontal", 240.0, 230.0, col().gap(8.0).child(scroll).child(el().w(220.0).child(scroll_area::horizontal(th, "g-hscroll", chips)))));

    // Modal scenes.
    let dlg_inset = 16.0;
    Dialog::new("g-dialog")
        .open(st.dialog)
        .on_open_change(|s: &mut MainUi, open, _| g(s).dialog = open)
        .max_width(520.0)
        .gap(20.0)
        .padding(dlg_inset)
        .radius(radius::XXL)
        .float()
        .ring(false)
        .label("Group settings")
        .backdrop_blur(0.0)
        .children(Vec::new())
        .child(dialog::header(cx, "Group settings", Some("Snippets in this group (and its subgroups) inherit these unless they set their own.")))
        .child(text("Only expand in these apps").font(ty::BASE).color(t.foreground))
        .child(dialog::footer(cx, dlg_inset, radius::XXL, [
            Button::new("Cancel").variant(Variant::Outline).id("g-dlg-cancel").on_press(|s: &mut MainUi| g(s).dialog = false).build(th),
            Button::new("Save").id("g-dlg-save").on_press(|s: &mut MainUi| g(s).dialog = false).build(th),
        ]))
        .build(cx, &mut layer);
    AlertDialog::new("g-alert")
        .open(st.alert)
        .on_open_change(|s: &mut MainUi, open, _| g(s).alert = open)
        .title("Clear clipboard history?")
        .description("Deletes everything except your 2 pinned items. This can't be undone.")
        .cancel("Cancel", |_: &mut MainUi, _| {})
        .action("Clear history", Variant::Destructive, |s: &mut MainUi, _| s.toasts.push(Toast::success("Deleted 12 items")))
        .build(cx, &mut layer);
    AlertDialog::new("g-alert-sm")
        .open(st.alert_sm)
        .size(AlertSize::Sm)
        .on_open_change(|s: &mut MainUi, open, _| g(s).alert_sm = open)
        .media(rune::icon(&icons::TRASH, 24.0).color(t.foreground))
        .title("Delete 3 snippets?")
        .description("You can undo this right after.")
        .cancel("Cancel", |_: &mut MainUi, _| {})
        .action("Delete", Variant::Destructive, |_: &mut MainUi, _| {})
        .build(cx, &mut layer);
    if scene() == "command-shadcn" {
        let items = vec![
            CommandItem::new("Date").keywords(["today", "calendar"]).icon(&icons::CALENDAR_DAYS).text("Date"),
            CommandItem::new("Clipboard").keywords(["paste"]).icon(&icons::CLIPBOARD).text("Clipboard").checked(true),
            CommandItem::new("Cursor").icon(&icons::TEXT_CURSOR).text("Cursor").checked(false),
            CommandItem::new("Form field").icon(&icons::TEXT_CURSOR_INPUT).text("Form field").disabled(true),
        ];
        let cmd = Command::new("g-cmd2", &st.command, |s: &mut MainUi| &mut g(s).command)
            .placeholder("Insert a variable or snippet…")
            .wrap(true)
            .on_escape(|s: &mut MainUi, _, _| g(s).command.reset())
            .group(Some("Variables"), items)
            .empty(text("Nothing matches.").font(ty::SM).color(t.popover_foreground))
            .build(cx);
        crate::ui::kit::command::dialog::<MainUi>("g-cmd2-dialog").open(true).child(cmd).build(cx, &mut layer);
    }
    if st.command_open {
        let palette = palette(s, cx);
        Dialog::new("g-cmdk")
            .open(true)
            .on_open_change(|s: &mut MainUi, open, _| {
                g(s).command_open = open;
                g(s).command.reset();
            })
            .close_button(false)
            .top(0.18)
            .max_width(600.0)
            .padding(0.0)
            .gap(0.0)
            .radius(radius::XXL)
            .float()
            .ring(false)
            .label("Search and commands")
            .description("Find snippets, jump anywhere, or run an action.")
            .child(palette)
            .build(cx, &mut layer);
    }

    let toasts_btn = Button::new("Show toasts")
        .variant(Variant::Outline)
        .size(Size::Sm)
        .id("g-toasts")
        .on_press(|s: &mut MainUi| {
            s.toasts.push(Toast::new("Deleted “Email signature”").duration_ms(600_000).action("Undo", |_, _| {}));
            s.toasts.push(Toast::error("Couldn't save: the disk is full.").duration_ms(600_000));
            s.toasts.push(
                Toast::new("Kudzu 1.4.0 is ready")
                    .description("It installs when you quit, or restart now.")
                    .duration_ms(600_000)
                    .action("Restart", |_, _| {})
                    .cancel("What's new", |_, _| {}),
            );
        })
        .build(th)
        .key("g-toasts-btn");
    let quick = Button::new("Toast (4 s)")
        .variant(Variant::Outline)
        .size(Size::Sm)
        .id("g-toast-quick")
        .on_press(|s: &mut MainUi| s.toasts.push(Toast::success("Copied to the clipboard")))
        .build(th)
        .key("g-toast-quick-btn");

    let page = col()
        .key("gallery-overlays")
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(18.0)
        .fill(t.background)
        .child(row().gap(12.0).center_y().child(text("Overlays").font(ty::XL.weight_of(weight::SEMIBOLD)).color(t.foreground)).child(toasts_btn).child(quick))
        .child(menus)
        .child(row2)
        .child(row3)
        .child(row35)
        .child(row4);
    // The frame mounts the gallery as its page, so its overlays ride above it here.
    el().key("gallery-overlays-root").w_full().h_full().child(page).child(el().key("gallery-layer").absolute(0.0, 0.0).w_full().h_full().children(layer))
}

/// The ⌘K palette as `command-menu.tsx` builds it, on fixture-like rows.
fn palette(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let snippet = |name: &str, path: &str, trig: &str| {
        let mut it = CommandItem::new(format!("snippet-{name}")).icon(&icons::FILE_TEXT).text(name.to_string());
        if !path.is_empty() {
            it = it.child(text(path.to_string()).font(ty::XS).color(t.faint).truncate());
        }
        it.child(el().grow(1.0).justify_end().child(trigger_chip(th, trig, false))).on_select(|s: &mut MainUi, _| g(s).command_open = false)
    };
    let action = |id: &str, i: &'static rune::Icon, l: &str, sc: Option<&str>| {
        let mut it = CommandItem::new(id.to_string()).icon(i).text(l.to_string());
        if id == "del" {
            it = it.destructive();
        }
        if let Some(sc) = sc {
            it = it.child(el().grow(1.0).justify_end().child(Shortcut::new(sc).build(th)));
        }
        it.on_select(|s: &mut MainUi, _| g(s).command_open = false)
    };
    let st = &s.gallery.overlays;
    let q = st.command.query().trim().to_lowercase();
    let keep = |l: &str| q.is_empty() || q.split_whitespace().all(|w| l.to_lowercase().contains(w));
    let snippets: Vec<_> = [("Email signature", "Work › Email", ";sig"), ("Meeting follow-up", "Work › Email", ";fu"), ("Out of office", "Work › Email", ";ooo"), ("Today", "", ";date"), ("Current time", "", ";now")]
        .into_iter()
        .filter(|(n, _, _)| keep(n))
        .map(|(n, p, tr)| snippet(n, p, tr))
        .collect();
    let selected: Vec<_> = [
        ("fav", &icons::STAR, "Unfavorite “Email signature”", Some("mod+shift+f")),
        ("en", &icons::CIRCLE_OFF, "Turn off “Email signature”", Some("mod+shift+e")),
        ("dup", &icons::COPY, "Duplicate “Email signature”", Some("mod+d")),
        ("del", &icons::TRASH, "Delete “Email signature”", Some("mod+backspace")),
    ]
    .into_iter()
    .filter(|a| keep(a.2))
    .map(|(id, i, l, sc)| action(id, i, l, sc))
    .collect();
    let actions: Vec<_> = [("new", &icons::PLUS, "New snippet", Some("mod+n")), ("newg", &icons::FOLDER_PLUS, "New group", Some("mod+shift+n")), ("pause", &icons::PAUSE, "Pause expanding", Some("mod+shift+p"))]
        .into_iter()
        .filter(|a| keep(a.2))
        .map(|(id, i, l, sc)| action(id, i, l, sc))
        .collect();
    Command::new("g-palette", &st.command, |s: &mut MainUi| &mut g(s).command)
        .style(CommandStyle::Palette)
        .filter(false)
        .wrap(true)
        .in_dialog()
        .placeholder("Search snippets, groups and actions…")
        .trailing(Shortcut::new("escape").build(th))
        .group(Some(if q.is_empty() { "Frequently used" } else { "Snippets" }), snippets)
        .group(Some("Selected snippet"), selected)
        .group(Some("Actions"), actions)
        .empty(text(format!("No results for “{}”.", st.command.query())).font(ty::SM).color(t.muted_foreground))
        .build(cx)
}
