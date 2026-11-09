// Every Lucide icon the web UI imports (apps/desktop/components, apps/desktop/app), mapped to the
// SF Symbol with the same meaning. `IconTests` checks the map covers every imported name and that
// each symbol resolves. No Lucide SVGs are bundled: every name has a sensible symbol.

import SwiftUI

extension Icon {
    /// Lucide import name → SF Symbol name (aliases such as `CheckIcon` share their base icon).
    public static let lucide: [String: String] = [
        "AlignLeft": "text.alignleft",
        "ArrowDownUp": "arrow.up.arrow.down",
        "ArrowLeft": "arrow.left",
        "ArrowRight": "arrow.right",
        "ArrowUpRight": "arrow.up.right",
        "BarChart3": "chart.bar",
        "Bold": "bold",
        "Braces": "curlybraces",
        "CalendarDays": "calendar",
        "Check": "checkmark",
        "CheckIcon": "checkmark",
        "ChevronDown": "chevron.down",
        "ChevronDownIcon": "chevron.down",
        "ChevronRight": "chevron.right",
        "ChevronRightIcon": "chevron.right",
        "ChevronUpIcon": "chevron.up",
        "CircleCheckIcon": "checkmark.circle",
        "CircleHelp": "questionmark.circle",
        "CircleOff": "circle.slash",
        "Clipboard": "clipboard",
        "ClipboardList": "list.clipboard",
        "Clock": "clock",
        "Clock3": "clock",
        "Copy": "doc.on.doc",
        "CornerDownLeft": "arrow.turn.down.left",
        "CornerDownRight": "arrow.turn.down.right",
        "Download": "square.and.arrow.down",
        "ExternalLink": "arrow.up.right.square",
        "EyeOff": "eye.slash",
        "File": "doc",
        "FileText": "doc.text",
        "Files": "doc.on.doc",
        "Folder": "folder",
        "FolderInput": "folder",
        "FolderOpen": "folder",
        "FolderPlus": "folder.badge.plus",
        "Globe": "globe",
        "History": "clock.arrow.circlepath",
        "Image": "photo",
        "ImagePlus": "photo.badge.plus",
        "InfoIcon": "info.circle",
        "Italic": "italic",
        "KeyRound": "key",
        "Keyboard": "keyboard",
        "Layers": "square.3.layers.3d",
        "Link": "link",
        "List": "list.bullet",
        "ListChecks": "checklist",
        "ListOrdered": "list.number",
        "Loader2": "circle.dotted",
        "Loader2Icon": "circle.dotted",
        "Lock": "lock",
        "Moon": "moon",
        "MoreHorizontal": "ellipsis",
        "MousePointer2": "cursorarrow",
        "OctagonXIcon": "xmark.octagon",
        "Package": "shippingbox",
        "Palette": "paintpalette",
        "PanelLeft": "sidebar.left",
        "Pause": "pause",
        "Pencil": "pencil",
        "Pin": "pin",
        "PinOff": "pin.slash",
        "Play": "play",
        "Plus": "plus",
        "RefreshCw": "arrow.clockwise",
        "RemoveFormatting": "eraser",
        "RotateCcw": "arrow.counterclockwise",
        "Search": "magnifyingglass",
        "SearchIcon": "magnifyingglass",
        "Settings": "gearshape",
        "Settings2": "switch.2",
        "ShieldAlert": "exclamationmark.shield",
        "ShieldCheck": "checkmark.shield",
        "ShieldOff": "shield.slash",
        "SlidersHorizontal": "slider.horizontal.3",
        "Sparkles": "sparkles",
        "Star": "star",
        "Sun": "sun.max",
        "TextCursor": "character.cursor.ibeam",
        "TextCursorInput": "character.textbox",
        "Trash2": "trash",
        "TriangleAlert": "exclamationmark.triangle",
        "TriangleAlertIcon": "exclamationmark.triangle",
        "Type": "textformat",
        "Underline": "underline",
        "Upload": "square.and.arrow.up",
        "X": "xmark",
        "XIcon": "xmark",
    ]

