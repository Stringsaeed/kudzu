import AppKit
import SwiftUI
import Testing
@testable import KudzuDesign

@Suite struct ReviewRegressionTests {
    @Test @MainActor func edgeHighlightsPreserveDarkSurfaceCenters() async throws {
        _ = NSApplication.shared
        // These levels used to wash the whole fill with their zero-radius inner shadow.
        for depth in [Depth.card, .raised, .raisedHover, .key, .selected, .accent, .soft] {
            let view = Color.clear.frame(width: 100, height: 60)
                .depth(depth, fill: Color.black, in: RoundedRectangle(cornerRadius: 10))
                .padding(10)
                .environment(\.colorScheme, .dark)
            let bitmap = try await Self.render(view, size: NSSize(width: 120, height: 80))
            let scale = bitmap.pixelsWide / 120
            let center = try #require(bitmap.colorAt(x: 60 * scale, y: 40 * scale)?.usingColorSpace(.sRGB))
            #expect(center.redComponent <= 1.0 / 255, "\(depth.name) changed its black fill")
            #expect(center.greenComponent <= 1.0 / 255)
            #expect(center.blueComponent <= 1.0 / 255)
            let edge = try #require(bitmap.colorAt(x: 60 * scale, y: 10 * scale)?.usingColorSpace(.sRGB))
            #expect(edge.redComponent > center.redComponent, "\(depth.name) lost its top light")
        }
    }

    @Test @MainActor func wordmarkKeepsEchoIconBlue() throws {
        let blue = try #require(NSColor(Wordmark.brandBlue).usingColorSpace(.sRGB))
        #expect(abs(blue.redComponent - 20.0 / 255) < 1e-6)
        #expect(abs(blue.greenComponent - 71.0 / 255) < 1e-6)
        #expect(abs(blue.blueComponent - 230.0 / 255) < 1e-6)
    }

    @Test @MainActor func filledKeycapsInheritLightLabelColor() async throws {
        for filledButton in [false, true] {
            let view = VStack {
                if filledButton {
                    Button {} label: { Kbd("M") }.buttonStyle(.kz())
                } else {
                    Kbd("M", tone: .onAccent).foregroundStyle(.white)
                }
            }
            .foregroundStyle(.black)
            .background(Color.blue)
            .environment(\.colorScheme, .light)
            let bitmap = try await Self.render(view, size: NSSize(width: 40, height: 32))
            var brightPixels = 0
            for y in 0..<bitmap.pixelsHigh {
                for x in 0..<bitmap.pixelsWide {
                    let color = try #require(bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB))
                    if min(color.redComponent, color.greenComponent, color.blueComponent) > 0.8 { brightPixels += 1 }
                }
            }
            #expect(brightPixels > 5, "Filled keycap lost the inherited white glyph")
        }
    }

    @MainActor private static func render<V: View>(_ content: V, size: NSSize) async throws -> NSBitmapImageRep {
        _ = NSApplication.shared
        let view = NSHostingView(rootView: content.frame(width: size.width, height: size.height))
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = view
        defer { window.contentView = nil }
        view.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        view.layoutSubtreeIfNeeded()
        let bitmap = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        return bitmap
    }

    @Test func smallButtonLabelMetrics() {
        #expect(ButtonSize.xs.gap == 4 && ButtonSize.sm.gap == 4)
        #expect(ButtonSize.default.gap == 6 && ButtonSize.lg.gap == 6)
        #expect(ButtonSize.sm.inlineIconInset == 4)
        #expect(ButtonSize.xs.inlineIconInset == 2 && ButtonSize.default.inlineIconInset == 2)
    }

    @Test @MainActor func filledKeycapsHaveQuietInsetOnly() {
        for dark in [false, true] {
            let r = Kbd.filledDepth.recipe(dark: dark)
            #expect(r.drops.isEmpty && r.hairline == nil)
            #expect(r.inner == [.init(radius: 0.5, y: 1, color: OKLCH(0, 0, 0, alpha: 0.15))])
        }
    }

    @Test @MainActor func inlineImageFitsContainerWithoutChangingAspectRatio() throws {
        let source = NSImage(size: NSSize(width: 400, height: 200), flipped: false) { rect in
            NSColor.red.setFill(); NSBezierPath(rect: rect).fill(); return true
        }
        let data = try #require(source.tiffRepresentation)
        let url = "data:image/tiff;base64," + data.base64EncodedString()
        let image = try #require(FormattedText.image(dataURL: url, width: 512, maximumWidth: 120))
        #expect(image.size == NSSize(width: 120, height: 60))
        let small = try #require(FormattedText.image(dataURL: url, width: 80, maximumWidth: 120))
        #expect(small.size == NSSize(width: 80, height: 40))
    }

    @Test @MainActor func replacingToastCancelsPreviousDeadline() async throws {
        let center = ToastCenter()
        center.show("Old", duration: 0.02, id: "same")
        center.show("Still loading", kind: .loading, id: "same")
        try await Task.sleep(for: .milliseconds(80))
        #expect(center.toasts.map(\.title) == ["Still loading"])
        center.dismiss()
    }

    @Test @MainActor func hoveringSingleToastPausesDeadline() async throws {
        let center = ToastCenter()
        center.show("Undo", duration: 0.04)
        center.hovered = true
        try await Task.sleep(for: .milliseconds(80))
        #expect(center.toasts.count == 1)
        center.hovered = false
        try await Task.sleep(for: .milliseconds(80))
        #expect(center.toasts.isEmpty)
    }

    @Test func neutralAndSaturatedConversions() {
        let grey = OKLCH(0.5, 0, 0).srgb
        #expect(abs(grey.r - 0.388572859) < 1e-8)
        #expect(grey.hex == "#636363")
        #expect(Palette.value(.primary, dark: false).srgb.hex == "#1447E6")
        #expect(Palette.value(.iris, dark: true).srgb.hex == "#2B7FFF")
    }
}
