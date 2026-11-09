import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Quick search (`palette.tsx`). The shell hosts this in the palette panel (760×480). Search state
/// lives in ``PaletteModel``; inserting, copying and editing go through ``AppModel/perform(_:)``.
public struct PaletteView: View {
    @Environment(AppModel.self) private var model
    @State private var box = PaletteBox()

    public init() {}

    /// A palette that already has its library (previews and `ImageRenderer`).
    public init(palette: PaletteModel) {
        let box = PaletteBox()
        box.model = palette
        _box = State(initialValue: box)
    }

    public var body: some View {
        PaletteScreen(palette: box.resolve(model))
            .task { await box.model?.start() }
    }
}

/// Holds the palette for the lifetime of the view. Created during `body` so the first frame,
/// including `ImageRenderer`, already has the library.
@MainActor
private final class PaletteBox {
    var model: PaletteModel?
    func resolve(_ app: AppModel) -> PaletteModel {
        if let model { return model }
        let created = PaletteModel(backend: app.backend, library: app.library)
        created.show(target: app.paletteTarget)
        model = created
        return created
    }
}

struct PaletteScreen: View {
    @Bindable var palette: PaletteModel
    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var scheme
    @FocusState private var searchFocused: Bool
    @Namespace private var scopes
    @State private var flags = CommandKeyMonitor()
    @State private var hovered: Int?
    @State private var lastKey: PaletteKey?
    @State private var lastKeyAt: ContinuousClock.Instant = .now

    var body: some View {
        let theme = KZTheme(scheme)
        VStack(spacing: 0) {
            searchBar(theme)
            hairline(theme)
            GeometryReader { geo in
                let listWidth = (geo.size.width * 0.54).rounded()
                HStack(spacing: 0) {
                    results(theme).frame(width: listWidth)
                    Rectangle().fill(theme[.hairline]).frame(width: 1)
                    PalettePreviewPane(palette: palette).frame(width: max(0, geo.size.width - listWidth - 1))
                }
            }
            .frame(maxHeight: .infinity)
            hairline(theme)
            footer(theme)
        }
        .background(theme[.popover].opacity(0.55))
        .foregroundStyle(theme[.popoverForeground])
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Quick search")
        .onAppear {
            searchFocused = true
            palette.showNumbers = NSEvent.modifierFlags.contains(.command)
            flags.install { palette.showNumbers = $0 }
        }
        .onDisappear { flags.remove() }
        .onChange(of: palette.showCount) { searchFocused = true }
        .onExitCommand { perform(.escape) }
        .background { commandShortcuts }
    }

    // MARK: Search

    private func searchBar(_ theme: KZTheme) -> some View {
        HStack(spacing: 12) {
            Icon.search.icon(18).foregroundStyle(theme[.mutedForeground])
            TextField(text: $palette.query, prompt: Text("Search snippets…").foregroundStyle(theme[.faint])) { Text("Search snippets") }
                .textFieldStyle(.plain)
                .font(.system(size: 17))
                .tracking(-0.17)
                .focused($searchFocused)
                .autocorrectionDisabled()
                .accessibilityLabel("Search snippets")
                .onKeyPress(phases: [.down, .repeat]) { press in
                    guard let key = PaletteKeys.interpret(Self.input(press), command: press.modifiers.contains(.command), control: press.modifiers.contains(.control), shift: press.modifiers.contains(.shift)) else {
                        return .ignored
                    }
                    perform(key)
                    return .handled
                }
            scopeControl(theme)
        }
        .padding(.horizontal, 16)
        .frame(height: 58)
    }

    private func scopeControl(_ theme: KZTheme) -> some View {
        HStack(spacing: 0) {
            ForEach(PaletteScope.allCases, id: \.self) { scope in
                Button {
                    palette.setScope(scope)
                } label: {
                    Text(scope.label)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(palette.scope == scope ? theme[.foreground] : theme[.mutedForeground])
                        .frame(height: 24)
                        .padding(.horizontal, 8)
                        .selectionPill(palette.scope == scope, in: scopes, style: Self.scopePill)
                }
                .buttonStyle(.press)
                .accessibilityAddTraits(palette.scope == scope ? [.isSelected] : [])
            }
        }
        .padding(2)
        .depth(.well, fill: theme[.foreground, alpha: 0.04], in: RoundedRectangle(cornerRadius: Radius.lg))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Scope")
    }

