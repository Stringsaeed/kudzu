// Gallery sections: motion on demand, restyled native controls, custom views and the icon map.

import KudzuCore
import SwiftUI

// MARK: Motion

struct GalleryMotion: View {
    @State private var run = 0
    @State private var count = 3
    @State private var star = 0
    @State private var showInserted = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        GalleryBlock("Keyframes (replay to see them again)") {
            Button("Replay") { run += 1 }.buttonStyle(.kz(.outline, size: .sm))
            HStack(spacing: 24) {
                demo("rise") { tile(t).rise() }
                demo("pop") { tile(t).pop() }
                demo("check-in") { Icon.check.icon(16).foregroundStyle(t[.success]).checkIn() }
                demo("permission-confirm") { tile(t).permissionConfirm() }
                demo("bar-grow") { InsightBar(fraction: 0.7).frame(width: 120) }
            }
            .id(run)
        }
        GalleryBlock("Triggered") {
            HStack(spacing: 24) {
                Button("Count \(count)") { count += 1 }.buttonStyle(.kz(.ghost, size: .sm))
                Text("\(count)").font(.system(size: 11).monospacedDigit()).faint().tick(count)
                Button { star += 1 } label: { Icon.starFill.icon(16).foregroundStyle(t[.favorite]).starBurst(star) }.buttonStyle(.kz(.ghost, size: .icon))
                Button(showInserted ? "Remove" : "Insert") { withAnimation { showInserted.toggle() } }.buttonStyle(.kz(.outline, size: .sm))
                if showInserted { tile(t).kzTransition(.pop) }
            }
        }
        GalleryBlock("Stagger · shimmer · spin · press") {
            VStack(alignment: .leading, spacing: 6) {
                ForEach(0..<7, id: \.self) { i in
                    Text("Child \(i + 1) — \(Int(Stagger.delay(for: i) * 1000)) ms").stagger(i)
                }
            }
            .id(run)
            Skeleton().frame(width: 220, height: 16)
            ProgressView().controlSize(.small)
            Button("Press me") {}.buttonStyle(.kz(.outline))
            Button("Press (custom row)") {}.buttonStyle(.press(scale: Press.subtle))
        }
        GalleryBlock("Springs (fitted to the CSS tables)") {
            Text("soft: bounce \(Motion.softBounce, specifier: "%.3f"), \(Motion.softSpring().duration * 1000, specifier: "%.0f") ms · pop: bounce \(Motion.popBounce, specifier: "%.3f"), \(Motion.popSpring().duration * 1000, specifier: "%.0f") ms")
                .font(.system(size: 12).monospacedDigit()).mutedForeground()
        }
    }

    private func tile(_ t: KZTheme) -> some View {
        Color.clear.frame(width: 56, height: 32).depth(.raised, fill: t[.surfaceRaised], in: RoundedRectangle(cornerRadius: Radius.md))
    }

    private func demo<V: View>(_ name: String, @ViewBuilder _ content: () -> V) -> some View {
        VStack(spacing: 6) {
            content().frame(height: 40)
            Text(name).font(.system(size: 11)).mutedForeground()
        }
    }
}

// MARK: Controls

struct GalleryControls: View {
    @State private var text = ""
    @State private var on = true
    @State private var checked = true
    @State private var choice = "typed"
    @State private var popover = false
    @State private var sheet = false
    @State private var alert = false
    @State private var notes = "Multi-line notes"

