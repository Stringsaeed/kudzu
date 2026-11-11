//! `popover`, `tooltip`, `dialog`, `alert-dialog`, `dropdown-menu`, `context-menu`, `command`,
//! `sonner`: rune-kit's overlays with Kudzu's `Kit`, forced open. Each scene shows one overlay,
//! as the app never stacks two.

use rune::{col, row, text, El, TextEdit};
use rune_kit::{
    Menu, MenuItem, Placement, Popover, PopupItem, PopupList, Toast, Tooltip, TooltipAppearance,
};

use super::line;
use crate::gallery::GalleryState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::{
    button::{Button, Size, Variant},
    dialog::Dialog,
    input::Input,
    menu, toast,
};

/// An overlay finds its trigger by the key rune-kit gave it, so the sample key goes on a
/// wrapper rather than replacing it.
fn keep_anchor(trigger: El, key: &str) -> El {
    col().key(key).child(trigger)
}

fn trigger(theme: &Theme, label: &str) -> El {
    Button::new(label)
        .variant(Variant::Outline)
        .size(Size::Sm)
        .expanded(true)
        .build(theme)
}

/// The chip configuration popovers' shape: a title, a line of help, a field.
pub fn popover(theme: &Theme, layer: &mut Vec<El>) -> El {
    let k = theme.kit();
    let t = Popover::new("chip-popover")
        .trigger(trigger(theme, "Date · Long date"))
        .title("Date")
        .description("Inserts today's date when the snippet expands.")
        .child(
            Input::new(&TextEdit::new("MMMM d, y"))
                .with(|t| t.label("Format").width(None))
                .build(theme),
        )
        .placement(Placement::BottomStart)
        .open(true)
        .on_close(|_: &mut GalleryState| {})
        .build(k, layer);
    col().child(line(
        theme,
        "Open below its trigger",
        [keep_anchor(t, "info:popover/trigger")],
    ))
}

pub fn tooltip(theme: &Theme, layer: &mut Vec<El>) -> El {
    let k = theme.kit();
    let t = Tooltip::new("tip")
        .trigger(
            Button::new("Pause expansion")
                .variant(Variant::Ghost)
                .size(Size::IconSm)
                .icon(&icons::PAUSE)
                .build(theme),
        )
        // Standard is the dark-on-light look of shadcn's `bg-foreground text-background`.
        .appearance(TooltipAppearance::Standard)
        .text(k, "Pause expansion ⌘⇧P")
        .placement(Placement::Bottom)
        .open(true)
        .build(k, layer);
    col().child(line(
        theme,
        "Open below an icon button",
        [keep_anchor(t, "info:tooltip/trigger")],
    ))
}

fn footer(theme: &Theme, confirm: &str, variant: Variant) -> El {
    row()
        .w_full()
        .gap(8.0)
        .justify_end()
        .child(Button::new("Cancel").variant(Variant::Outline).build(theme))
        .child(Button::new(confirm).variant(variant).build(theme))
}

/// The group settings dialog's shape: a title, a field, a footer. Kudzu's own dialogs are
/// `rounded-2xl` (18) on `shadow-float`; `kit_ext::dialog` (D6, after S06).
pub fn dialog(theme: &Theme, layer: &mut Vec<El>) -> El {
    Dialog::new("group", "Group settings")
        .description("Applies to every snippet in the group.")
        .child(
            Input::new(&TextEdit::new("Work"))
                .with(|t| t.label("Group name").width(None))
                .build(theme),
        )
        .child(footer(theme, "Save", Variant::Default))
        .width(520.0)
        .kudzu()
        .sample("spec:dialog/kudzu:520x_")
        .open(true)
        .on_close(|_: &mut GalleryState| {})
        .build(theme, layer);
    col().child(
        text("Open over a scrim")
            .font(theme.ty.body)
            .color(theme.c.muted_foreground),
    )
}

