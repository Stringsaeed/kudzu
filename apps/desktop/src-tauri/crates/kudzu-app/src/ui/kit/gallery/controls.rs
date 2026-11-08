//! Gallery page: the controls in kit/, in every variant, size and state, laid out like a
//! component sheet. `KUDZU_GALLERY=controls` draws it in the main window; the theme follows the
//! app's appearance, so snap it once per theme.
//!
//! Columns of a state row read: rest, hover, pressed, focused (keyboard), disabled, invalid.

use kudzu_core::formatting::{FormatSpan, FormatStyle};
use kudzu_core::template::RenderResult;
use rune::{col, row, text, Cx, El, TextEdit};

use crate::ui::kit::app_icon::app_icon;
use crate::ui::kit::badge::{Badge, BadgeVariant};
use crate::ui::kit::breadcrumbs::{Breadcrumbs, Entry};
use crate::ui::kit::button::{Button, Size, State as Look, Variant};
use crate::ui::kit::checkbox::Checkbox;
use crate::ui::kit::formatted_text::FormattedText;
use crate::ui::kit::input::Input;
use crate::ui::kit::input_group::{addon_icon, group_button, AddonKind, Align, GroupButtonSize, InputGroup};
use crate::ui::kit::item::{item_actions, item_content, item_description, item_media, item_title, Item, ItemSize, ItemVariant, MediaVariant};
use crate::ui::kit::kbd::{Kbd, KbdContext};
use crate::ui::kit::label::Label;
use crate::ui::kit::radio_group::{radio_group, RadioItem};
use crate::ui::kit::segmented::{Segmented, SegmentedSize};
use crate::ui::kit::separator::{horizontal, vertical};
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, ShortcutSize, Tone};
use crate::ui::kit::slider::Slider;
use crate::ui::kit::switch::{Switch, SwitchSize};
use crate::ui::kit::textarea::Textarea;
use crate::ui::kit::toggle::{Toggle, ToggleSize, ToggleVariant};
use crate::ui::kit::toggle_group::ToggleGroup;
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::kit::wordmark::{wordmark, WordmarkVariant};
use crate::ui::main::MainUi;
use crate::ui::theme::{text as ty, Theme};
use crate::ui::{icons, theme_for};

/// The gallery's own state: fields, choices and toggles the sheet's live controls change.
pub struct State {
    pub name: TextEdit,
    pub empty: TextEdit,
    pub note: TextEdit,
    pub search: TextEdit,
    pub switch_on: bool,
    pub checked: bool,
    pub radio: usize,
    pub theme: String,
    pub size: String,
    pub volume: f32,
    pub bold: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            name: TextEdit::new("Ada Lovelace"),
            empty: TextEdit::new(""),
            note: TextEdit::new("Thanks for reaching out — I'll get back to you by Friday."),
            search: TextEdit::new(""),
            switch_on: true,
            checked: true,
            radio: 0,
            theme: "light".into(),
            size: "md".into(),
            volume: 40.0,
            bold: true,
        }
    }
}

fn g(s: &mut MainUi) -> &mut State {
    &mut s.gallery.controls
}

fn caption(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::XS).color(th.t.muted_foreground)
}

fn section(th: &Theme, title: &str, body: El) -> El {
    col().gap(8.0).child(caption(th, title)).child(body)
}