    var body: some View {
        GalleryBlock("Buttons: variants × sizes") {
            Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 8) {
                ForEach(ButtonVariant.allCases, id: \.self) { variant in
                    GridRow {
                        Text(variant.rawValue).font(.system(size: 11)).mutedForeground()
                        ForEach([ButtonSize.xs, .sm, .default, .lg], id: \.self) { size in
                            Button {} label: { ButtonLabel("Button", icon: Icon.plus) }.buttonStyle(.kz(variant, size: size))
                        }
                        Button {} label: { Icon.settings.icon() }.buttonStyle(.kz(variant, size: .icon))
                        Button("Disabled") {}.buttonStyle(.kz(variant)).disabled(true)
                    }
                }
            }
            Button {} label: { HStack(spacing: 6) { Text("Save"); Shortcut("mod+s") } }.buttonStyle(.kz())
        }
        GalleryBlock("Fields") {
            TextField("Snippet name", text: $text).kzInput().frame(width: 260)
            TextField("Invalid", text: $text).kzInput(invalid: true).frame(width: 260)
            InputGroup { Icon.search.icon() } field: { TextField("Search snippets", text: $text) } trailing: { Shortcut("mod+k", tone: .ghost) }.frame(width: 260)
            TextEditor(text: $notes).kzTextarea().frame(width: 260, height: 72)
            FieldLabel("Label") { TextField("With label", text: $text).kzInput().frame(width: 260) }
        }
        GalleryBlock("Toggles and pickers (native)") {
            Toggle("Switch", isOn: $on).kzSwitch()
            Toggle("Small switch", isOn: $on).kzSwitch(size: .sm)
            Toggle("Checkbox", isOn: $checked).kzCheckbox()
            Picker("Insertion", selection: $choice) { Text("Typed").tag("typed"); Text("Pasted").tag("pasted") }.kzRadioGroup()
            Picker("Insertion", selection: $choice) { Text("Typed").tag("typed"); Text("Pasted").tag("pasted") }.kzSelect()
            Segmented("Insertion", selection: $choice, options: [SegmentOption("Typed", value: "typed"), SegmentOption("Pasted", value: "pasted")])
        }
        GalleryBlock("Menus, popovers, dialogs, tooltips (native)") {
            HStack(spacing: 8) {
                Menu {
                    Section("Snippet") {
                        Button("Duplicate") {}.keyboardShortcut("d")
                        Toggle("Favourite", isOn: $checked)
                        Menu("Move to") { Button("Work") {}; Button("Personal") {} }
                    }
                    Divider()
                    Button("Delete", role: .destructive) {}.keyboardShortcut(.delete)
                } label: { Icon.moreHorizontal.icon() }
                .kzMenuTrigger(.ghost, size: .icon)
                Button("Popover") { popover = true }.buttonStyle(.kz(.outline)).kzPopover(isPresented: $popover) {
                    Text("Popover title").font(.system(size: 14, weight: .medium))
                    Text("Content with the web's padding and gap.").mutedForeground()
                }
                Button("Dialog") { sheet = true }.buttonStyle(.kz(.outline)).sheet(isPresented: $sheet) {
                    DialogLayout("Group settings", description: "Rename the group or change its rules.") {
                        TextField("Name", text: $text).kzInput()
                    } footer: {
                        Button("Cancel") { sheet = false }.buttonStyle(.kz(.outline))
                        Button("Save") { sheet = false }.buttonStyle(.kz())
                    }
                }
                Button("Alert") { alert = true }.buttonStyle(.kz(.destructive)).alert("Delete 3 snippets?", isPresented: $alert) {
                    Button("Delete", role: .destructive) {}
                    Button("Cancel", role: .cancel) {}
                } message: { Text("You can undo this from the toast.") }
                Button("Tooltip") {}.buttonStyle(.kz(.ghost)).kzHelp("New snippet", shortcut: "mod+n")
            }
            .contextMenu { Button("Context menu item") {}; Button("Another") {} }
        }
    }
}

// MARK: Views