/// Clear clipboard history: an alert dialog is the same `Dialog` with a destructive action,
/// shadcn's `rounded-xl` (14) and `sm:max-w-sm` (384).
pub fn alert_dialog(theme: &Theme, layer: &mut Vec<El>) -> El {
    Dialog::new("clear", "Clear clipboard history?")
        .description("This deletes 42 items. Pinned items stay.")
        .child(footer(theme, "Clear history", Variant::Destructive))
        .sample("spec:alert-dialog:384x_")
        .open(true)
        .click_outside(false)
        .on_close(|_: &mut GalleryState| {})
        .build(theme, layer);
    col().child(
        text("Open over a scrim")
            .font(theme.ty.body)
            .color(theme.c.muted_foreground),
    )
}

fn snippet_actions() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Duplicate").icon(&icons::COPY).shortcut("⌘D"),
        MenuItem::new("Move to group").icon(&icons::FOLDER_INPUT),
        MenuItem::new("Pinned").icon(&icons::PIN).checked(true),
    ]
}

/// The snippet editor's "More actions" menu.
pub fn dropdown_menu(theme: &Theme, layer: &mut Vec<El>) -> El {
    let mut m = Menu::new("more").trigger(
        Button::new("More actions")
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .icon(&icons::MORE_HORIZONTAL)
            .expanded(true)
            .build(theme),
    );
    m = m.group_label("Snippet");
    for item in snippet_actions() {
        m = m.item(item);
    }
    let m = m
        .separator()
        .item(MenuItem::new("Delete").icon(&icons::TRASH_2).danger(true))
        .placement(Placement::BottomEnd)
        .open(true)
        .on_close(|_: &mut GalleryState| {});
    let t = menu::build(m, theme, layer);
    col().items_end().w(420.0).child(line(
        theme,
        "Open from its trigger",
        [keep_anchor(t, "info:dropdown-menu/trigger")],
    ))
}

/// The sidebar group's context menu, opened at the pointer.
pub fn context_menu(theme: &Theme, layer: &mut Vec<El>) -> El {
    let m = Menu::new("group-menu")
        .trigger(
            text("Right-click a group")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        )
        .item(MenuItem::new("New snippet").icon(&icons::PLUS))
        .item(MenuItem::new("Group settings…").icon(&icons::SETTINGS))
        .item(MenuItem::new("Turn off group").icon(&icons::CIRCLE_OFF))
        .separator()
        .item(
            MenuItem::new("Delete group")
                .icon(&icons::TRASH_2)
                .danger(true),
        )
        .at(120.0, 140.0)
        .open(true)
        .on_close(|_: &mut GalleryState| {});
    let t = menu::build(m, theme, layer);
    col().child(keep_anchor(t, "info:context-menu/trigger"))
}

/// shadcn's `Command`: a filter field over a list (the app picker, the chip snippet picker).
/// rune-kit's `PopupList` under an input group, as the slash menu will use it too.
pub fn command(theme: &Theme, layer: &mut Vec<El>) -> El {
    let k = theme.kit();
    let field = Input::new(&TextEdit::new(""))
        .leading_icon(&icons::SEARCH)
        .with(|t| t.placeholder("Search apps…").width(Some(320.0)))
        .build(theme)
        .key("command-field");
    PopupList::<GalleryState>::new("apps", "command-field")
        .items([
            PopupItem::new("safari", "Safari").description("com.apple.Safari"),
            PopupItem::new("mail", "Mail").description("com.apple.mail"),
            PopupItem::new("notes", "Notes").description("com.apple.Notes"),
            PopupItem::new("terminal", "Terminal").description("com.apple.Terminal"),
        ])
        .active(Some(1))
        .width(320.0)
        .open(true)
        .build(k, layer);
    // The list anchors to the field's own key, so the sample key goes on a wrapper.
    let field = col().key("spec:command/field:320x32").child(field);
    col().child(line(theme, "Input group over a popup list", [field]))
}

/// shadcn's `sonner`: rune-kit's `Toast` (the undo toast after a delete), with Sonner's 14
/// point corners (`kit_ext::toast`, D6 after S06).
pub fn sonner(theme: &Theme) -> El {
    let k = theme.kit();
    let t = Toast::new("deleted", "Snippet deleted")
        .description("“Email signature” moved to the trash.")
        .action(k, "Undo", true, |_: &mut GalleryState, _| {});
    let toast = toast::build(t, theme);
    col().child(line(
        theme,
        "Toast with an action",
        [toast.key("spec:sonner/toast:356x_")],
    ))
}
