import AppKit
import KudzuUI
import SwiftUI
import Testing

@Suite("Fonts")
struct FontTests {
    @Test("Registration succeeds and is idempotent")
    func registers() {
        #expect(KzFonts.register() == .registered)
        #expect(KzFonts.register() == .registered)
        #expect(KzFonts.status == .registered)
    }

    @Test("Every type style resolves to its bundled face", arguments: KzTextStyle.allCases)
    func styleUsesBundledFace(_ style: KzTextStyle) {
        KzFonts.register()
        #expect(style.usesBundledFont)
        #expect(style.nsFont.fontName == style.postScriptName)
        #expect(style.nsFont.pointSize == style.spec.size)
    }

    @MainActor
    @Test("A single line is exactly the CSS line height", arguments: KzTextStyle.allCases)
    func lineBox(_ style: KzTextStyle) throws {
        let image = try #require(KzSnapshot.image(of: Text("Ag").kzTextStyle(style), appearance: .light, scale: 2))
        // Within one device pixel at 2x (the renderer rounds the image size up).
        #expect(abs(CGFloat(image.height) / 2 - style.spec.lineHeight) <= 0.5, "\(style): \(image.height / 2)pt, natural \(style.naturalLineHeight)")
    }

    @Test("Faces map to the right weights")
    func faces() {
        #expect(KzTextStyle.body.postScriptName == "Geist-Regular")
        #expect(KzTextStyle.bodyStrong.postScriptName == "Geist-Medium")
        #expect(KzTextStyle.title.postScriptName == "Geist-SemiBold")
        #expect(KzTextStyle.trigger.postScriptName == "GeistMono-Medium")
        #expect(KzTextStyle.code.postScriptName == "GeistMono-Regular")
    }

    @Test("Stat uses tabular figures")
    func tabular() {
        let font = KzTextStyle.stat.nsFont
        let one = ("1" as NSString).size(withAttributes: [.font: font]).width
        let eight = ("8" as NSString).size(withAttributes: [.font: font]).width
        #expect(abs(one - eight) < 0.01)
    }
}
