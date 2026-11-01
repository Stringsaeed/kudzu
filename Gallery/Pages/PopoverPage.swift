import KudzuUI
import SwiftUI

struct PopoverPage: GalleryPage {
    static let id = "popover"
    static let title = "Popover"
    static let section = GallerySection.components
    static let icon = KzIconName.info

    @State private var showsInspector = false
    @State private var showsPinned = false
    @State private var edgeShown: Edge?
    @State private var format = "%d %b %Y"
    @State private var uppercase = false

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A glass floating layer with no arrow, anchored to a view: chip-editor inspectors and similar. It takes keyboard focus without activating Kudzu, Esc closes it, and focus goes back where it was.") {
            SpecimenSection("Specimen", note: "glass-strong, shadow-float, radius-lg, 12pt content inset.") {
                HStack(alignment: .top, spacing: KzSpace.s6) {
                    KzPopoverSurface { inspector(interactive: false) }
                }
                .padding(.bottom, KzSpace.s8)
            }

            SpecimenSection("Try it", note: "Transient popovers close on a click outside or when another window takes focus; the pinned one only on Esc or its button.") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s4) {
                        HStack(spacing: KzSpace.s3) {
                            Button { showsInspector.toggle() } label: {
                                KzTriggerChip("{date}")
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Edit date tag")
                            .kzPopover(isPresented: $showsInspector) {
                                inspector(interactive: true)
                            }

                            Button("Pinned popover") { showsPinned.toggle() }
                                .buttonStyle(FloatDemoButtonStyle())
                                .kzPopover(isPresented: $showsPinned, behavior: .applicationDefined) {
                                    VStack(alignment: .leading, spacing: KzSpace.s2) {
                                        Text("Stays open").kzTextStyle(.headline).foregroundStyle(KzColor.ink)
                                        Text("Click elsewhere; it stays. Esc closes it.").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                                    }
                                    .frame(width: 220, alignment: .leading)
                                }
                        }
                        HStack(spacing: KzSpace.s3) {
                            ForEach([Edge.top, .bottom, .leading, .trailing], id: \.self) { edge in
                                Button(name(of: edge)) { edgeShown = edge }
                                    .buttonStyle(FloatDemoButtonStyle())
                                    .kzPopover(isPresented: edgeBinding(edge), edge: edge) {
                                        Text("Opens on the \(name(of: edge).lowercased()) edge")
                                            .kzTextStyle(.body).foregroundStyle(KzColor.ink)
                                    }
                            }
                        }
                    }
                }
            }
        }
    }

    private func inspector(interactive: Bool) -> some View {
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            Text("Date tag").kzTextStyle(.headline).foregroundStyle(KzColor.ink)
            VStack(alignment: .leading, spacing: KzSpace.s1) {
                Text("Format").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                if interactive {
                    TextField("Format", text: $format)
                        .textFieldStyle(.roundedBorder)
                        .font(KzTextStyle.code.font)
                } else {
                    // Specimens render offscreen, where AppKit-backed fields can't draw.
                    Text(format).kzTextStyle(.code).foregroundStyle(KzColor.ink)
                        .padding(.horizontal, KzSpace.s2)
                        .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.sm))
                }
            }
            if interactive {
                Toggle("Uppercase", isOn: $uppercase)
            }
            Text("Preview: \(Date.now.formatted(date: .abbreviated, time: .omitted))")
                .kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
        }
        .frame(width: 240, alignment: .leading)
    }

    private func edgeBinding(_ edge: Edge) -> Binding<Bool> {
        Binding(get: { edgeShown == edge }, set: { edgeShown = $0 ? edge : (edgeShown == edge ? nil : edgeShown) })
    }

    private func name(of edge: Edge) -> String {
        switch edge {
        case .top: "Top"
        case .bottom: "Bottom"
        case .leading: "Leading"
        case .trailing: "Trailing"
        }
    }
}
