// Icons: the Lucide → SF Symbol map covers every Lucide name imported under apps/desktop/components
// and apps/desktop/app (list below from `grep 'from "lucide-react"'`), and every symbol resolves.

import AppKit
import Testing
@testable import KudzuDesign

@Suite struct IconTests {
    static let imported: [String] = ["AlignLeft", "ArrowDownUp", "ArrowLeft", "ArrowRight", "ArrowUpRight", "BarChart3", "Bold", "Braces", "CalendarDays", "Check", "CheckIcon", "ChevronDown", "ChevronDownIcon", "ChevronRight", "ChevronRightIcon", "ChevronUpIcon", "CircleCheckIcon", "CircleHelp", "CircleOff", "Clipboard", "ClipboardList", "Clock", "Clock3", "Copy", "CornerDownLeft", "CornerDownRight", "Download", "ExternalLink", "EyeOff", "File", "FileText", "Files", "Folder", "FolderInput", "FolderOpen", "FolderPlus", "Globe", "History", "Image", "ImagePlus", "InfoIcon", "Italic", "KeyRound", "Keyboard", "Layers", "Link", "List", "ListChecks", "ListOrdered", "Loader2", "Loader2Icon", "Lock", "Moon", "MoreHorizontal", "MousePointer2", "OctagonXIcon", "Package", "Palette", "PanelLeft", "Pause", "Pencil", "Pin", "PinOff", "Play", "Plus", "RefreshCw", "RemoveFormatting", "RotateCcw", "Search", "SearchIcon", "Settings", "Settings2", "ShieldAlert", "ShieldCheck", "ShieldOff", "SlidersHorizontal", "Sparkles", "Star", "Sun", "TextCursor", "TextCursorInput", "Trash2", "TriangleAlert", "TriangleAlertIcon", "Type", "Underline", "Upload", "X", "XIcon"]

    @Test func mapCoversEveryImport() {
        for name in Self.imported {
            #expect(Icon.lucide[name] != nil, "no symbol for \(name)")
        }
        #expect(Set(Icon.lucide.keys) == Set(Self.imported))
    }

    // Apple SF Symbols name_availability.plist: all these names shipped by macOS 14.0.
    // A runtime lookup alone on a newer Mac cannot establish Sonoma compatibility.
    static let sonomaSymbols: Set<String> = [
        "arrow.clockwise",
        "arrow.counterclockwise",
        "arrow.left",
        "arrow.right",
        "arrow.turn.down.left",
        "arrow.turn.down.right",
        "arrow.up.arrow.down",
        "arrow.up.right",
        "arrow.up.right.square",
        "bold",
        "calendar",
        "character.cursor.ibeam",
        "character.textbox",
        "chart.bar",
        "checklist",
        "checkmark",
        "checkmark.circle",
        "checkmark.shield",
        "chevron.down",
        "chevron.right",
        "chevron.up",
        "circle.dotted",
        "circle.slash",
        "clipboard",
        "clock",
        "clock.arrow.circlepath",
        "curlybraces",
        "cursorarrow",
        "doc",
        "doc.on.doc",
        "doc.text",
        "ellipsis",
        "eraser",
        "exclamationmark.shield",
        "exclamationmark.triangle",
        "eye.slash",
        "folder",
        "folder.badge.plus",
        "gearshape",
        "globe",
        "info.circle",
        "italic",
        "key",
        "keyboard",
        "link",
        "list.bullet",
        "list.clipboard",
        "list.number",
        "lock",
        "magnifyingglass",
        "moon",
        "paintpalette",
        "pause",
        "pencil",
        "photo",
        "photo.badge.plus",
        "pin",
        "pin.slash",
        "play",
        "plus",
        "questionmark.circle",
        "shield.slash",
        "shippingbox",
        "sidebar.left",
        "slider.horizontal.3",
        "sparkles",
        "square.3.layers.3d",
        "square.and.arrow.down",
        "square.and.arrow.up",
        "star",
        "sun.max",
        "switch.2",
        "text.alignleft",
        "textformat",
        "trash",
        "underline",
        "xmark",
        "xmark.octagon",
    ]

    @Test func mappingsStayWithinSonomaCatalog() {
        #expect(Set(Icon.lucide.values).isSubset(of: Self.sonomaSymbols))
        #expect(Icon.lucide["Loader2"] == "circle.dotted")
    }

    @Test func everySymbolResolves() {
        for (name, symbol) in Icon.lucide {
            #expect(NSImage(systemSymbolName: symbol, accessibilityDescription: nil) != nil, "\(name) → \(symbol) does not resolve")
        }
        #expect(NSImage(systemSymbolName: "star.fill", accessibilityDescription: nil) != nil)
    }
}
