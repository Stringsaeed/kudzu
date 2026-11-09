// Image insertion (image-tools.tsx). Native flow: **Image** opens an `NSOpenPanel` for PNG, JPEG
// or WebP; the chosen file is compacted off the main thread; a popover then shows the preview,
// the text alternative and the display width (field + `Menu` of presets) before inserting.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI
import UniformTypeIdentifiers

public struct ImageTools: View {
    var handle: TemplateEditorHandle

    @State private var open = false
    @State private var image: CompactImage?
    @State private var label = ""
    @State private var width = 240
    @State private var error = ""
    @State private var loading = false
    @State private var request = 0

    public init(handle: TemplateEditorHandle) { self.handle = handle }

    static let widthPresets = [64, 120, 160, 240, 320, 512]

    public var body: some View {
        Button(action: choose) {
            HStack(spacing: 4) {
                Icon.imagePlus.icon(14)
                Text("Image")
            }
            .foregroundStyle(Color.kz.mutedForeground)
        }
        .buttonStyle(.kz(.ghost, size: .sm, expanded: open))
        .kzHelp("Insert image")
        .accessibilityLabel("Insert image")
        .kzPopover(isPresented: $open, width: 320) { form }
        .onChange(of: open) { _, isOpen in
            if !isOpen {
                request += 1
                handle.focus()
            }
        }
    }

    @ViewBuilder private var form: some View {
        Text("Embed an image").font(.system(size: 14, weight: .medium))
        Text("Saved with your snippet. Large images are resized automatically.")
            .font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
            .fixedSize(horizontal: false, vertical: true)
        if loading {
            HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Preparing image…").font(.system(size: 12)) }
        }
        if let image, let picture = ImageCell.decode(image.dataUrl) {
            Image(nsImage: picture)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(maxWidth: min(CGFloat(width), 300), maxHeight: 160)
                .clipShape(RoundedRectangle(cornerRadius: Radius.sm))
                .frame(maxWidth: .infinity)
                .accessibilityLabel(label)
            FieldLabel("Text alternative") {
                TextField("Image", text: $label).kzInput()
                    .onChange(of: label) { _, value in if value.count > 200 { label = String(value.prefix(200)) } }
            }
            FieldLabel("Width in pixels") {
                HStack(spacing: 6) {
                    TextField("Width", value: $width, format: .number).kzInput()
                        .onSubmit { width = Self.clamp(width) }
                    Menu {
                        ForEach(Self.widthPresets, id: \.self) { preset in
                            Button("\(preset) px") { width = preset }
                        }
                        Divider()
                        Button("Original (\(image.pixelWidth) px)") { width = Self.clamp(image.pixelWidth) }
                    } label: { Text("Presets") }
                    .kzMenuTrigger(.outline, size: .sm, indicator: true)
                }
            }
        }
        if !error.isEmpty {
            Text(error).font(.system(size: 12)).foregroundStyle(Color.kz.destructive)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isStaticText)
        }
        HStack {
            Button("Choose another…", action: choose).buttonStyle(.kz(.outline, size: .sm))
            Spacer()
            Button(loading ? "Preparing image…" : "Insert image", action: insert)
                .buttonStyle(.kz(size: .sm))
                .keyboardShortcut(.defaultAction)
                .disabled(image == nil || loading)
        }
    }

    static func clamp(_ width: Int) -> Int { min(512, max(16, width)) }

    private func choose() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = ImageCompactor.accepted
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.prompt = "Choose"
        panel.message = "Choose a PNG, JPEG, or WebP image."
        open = false
        panel.begin { response in
            guard response == .OK, let url = panel.url else {
                MainActor.assumeIsolated { handle.focus() }
                return
            }
            MainActor.assumeIsolated { load(url) }
        }
    }

    private func load(_ url: URL) {
        request += 1
        let token = request
        image = nil
        error = ""
        loading = true
        open = true
        Task {
            let result = await Task.detached(priority: .userInitiated) { () -> Result<CompactImage, ImageCompactor.Failure> in
                do throws(ImageCompactor.Failure) { return .success(try ImageCompactor.compact(contentsOf: url)) } catch { return .failure(error) }
            }.value
            guard token == request else { return }
            loading = false
            switch result {
            case .success(let compact):
                image = compact
                width = compact.width
                label = url.deletingPathExtension().lastPathComponent
            case .failure(let failure):
                error = failure.localizedDescription
            }
        }
    }

    private func insert() {
        guard let image, !loading, !handle.isComposing else { return }
        do {
            try handle.insertImage(dataUrl: image.dataUrl, width: Self.clamp(width), label: label)
            open = false
        } catch {
            self.error = error.localizedDescription
        }
    }
}