    /// Lucide `AlignLeft` → `text.alignleft`.
    public static var alignLeft: Image { Image(systemName: "text.alignleft") }
    /// Lucide `ArrowDownUp` → `arrow.up.arrow.down`.
    public static var arrowDownUp: Image { Image(systemName: "arrow.up.arrow.down") }
    /// Lucide `ArrowLeft` → `arrow.left`.
    public static var arrowLeft: Image { Image(systemName: "arrow.left") }
    /// Lucide `ArrowRight` → `arrow.right`.
    public static var arrowRight: Image { Image(systemName: "arrow.right") }
    /// Lucide `ArrowUpRight` → `arrow.up.right`.
    public static var arrowUpRight: Image { Image(systemName: "arrow.up.right") }
    /// Lucide `BarChart3` → `chart.bar`.
    public static var barChart3: Image { Image(systemName: "chart.bar") }
    /// Lucide `Bold` → `bold`.
    public static var bold: Image { Image(systemName: "bold") }
    /// Lucide `Braces` → `curlybraces`.
    public static var braces: Image { Image(systemName: "curlybraces") }
    /// Lucide `CalendarDays` → `calendar`.
    public static var calendarDays: Image { Image(systemName: "calendar") }
    /// Lucide `Check` → `checkmark`.
    public static var check: Image { Image(systemName: "checkmark") }
    /// Lucide `ChevronDown` → `chevron.down`.
    public static var chevronDown: Image { Image(systemName: "chevron.down") }
    /// Lucide `ChevronRight` → `chevron.right`.
    public static var chevronRight: Image { Image(systemName: "chevron.right") }
    /// Lucide `ChevronUpIcon` → `chevron.up`.
    public static var chevronUp: Image { Image(systemName: "chevron.up") }
    /// Lucide `CircleCheckIcon` → `checkmark.circle`.
    public static var circleCheck: Image { Image(systemName: "checkmark.circle") }
    /// Lucide `CircleHelp` → `questionmark.circle`.
    public static var circleHelp: Image { Image(systemName: "questionmark.circle") }
    /// Lucide `CircleOff` → `circle.slash`.
    public static var circleOff: Image { Image(systemName: "circle.slash") }
    /// Lucide `Clipboard` → `clipboard`.
    public static var clipboard: Image { Image(systemName: "clipboard") }
    /// Lucide `ClipboardList` → `list.clipboard`.
    public static var clipboardList: Image { Image(systemName: "list.clipboard") }
    /// Lucide `Clock` → `clock`.
    public static var clock: Image { Image(systemName: "clock") }
    /// Lucide `Clock3` → `clock`.
    public static var clock3: Image { Image(systemName: "clock") }
    /// Lucide `Copy` → `doc.on.doc`.
    public static var copy: Image { Image(systemName: "doc.on.doc") }
    /// Lucide `CornerDownLeft` → `arrow.turn.down.left`.
    public static var cornerDownLeft: Image { Image(systemName: "arrow.turn.down.left") }
    /// Lucide `CornerDownRight` → `arrow.turn.down.right`.
    public static var cornerDownRight: Image { Image(systemName: "arrow.turn.down.right") }
    /// Lucide `Download` → `square.and.arrow.down`.
    public static var download: Image { Image(systemName: "square.and.arrow.down") }
    /// Lucide `ExternalLink` → `arrow.up.right.square`.
    public static var externalLink: Image { Image(systemName: "arrow.up.right.square") }
    /// Lucide `EyeOff` → `eye.slash`.
    public static var eyeOff: Image { Image(systemName: "eye.slash") }
    /// Lucide `File` → `doc`.
    public static var file: Image { Image(systemName: "doc") }
    /// Lucide `Files` → `doc.on.doc`.
    public static var files: Image { Image(systemName: "doc.on.doc") }
    /// Lucide `FileText` → `doc.text`.
    public static var fileText: Image { Image(systemName: "doc.text") }
    /// Lucide `Folder` → `folder`.
    public static var folder: Image { Image(systemName: "folder") }
    /// Lucide `FolderInput` → `folder`.
    public static var folderInput: Image { Image(systemName: "folder") }
    /// Lucide `FolderOpen` → `folder`.
    public static var folderOpen: Image { Image(systemName: "folder") }
    /// Lucide `FolderPlus` → `folder.badge.plus`.
    public static var folderPlus: Image { Image(systemName: "folder.badge.plus") }
    /// Lucide `Globe` → `globe`.
    public static var globe: Image { Image(systemName: "globe") }
    /// Lucide `History` → `clock.arrow.circlepath`.
    public static var history: Image { Image(systemName: "clock.arrow.circlepath") }
    /// Lucide `Image` → `photo`.
    public static var image: Image { Image(systemName: "photo") }
    /// Lucide `ImagePlus` → `photo.badge.plus`.
    public static var imagePlus: Image { Image(systemName: "photo.badge.plus") }
    /// Lucide `InfoIcon` → `info.circle`.
    public static var info: Image { Image(systemName: "info.circle") }
    /// Lucide `Italic` → `italic`.
    public static var italic: Image { Image(systemName: "italic") }
    /// Lucide `Keyboard` → `keyboard`.
    public static var keyboard: Image { Image(systemName: "keyboard") }
    /// Lucide `KeyRound` → `key`.
    public static var keyRound: Image { Image(systemName: "key") }
    /// Lucide `Layers` → `square.3.layers.3d`.
    public static var layers: Image { Image(systemName: "square.3.layers.3d") }
    /// Lucide `Link` → `link`.
    public static var link: Image { Image(systemName: "link") }
    /// Lucide `List` → `list.bullet`.
    public static var list: Image { Image(systemName: "list.bullet") }
    /// Lucide `ListChecks` → `checklist`.
    public static var listChecks: Image { Image(systemName: "checklist") }
    /// Lucide `ListOrdered` → `list.number`.
    public static var listOrdered: Image { Image(systemName: "list.number") }
    /// Lucide `Loader2` → `circle.dotted`.
    public static var loader2: Image { Image(systemName: "circle.dotted") }
    /// Lucide `Lock` → `lock`.
    public static var lock: Image { Image(systemName: "lock") }
    /// Lucide `Moon` → `moon`.
    public static var moon: Image { Image(systemName: "moon") }
    /// Lucide `MoreHorizontal` → `ellipsis`.
    public static var moreHorizontal: Image { Image(systemName: "ellipsis") }
    /// Lucide `MousePointer2` → `cursorarrow`.
    public static var mousePointer2: Image { Image(systemName: "cursorarrow") }
    /// Lucide `OctagonXIcon` → `xmark.octagon`.
    public static var octagonX: Image { Image(systemName: "xmark.octagon") }
    /// Lucide `Package` → `shippingbox`.
    public static var package: Image { Image(systemName: "shippingbox") }
    /// Lucide `Palette` → `paintpalette`.
    public static var palette: Image { Image(systemName: "paintpalette") }
    /// Lucide `PanelLeft` → `sidebar.left`.
    public static var panelLeft: Image { Image(systemName: "sidebar.left") }
    /// Lucide `Pause` → `pause`.
    public static var pause: Image { Image(systemName: "pause") }
    /// Lucide `Pencil` → `pencil`.
    public static var pencil: Image { Image(systemName: "pencil") }
    /// Lucide `Pin` → `pin`.
    public static var pin: Image { Image(systemName: "pin") }
    /// Lucide `PinOff` → `pin.slash`.
    public static var pinOff: Image { Image(systemName: "pin.slash") }
    /// Lucide `Play` → `play`.
    public static var play: Image { Image(systemName: "play") }
    /// Lucide `Plus` → `plus`.
    public static var plus: Image { Image(systemName: "plus") }
    /// Lucide `RefreshCw` → `arrow.clockwise`.
    public static var refreshCw: Image { Image(systemName: "arrow.clockwise") }
    /// Lucide `RemoveFormatting` → `eraser`.
    public static var removeFormatting: Image { Image(systemName: "eraser") }
    /// Lucide `RotateCcw` → `arrow.counterclockwise`.
    public static var rotateCcw: Image { Image(systemName: "arrow.counterclockwise") }
    /// Lucide `Search` → `magnifyingglass`.
    public static var search: Image { Image(systemName: "magnifyingglass") }
    /// Lucide `Settings` → `gearshape`.
    public static var settings: Image { Image(systemName: "gearshape") }
    /// Lucide `Settings2` → `switch.2`.
    public static var settings2: Image { Image(systemName: "switch.2") }
    /// Lucide `ShieldAlert` → `exclamationmark.shield`.
    public static var shieldAlert: Image { Image(systemName: "exclamationmark.shield") }
    /// Lucide `ShieldCheck` → `checkmark.shield`.
    public static var shieldCheck: Image { Image(systemName: "checkmark.shield") }
    /// Lucide `ShieldOff` → `shield.slash`.
    public static var shieldOff: Image { Image(systemName: "shield.slash") }
    /// Lucide `SlidersHorizontal` → `slider.horizontal.3`.
    public static var slidersHorizontal: Image { Image(systemName: "slider.horizontal.3") }
    /// Lucide `Sparkles` → `sparkles`.
    public static var sparkles: Image { Image(systemName: "sparkles") }
    /// Lucide `Star` → `star`.
    public static var star: Image { Image(systemName: "star") }
    /// Lucide `Sun` → `sun.max`.
    public static var sun: Image { Image(systemName: "sun.max") }
    /// Lucide `TextCursor` → `character.cursor.ibeam`.
    public static var textCursor: Image { Image(systemName: "character.cursor.ibeam") }
    /// Lucide `TextCursorInput` → `character.textbox`.
    public static var textCursorInput: Image { Image(systemName: "character.textbox") }
    /// Lucide `Trash2` → `trash`.
    public static var trash2: Image { Image(systemName: "trash") }
    /// Lucide `TriangleAlert` → `exclamationmark.triangle`.
    public static var triangleAlert: Image { Image(systemName: "exclamationmark.triangle") }
    /// Lucide `Type` → `textformat`.
    public static var type: Image { Image(systemName: "textformat") }
    /// Lucide `Underline` → `underline`.
    public static var underline: Image { Image(systemName: "underline") }
    /// Lucide `Upload` → `square.and.arrow.up`.
    public static var upload: Image { Image(systemName: "square.and.arrow.up") }
    /// Lucide `X` → `xmark`.
    public static var x: Image { Image(systemName: "xmark") }
    /// The filled star (`<Star className="fill-current" />`, a favourite).
    public static var starFill: Image { Image(systemName: "star.fill") }
}
