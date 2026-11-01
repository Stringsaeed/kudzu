import AppKit
import KudzuUI
import SwiftUI

struct FloatingPanelPage: GalleryPage {
    static let id = "floating-panel"
    static let title = "FloatingPanel"
    static let section = GallerySection.floating
    static let icon = KzIconName.stack

    @State private var demo = FloatingPanelDemo()

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "KzFloatingPanel: a non-activating, key-capable NSPanel hosting SwiftUI on Kudzu glass. The search palette and the form at the caret are built on it.") {
            SpecimenSection("Specimen", note: "glass-strong, shadow-float, radius-xl. The window is larger than the glass so the shadow isn't clipped.") {
                KzFloatingPanelSurface {
                    PaletteDemoView(model: PaletteDemoModel(), interactive: false)
                }
                .padding(.bottom, KzSpace.s8)
            }

            SpecimenSection("Try it", note: "The palette is centred in the upper part of the screen; ↑ ↓ move, Return picks, Esc or clicking elsewhere closes it. The form opens under the pointer like it would under the caret, and stays until Esc or a button.") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s3) {
                        HStack(spacing: KzSpace.s3) {
                            KzButton("Open palette") { demo.showPalette() }
                            KzButton("Open form at pointer") { demo.showForm() }
                        }
                        Text(demo.log).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
        }
    }
}

@MainActor @Observable
final class FloatingPanelDemo {
    var log = "Closed."
    @ObservationIgnored private var palette: KzFloatingPanel?
    @ObservationIgnored private var form: KzFloatingPanel?
    @ObservationIgnored private let model = PaletteDemoModel()

    func showPalette() {
        let panel = palette ?? KzFloatingPanel(sizing: .fixed(CGSize(width: 520, height: 260))) {
            PaletteDemoView(model: model, interactive: true)
        }
        panel.keyHandler = { [weak self] event in self?.handle(event) ?? false }
        panel.onClose = { [weak self] reason in self?.log = "Palette closed (\(reason))." }
        palette = panel
        model.selection = 0
        panel.present(.centered(topFraction: 1.0 / 6))
        log = "Palette open."
    }

    func showForm() {
        let panel = form ?? KzFloatingPanel(behavior: .modeless) {
            FormDemoView(close: { [weak self] in self?.form?.dismiss() })
        }
        panel.onClose = { [weak self] reason in self?.log = "Form closed (\(reason))." }
        form = panel
        let pointer = NSEvent.mouseLocation
        panel.present(.edge(.bottom, anchor: CGRect(x: pointer.x, y: pointer.y - 8, width: 1, height: 16), gap: 6))
        log = "Form open."
    }

    private func handle(_ event: NSEvent) -> Bool {
        switch event.keyCode {
        case 125: model.move(1); return true
        case 126: model.move(-1); return true
        case 36, 76:
            log = "Picked \(model.items[model.selection])."
            palette?.dismiss()
            return true
        default: return false
        }
    }
}

@MainActor @Observable
final class PaletteDemoModel {
    let items = ["address", "signature", "meeting link", "shrug", "thanks"]
    var selection = 0
    var query = ""

    func move(_ delta: Int) {
        selection = (selection + delta + items.count) % items.count
    }
}

struct PaletteDemoView: View {
    @Bindable var model: PaletteDemoModel
    let interactive: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: KzSpace.s2) {
                KzIcon(.search, size: 16).foregroundStyle(KzColor.inkMuted)
                if interactive {
                    TextField("Search snippets", text: $model.query)
                        .textFieldStyle(.plain)
                        .kzTextStyle(.body, matchLineBox: false)
                } else {
                    Text("Search snippets").kzTextStyle(.body).foregroundStyle(KzColor.inkFaint)
                    Spacer()
                }
                KzKbd("esc")
            }
            .padding(KzSpace.s3)
            Rectangle().fill(KzColor.line).frame(height: 1)
            VStack(spacing: 2) {
                ForEach(Array(model.items.prefix(4).enumerated()), id: \.offset) { index, item in
                    HStack {
                        KzTriggerChip(":" + item.prefix(3))
                        Text(item).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
                        Spacer()
                    }
                    .padding(.horizontal, KzSpace.s2)
                    .frame(height: 36)
                    .background(index == model.selection ? KzColor.vineSoft : .clear, in: RoundedRectangle(cornerRadius: KzRadius.md))
                }
            }
            .padding(KzSpace.s2)
        }
        .frame(width: 520, alignment: .topLeading)
    }
}

struct FormDemoView: View {
    let close: () -> Void
    @State private var name = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            Text("Fill in “greeting”").kzTextStyle(.headline).foregroundStyle(KzColor.ink)
            KzTextInput("Name", text: $name, focus: $focused)
            HStack {
                KzButton("Cancel", action: close)
                Spacer()
                KzButton("Insert", variant: .primary, isDefault: true, action: close)
            }
        }
        .padding(KzSpace.s4)
        .frame(width: 300)
        .onAppear { focused = true }
    }
}