struct GalleryViews: View {
    @Namespace private var pill
    @State private var query = ""
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        GalleryBlock("Kbd · Shortcut") {
            HStack(spacing: 12) {
                Kbd("⌘")
                Shortcut("mod+shift+k")
                Shortcut("mod+k", tone: .ghost)
                Shortcut("alt+space", tone: .accent)
                Shortcut("enter", tone: .onAccent).foregroundStyle(t[.irisContrast]).padding(4).background(t[.iris], in: RoundedRectangle(cornerRadius: 5))
            }
        }
        GalleryBlock("Badge · TriggerChip · Skeleton") {
            HStack(spacing: 8) {
                ForEach(BadgeVariant.allCases, id: \.self) { Badge($0.rawValue, variant: $0) }
                TriggerChip(";kudzu")
                TriggerChip("; addr", muted: true)
                Skeleton().frame(width: 80, height: 12)
            }
        }
        GalleryBlock("SelectionPill (custom row highlight)") {
            Text("Selected snippet")
                .font(.system(size: 12, weight: .medium))
                .padding(.horizontal, 10).frame(height: 28)
                .selectionPill(true, in: pill, style: .nav)
        }
        GalleryBlock("Toasts") {
            HStack(spacing: 8) {
                Button("Toast") { toast("Kudzu 0.7.0 is ready", description: "It installs when you quit, or restart now.", action: ToastButton("Restart") {}, cancel: ToastButton("What's new") {}, duration: 15) }
                Button("Undo toast") { toast("Deleted “Address”", action: ToastButton("Undo") {}, duration: 6) }
                Button("Success") { toast.success("Library exported", description: "~/Desktop/kudzu.json") }
                Button("Error") { toast.error("Import failed", description: "The file isn't valid JSON.") }
            }
            .buttonStyle(.kz(.outline, size: .sm))
        }
        GalleryBlock("CommandList") {
            CommandList(query: $query, items: [
                CommandItem(id: "new", title: "New snippet", icon: Icon.plus, shortcut: "mod+n", group: "Snippets") {},
                CommandItem(id: "import", title: "Import…", icon: Icon.download, group: "Snippets") {},
                CommandItem(id: "settings", title: "Settings", icon: Icon.settings, shortcut: "mod+,", group: "App") {},
                CommandItem(id: "insights", title: "Insights", icon: Icon.barChart3, group: "App") {},
            ])
            .frame(width: 360)
            .depth(.panel, in: RoundedRectangle(cornerRadius: Radius.xl))
        }
        GalleryBlock("FormattedText · AppIcon · Wordmark · Breadcrumbs") {
            FormattedText(text: "Hello bold and a link, cursor here", spans: [
                FormatSpan(start: 6, end: 10, style: .bold), FormatSpan(start: 17, end: 21, style: .link(url: "https://kudzu.site")),
            ], cursorOffset: 34)
            HStack(spacing: 12) {
                AppIcon(path: "/System/Applications/TextEdit.app", name: "TextEdit", size: 32)
                AppIcon(path: nil, name: "Slack", size: 32)
                Wordmark().frame(width: 160).foregroundStyle(.white).padding(12).background(Wordmark.brandBlue, in: RoundedRectangle(cornerRadius: Radius.lg))
                Wordmark(.small).frame(width: 76).foregroundStyle(t[.mutedForeground])
            }
            Breadcrumbs((0..<6).map { BreadcrumbEntry(id: $0, label: ["Library", "Work", "Clients", "Acme", "Email", "Signatures"][$0]) })
                .frame(width: 420)
        }
        GalleryBlock("Item rows · InsightBar") {
            VStack(spacing: 4) {
                ForEach(Array(["Email signature", "Address", "Meeting link"].enumerated()), id: \.offset) { i, name in
                    Item(i == 0 ? .muted : (i == 1 ? .outline : .default)) {
                        ItemMedia(.icon) { Icon.fileText.icon() }
                        ItemContent {
                            ItemTitle(name)
                            ItemDescription("Expands \(name.lowercased()) wherever you type")
                        }
                        ItemActions { TriggerChip(";s\(i)") }
                    }
                    InsightBar(fraction: [0.9, 0.5, 0.2][i], rank: i)
                }
            }
            .frame(width: 420)
        }
    }
}

// MARK: Icons

struct GalleryIcons: View {
    var body: some View {
        GalleryBlock("Lucide → SF Symbols (\(Icon.lucide.count) imports)") {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 170), spacing: 8)], alignment: .leading, spacing: 10) {
                ForEach(Icon.lucide.sorted { $0.key < $1.key }, id: \.key) { name, symbol in
                    HStack(spacing: 8) {
                        Image(systemName: symbol).icon(16)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(name).font(.system(size: 12))
                            Text(symbol).font(.system(size: 10)).faint()
                        }
                    }
                }
            }
        }
    }
}

#Preview("Design gallery — light") {
    DesignGallery().frame(width: 1000, height: 760).preferredColorScheme(.light)
}

#Preview("Design gallery — dark") {
    DesignGallery().frame(width: 1000, height: 760).preferredColorScheme(.dark)
}