fn line(items: impl IntoIterator<Item = El>) -> El {
    row().gap(12.0).center_y().children(items)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.gallery.controls;
    let looks = [Look::Rest, Look::Hover, Look::Pressed, Look::Focus];

    // Buttons: every variant in every state.
    let variants = [Variant::Default, Variant::Outline, Variant::Secondary, Variant::Ghost, Variant::Destructive, Variant::Link];
    let mut buttons = col().gap(10.0);
    for v in variants {
        let mut r = line([]);
        for (i, l) in looks.iter().enumerate() {
            r = r.child(Button::new("Button").variant(v).state(*l).id(format!("b{v:?}{i}")).build(th));
        }
        r = r.child(Button::new("Button").variant(v).disabled(true).build(th));
        r = r.child(Button::new("Button").variant(v).invalid(true).id(format!("i{v:?}")).build(th));
        r = r.child(Button::icon_only(&icons::PLUS, "Add").variant(v).id(format!("ic{v:?}")).build(th));
        r = r.child(Button::new("Expanded").variant(v).expanded(true).haspopup().id(format!("e{v:?}")).build(th));
        buttons = buttons.child(r);
    }
    let mut sizes = line([]);
    for sz in [Size::Xs, Size::Sm, Size::Default, Size::Lg] {
        sizes = sizes.child(Button::new("New").icon(&icons::PLUS).size(sz).id(format!("s{sz:?}")).build(th));
    }
    for sz in [Size::IconXs, Size::IconSm, Size::Icon, Size::IconLg] {
        sizes = sizes.child(Button::icon_only(&icons::PLUS, "Add").variant(Variant::Outline).size(sz).id(format!("s{sz:?}")).build(th));
    }
    sizes = sizes
        .child(Button::new("Inline").icon_start(&icons::PLUS).icon_end(&icons::CHEVRON_DOWN).variant(Variant::Outline).id("inline").build(th))
        .child(Button::new("New snippet").icon(&icons::PLUS).shortcut("mod+n").id("sc").build(th))
        .child(Button::new("Search").icon(&icons::SEARCH).variant(Variant::Outline).shortcut("mod+k").id("sc2").build(th));

    // Toggles and toggle groups.
    let mut toggles = line([]);
    for v in [ToggleVariant::Default, ToggleVariant::Outline] {
        for (i, l) in looks.iter().enumerate() {
            toggles = toggles.child(Toggle::icon_only(&icons::LETTER_B, "Bold", false).variant(v).state(*l).id(format!("t{v:?}{i}")).build(th));
        }
        toggles = toggles
            .child(Toggle::icon_only(&icons::LETTER_B, "Bold", true).variant(v).id(format!("ton{v:?}")).build(th))
            .child(Toggle::icon_only(&icons::LETTER_B, "Bold", false).variant(v).disabled(true).build(th))
            .child(vertical(th).h(24.0));
    }
    toggles = toggles
        .child(Toggle::new("Italic", true).icon(&icons::ITALIC, true).size(ToggleSize::Sm).id("tsm").build(th))
        .child(Toggle::new("Large", st.bold).size(ToggleSize::Lg).id("tlg").on_toggle(|s: &mut MainUi| g(s).bold ^= true).build(th));
    let groups = line([
        ToggleGroup::single("Alignment")
            .item(Toggle::icon_only(&icons::LETTER_B, "Bold", true).id("g1"))
            .item(Toggle::icon_only(&icons::ITALIC, "Italic", false).id("g2"))
            .item(Toggle::icon_only(&icons::UNDERLINE, "Underline", false).id("g3"))
            .build(th),
        ToggleGroup::multiple("Format")
            .variant(ToggleVariant::Outline)
            .spacing(0.0)
            .item(Toggle::icon_only(&icons::LETTER_B, "Bold", true).id("h1"))
            .item(Toggle::icon_only(&icons::ITALIC, "Italic", false).id("h2"))
            .item(Toggle::icon_only(&icons::UNDERLINE, "Underline", false).id("h3"))
            .build(th),
        ToggleGroup::single("Size")
            .size(ToggleSize::Sm)
            .item(Toggle::new("Small", false).id("k1"))
            .item(Toggle::new("Medium", true).id("k2"))
            .item(Toggle::new("Large", false).id("k3"))
            .build(th),
    ]);

    // Fields.
    let field = |e: El| el_w(e, 200.0);
    let inputs = line([
        field(Input::new(&st.empty).placeholder("Placeholder").on_edit(|s: &mut MainUi| &mut g(s).empty).build(th)),
        field(Input::new(&st.name).on_edit(|s: &mut MainUi| &mut g(s).name).build(th)),
        field(Input::new(&st.name).state(Look::Focus).build(th)),
        field(Input::new(&st.name).disabled(true).build(th)),
        field(Input::new(&st.name).invalid(true).build(th)),
    ]);
    let groups_fields = line([
        el_w(
            InputGroup::input(Input::new(&st.search).placeholder("Search…").on_edit(|s: &mut MainUi| &mut g(s).search))
                .start(addon_icon(th, &icons::SEARCH))
                .addon(Align::InlineEnd, AddonKind::Kbd, Shortcut::new("mod+k").build(th))
                .build(th),
            240.0,
        ),
        el_w(
            InputGroup::input(Input::new(&st.name).state(Look::Focus))
                .start(addon_icon(th, &icons::SEARCH))
                .addon(Align::InlineEnd, AddonKind::Button, group_button("Clear", GroupButtonSize::Xs).id("gclear").build(th))
                .build(th),
            240.0,
        ),
        el_w(Textarea::new(&st.note).on_edit(|s: &mut MainUi| &mut g(s).note).build(th), 300.0),
        el_w(Textarea::new(&st.empty).placeholder("Describe it…").state(Look::Focus).build(th), 200.0),
    ]);

    // Switches, checkboxes, radios, sliders.
    let mut switches = line([]);
    for (sz, n) in [(SwitchSize::Default, "d"), (SwitchSize::Sm, "s")] {
        switches = switches
            .child(Switch::new(false).size(sz).id(format!("sw{n}0")).build(th))
            .child(Switch::new(true).size(sz).id(format!("sw{n}1")).build(th))
            .child(Switch::new(false).size(sz).state(Look::Pressed).id(format!("sw{n}2")).build(th))
            .child(Switch::new(true).size(sz).state(Look::Pressed).id(format!("sw{n}3")).build(th))
            .child(Switch::new(true).size(sz).state(Look::Focus).id(format!("sw{n}4")).build(th))
            .child(Switch::new(true).size(sz).disabled(true).build(th))
            .child(Switch::new(false).size(sz).invalid(true).id(format!("sw{n}5")).build(th));
    }
    switches = switches.child(Label::new("Live").child(Switch::new(st.switch_on).id("swlive").label("Live").on_toggle(|s: &mut MainUi| g(s).switch_on ^= true).build(th)).build(th));
    let checks = line([
        Checkbox::new(false).id("c0").build(th),
        Checkbox::new(true).id("c1").build(th),
        Checkbox::new(false).state(Look::Focus).id("c2").build(th),
        Checkbox::new(true).state(Look::Focus).id("c3").build(th),
        Checkbox::new(true).disabled(true).build(th),
        Checkbox::new(false).invalid(true).id("c4").build(th),
        Label::new("Remember").child(Checkbox::new(st.checked).id("clive").label("Remember").on_toggle(|s: &mut MainUi| g(s).checked ^= true).build(th)).build(th),
        vertical(th).h(24.0),
        RadioItem::new(false).id("r0").build(th),
        RadioItem::new(true).id("r1").build(th),
        RadioItem::new(false).state(Look::Focus).id("r2").build(th),
        RadioItem::new(true).disabled(true).build(th),
        RadioItem::new(false).invalid(true).id("r3").build(th),
        el_w(
            radio_group(
                "Insertion",
                ["Automatic", "Paste"].into_iter().enumerate().map(|(i, l)| {
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(RadioItem::new(st.radio == i).id(format!("rl{i}")).label(l).on_select(move |s: &mut MainUi| g(s).radio = i).build(th))
                        .child(Label::new(l).build(th))
                }),
            ),
            120.0,
        ),
    ]);
    let sliders = line([
        el_w(Slider::new(st.volume).id("sl0").label("Volume").on_change(|s: &mut MainUi, v| g(s).volume = v).build(th, cx), 160.0),
        el_w(Slider::new(70.0).id("sl1").state(Look::Hover).build(th, cx), 160.0),
        el_w(Slider::new(20.0).id("sl2").state(Look::Focus).build(th, cx), 160.0),
        el_w(Slider::new(55.0).id("sl3").disabled(true).build(th, cx), 160.0),
    ]);

    // Badges, keycaps, chips.
    let mut badges = line([]);
    for v in [BadgeVariant::Default, BadgeVariant::Secondary, BadgeVariant::Destructive, BadgeVariant::Outline, BadgeVariant::Ghost, BadgeVariant::Link] {
        badges = badges.child(Badge::new(format!("{v:?}")).variant(v).build(th));
    }
    badges = badges
        .child(Badge::new("Ghost").variant(BadgeVariant::Ghost).state(Look::Hover).build(th))
        .child(Badge::new("Icon").icon(&icons::CHECK, true).variant(BadgeVariant::Secondary).build(th))
        .child(Badge::new("Dev").build(th))
        .child(vertical(th).h(20.0))
        .child(trigger_chip(th, ";sig", false))
        .child(trigger_chip(th, ";addr home", true))
        .child(text("Label").font(ty::XS).color(t.muted_foreground))
        .child(Label::new("Label").build(th));
    let keys = line([
        Kbd::new("⌘").build(th),
        Kbd::new("K").build(th),
        Shortcut::new("mod+shift+k").build(th),
        Shortcut::new("enter").size(ShortcutSize::Md).build(th),
        Shortcut::new("escape").tone(Tone::Ghost).build(th),
        Shortcut::keys(accelerator_labels("Control+Alt+Space")).tone(Tone::Accent).build(th),
        row().px(8.0).h(28.0).center_y().radius(8.0).fill(t.iris).child(Shortcut::new("enter").tone(Tone::OnAccent).color(t.iris_contrast).build(th)),
        row().px(8.0).h(28.0).center_y().gap(6.0).radius(6.0).fill(t.foreground).child(text("Favorite").font(ty::XS).color(t.background)).child(Kbd::new("F").context(KbdContext::Tooltip).height(18.0).build(th)),
    ]);

    // Segmented controls (the pill glides between options).
    let seg = line([
        Segmented::new("seg-theme", "Appearance")
            .option("dark", "Dark")
            .option("light", "Light")
            .option("system", "System")
            .selected(st.theme.clone())
            .on_change(|s: &mut MainUi, v: &str| g(s).theme = v.to_string())
            .build(th, cx),
        Segmented::new("seg-hover", "Hovered").option("a", "Automatic").option("p", "Paste").option("t", "Type").selected("a").hover("p").build(th, cx),
        Segmented::new("seg-size", "Size")
            .size(SegmentedSize::Sm)
            .option("sm", "Small")
            .option("md", "Medium")
            .option("lg", "Large")
            .selected(st.size.clone())
            .on_change(|s: &mut MainUi, v: &str| g(s).size = v.to_string())
            .build(th, cx),
    ]);

    // Items, breadcrumbs, brand, rich text.
    let items = row().gap(12.0).items_start().children([
        el_w(
            Item::new()
                .variant(ItemVariant::Outline)
                .size(ItemSize::Sm)
                .child(item_media(th, MediaVariant::Icon, ItemSize::Sm, true, rune::icon(&icons::SHIELD_CHECK, 16.0).color(t.success)))
                .child(item_content(ItemSize::Sm, [item_title(th, "Accessibility"), item_description(th, ItemSize::Sm, "Insert snippets into other apps.")]))
                .child(item_actions([Badge::new("Granted").variant(BadgeVariant::Secondary).build(th)]))
                .build(th),
            300.0,
        ),
        el_w(Item::new().variant(ItemVariant::Muted).child(item_content(ItemSize::Default, [item_title(th, "Muted item"), item_description(th, ItemSize::Default, "With a description.")])).build(th), 220.0),
        el_w(
            Item::new()
                .state(Look::Hover)
                .size(ItemSize::Xs)
                .child(item_media(th, MediaVariant::Image, ItemSize::Xs, false, app_icon(th, Some("/System/Applications/Calendar.app"), "Calendar", 24.0)))
                .child(item_content(ItemSize::Xs, [item_title(th, "Calendar (hovered link)")]))
                .on_press(|_: &mut MainUi| {})
                .build(th),
            240.0,
        ),
    ]);
    let crumbs = line([
        Breadcrumbs::new(vec![Entry::new("Settings").on_select(|_: &mut MainUi| {}), Entry::new("General")]).build(th),
        vertical(th).h(20.0),
        el_w(
            Breadcrumbs::new(vec![
                Entry::new("Work").on_select(|_: &mut MainUi| {}),
                Entry::new("Clients").on_select(|_: &mut MainUi| {}),
                Entry::new("Acme").on_select(|_: &mut MainUi| {}),
                Entry::new("Email").on_select(|_: &mut MainUi| {}),
                Entry::new("Replies"),
            ])
            .on_more(|_: &mut MainUi| {})
            .build(th),
            320.0,
        ),
        vertical(th).h(20.0),
        app_icon(th, None, "Notes", 18.0),
        app_icon(th, Some("/System/Applications/Notes.app"), "Notes", 22.0),
    ]);
    let brand = line([
        wordmark(WordmarkVariant::Full, 224.0, t.foreground),
        wordmark(WordmarkVariant::Small, 144.0, t.foreground),
        wordmark(WordmarkVariant::Small, 76.0, t.muted_foreground),
        el_w(FormattedText::new(&sample()).font(ty::XS3.lh(22.0)).build(th), 330.0),
    ]);

    col()
        .key("gallery-controls")
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(18.0)
        .fill(t.background)
        .child(section(th, "Button — variants: rest, hover, pressed, focus, disabled, invalid, icon, expanded", buttons))
        .child(section(th, "Button — sizes, icon sizes, inline icons, shortcuts", sizes))
        .child(section(th, "Toggle — default and outline: rest, hover, pressed, focus, on, disabled · sm, lg · ToggleGroup", col().gap(10.0).child(toggles).child(groups)))
        .child(section(th, "Input — placeholder, value, focus, disabled, invalid · InputGroup · Textarea", col().gap(10.0).child(inputs).child(groups_fields)))
        .child(section(th, "Switch (default, sm): off, on, pressed off/on, focus, disabled, invalid · Checkbox · Radio · Slider", col().gap(12.0).child(switches).child(checks).child(sliders)))
        .child(section(th, "Badge · TriggerChip · Label · Kbd · Shortcut tones", col().gap(10.0).child(badges).child(keys)))
        .child(section(th, "Segmented — md, hovered option, sm", seg))
        .child(section(th, "Item · Breadcrumbs · AppIcon", col().gap(10.0).child(items).child(crumbs)))
        .child(horizontal(th))
        .child(section(th, "Wordmark · FormattedText", brand))
}

fn el_w(e: El, w: f32) -> El {
    rune::el().w(w).child(e)
}

fn sample() -> RenderResult {
    let text = "Hi Ada, thanks for your order. Track it here and reply with questions.".to_string();
    let span = |a: usize, b: usize, style: FormatStyle| FormatSpan { start: a, end: b, style };
    RenderResult {
        formatting: Some(vec![
            span(3, 6, FormatStyle::Bold),
            span(8, 14, FormatStyle::Italic),
            span(40, 44, FormatStyle::Link { url: "https://kudzu.site".into() }),
            span(49, 54, FormatStyle::Underline),
        ]),
        cursor_offset: Some(text.len()),
        text,
        diagnostics: Vec::new(),
    }
}