    /// `SelectionPill className="rounded-md bg-surface-raised shadow-soft"` (no sheen).
    private static let scopePill = SelectionPillStyle(radius: Radius.md, fill: .surfaceRaised, depth: .soft, sheen: false)

    // MARK: Results

    @ViewBuilder private func results(_ theme: KZTheme) -> some View {
        if !palette.loaded {
            Color.clear
        } else if palette.total == 0 {
            VStack(spacing: 4) {
                Text(palette.emptyMessage).font(.system(size: 14)).foregroundStyle(theme[.mutedForeground])
                Text("Type to create one.").font(.system(size: 12)).foregroundStyle(theme[.faint])
            }
            .multilineTextAlignment(.center)
            .padding(.horizontal, 24)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollViewReader { proxy in
                List {
                    sectionHeader(theme)
                    ForEach(Array(palette.hits.enumerated()), id: \.element.snippet.id) { pair in
                        rowButton(index: pair.offset, theme: theme) {
                            PaletteHitRow(hit: pair.element, index: pair.offset, active: pair.offset == palette.clamped, showNumbers: palette.showNumbers, path: palette.path(of: pair.element.snippet))
                        }
                        .id(pair.offset)
                    }
                    if palette.createRow {
                        rowButton(index: palette.hits.count, selectedDepth: false, theme: theme) {
                            PaletteCreateRow(query: palette.trimmedQuery, active: palette.clamped == palette.hits.count)
                        }
                        .id(palette.hits.count)
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .environment(\.defaultMinListRowHeight, 28)
                .onChange(of: palette.clamped) { _, index in
                    // `scrollIntoView({ block: "nearest" })`: scroll only when the row is out of view.
                    proxy.scrollTo(index)
                }
            }
        }
    }

    private func sectionHeader(_ theme: KZTheme) -> some View {
        HStack(spacing: 4) {
            Text(palette.sectionLabel)
            if let group = palette.groupFilter {
                Text("· #\(group.isEmpty ? "…" : group)").foregroundStyle(theme[.irisForeground])
            }
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundStyle(theme[.faint])
        .frame(maxWidth: .infinity, alignment: .leading)
        .listRowInsets(EdgeInsets(top: 6, leading: 16, bottom: 2, trailing: 12))
        .listRowSeparator(.hidden)
        .listRowBackground(Color.clear)
        .accessibilityAddTraits(.isHeader)
    }

    /// A result row (`ResultRow`): `h-11 rounded-[10px]` touching rows inside the list's `p-1.5`;
    /// active `bg-foreground/[0.08] shadow-selected` (the create row has no shadow), hovered
    /// `bg-foreground/[0.03]`, over `duration-75`.
    private func rowButton(index: Int, selectedDepth: Bool = true, theme: KZTheme, @ViewBuilder label: () -> some View) -> some View {
        let active = index == palette.clamped
        return Button {
            pick(index)
        } label: {
            label().frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { inside in
            if inside {
                hovered = index
                palette.active = index
            } else if hovered == index {
                hovered = nil
            }
        }
        .listRowInsets(EdgeInsets(top: 0, leading: 6, bottom: 0, trailing: 6))
        .listRowSeparator(.hidden)
        .listRowBackground(
            rowFill(active: active, hovered: hovered == index, selectedDepth: selectedDepth, theme: theme)
                .padding(.horizontal, 6)
        )
    }

    private func rowFill(active: Bool, hovered: Bool, selectedDepth: Bool, theme: KZTheme) -> some View {
        let fill = active ? theme[.foreground, alpha: 0.08] : (hovered ? theme[.foreground, alpha: 0.03] : .clear)
        return Color.clear
            .depth(active && selectedDepth ? .selected : .none, fill: fill, in: RoundedRectangle(cornerRadius: 10))
            .kzAnimation(.kz.ease(duration: 0.075), value: active)
            .kzAnimation(.kz.ease(duration: 0.075), value: hovered)
    }

    // MARK: Footer

    private func footer(_ theme: KZTheme) -> some View {
        HStack(spacing: 16) {
            HStack(spacing: 6) {
                Icon.cornerDownLeft.icon(12)
                    .foregroundStyle(theme[.irisContrast])
                    .frame(width: 18, height: 18)
                    .depth(.accent, fill: theme[.iris], in: RoundedRectangle(cornerRadius: 5))
                    .overlay { RoundedRectangle(cornerRadius: 5).fill(Sheen.strong.gradient).allowsHitTesting(false) }
                Text(palette.footerLabel).lineLimit(1)
            }
            hint("mod+enter", "Copy", theme: theme)
            hint("mod+e", "Edit", theme: theme)
            hint("tab", "Scope", theme: theme)
            Spacer(minLength: 0)
            HStack(spacing: 6) {
                Text("#").font(KZFonts.mono(size: 11))
                Text("group")
            }
            .foregroundStyle(theme[.faint])
            Shortcut("escape")
        }
        .font(.system(size: 11.5))
        .foregroundStyle(theme[.mutedForeground])
        .padding(.horizontal, 16)
        .frame(height: 40)
    }

    private func hint(_ combo: String, _ title: String, theme: KZTheme) -> some View {
        HStack(spacing: 6) {
            Shortcut(combo)
            Text(title)
        }
        .foregroundStyle(theme[.mutedForeground])
    }

    private func hairline(_ theme: KZTheme) -> some View {
        Rectangle().fill(theme[.hairline]).frame(height: 1)
    }

    // MARK: Keys

    /// ⌘ chords also go through `.keyboardShortcut`, because a focused field sometimes swallows them
    /// before `onKeyPress`. Identical repeats inside one turn are ignored.
    private var commandShortcuts: some View {
        Group {
            Button("Edit") { perform(.edit) }.keyboardShortcut("e", modifiers: .command)
            Button("New snippet") { perform(.new) }.keyboardShortcut("n", modifiers: .command)
            Button("Copy") { perform(.enter(command: true)) }.keyboardShortcut(.return, modifiers: .command)
            ForEach(1..<10, id: \.self) { number in
                Button("Insert \(number)") { perform(.number(number)) }
                    .keyboardShortcut(KeyEquivalent(Character("\(number)")), modifiers: .command)
            }
        }
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .frame(width: 0, height: 0)
    }

    private func perform(_ key: PaletteKey) {
        let now = ContinuousClock.now
        if lastKey == key, now - lastKeyAt < .milliseconds(40) { return }
        lastKey = key
        lastKeyAt = now
        guard let action = palette.press(key) else { return }
        Task { await model.perform(action) }
    }

    private func pick(_ index: Int) {
        let action = palette.pick(index)
        Task { await model.perform(action) }
    }

    private static func input(_ press: KeyPress) -> PaletteInput {
        switch press.key {
        case .downArrow: return .down
        case .upArrow: return .up
        case .pageDown: return .pageDown
        case .pageUp: return .pageUp
        case .escape: return .escape
        case .tab: return .tab
        case .return: return .return
        default: return .character(press.characters)
        }
    }
}

#Preview("Palette") {
    PalettePreviewHost()
}

#Preview("Palette Dark") {
    PalettePreviewHost().preferredColorScheme(.dark)
}

private struct PalettePreviewHost: View {
    @State private var model: AppModel?

    var body: some View {
        Group {
            if let model {
                PaletteView().environment(model)
            } else {
                ProgressView()
            }
        }
        .frame(width: 760, height: 480)
        .task { await load() }
    }

    private func load() async {
        guard model == nil else { return }
        let app = AppModel(backend: MockBackend(), isDev: true)
        await app.start()
        app.paletteTarget = AppInfo(bundleId: "com.apple.mail", name: "Mail")
        model = app
    }
}
