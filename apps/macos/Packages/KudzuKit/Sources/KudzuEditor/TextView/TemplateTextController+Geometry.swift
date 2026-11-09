// Geometry: caret rects for `coordsAt` and the slash menu, chip hit-testing, and the 1.7 line
// height with CSS half-leading (text centred in each line box).

import AppKit
import KudzuCore

extension TemplateTextController {
    var layoutManager: NSLayoutManager? { textView.layoutManager }

    /// Caret rect at a storage index, in text view coordinates.
    func caretRect(atStorage index: Int) -> NSRect? {
        guard let layout = layoutManager, let container = textView.textContainer else { return nil }
        layout.ensureLayout(for: container)
        let origin = textView.textContainerOrigin
        let length = storage.length
        let text = storage.string as NSString
        let index = min(max(0, index), length)
        if length == 0 || (index == length && text.character(at: length - 1) == 0x0A) {
            var extra = layout.extraLineFragmentRect
            if extra.isEmpty { extra = NSRect(x: 0, y: 0, width: 1, height: lineHeight) }
            return NSRect(x: origin.x + extra.minX, y: origin.y + extra.minY, width: 1, height: max(extra.height, lineHeight))
        }
        let character = index < length ? index : length - 1
        let glyph = layout.glyphIndexForCharacter(at: character)
        let line = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        let x: CGFloat
        if index < length {
            x = line.minX + layout.location(forGlyphAt: glyph).x
        } else {
            x = layout.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: container).maxX
        }
        return NSRect(x: origin.x + x, y: origin.y + line.minY, width: 1, height: line.height)
    }

    /// The bounding rect of the character at a storage index, in text view coordinates.
    func glyphRect(atStorage index: Int) -> NSRect? {
        guard let layout = layoutManager, let container = textView.textContainer, index < storage.length else { return nil }
        layout.ensureLayout(for: container)
        let glyphs = layout.glyphRange(forCharacterRange: NSRange(location: index, length: 1), actualCharacterRange: nil)
        return layout.boundingRect(forGlyphRange: glyphs, in: container).offsetBy(dx: textView.textContainerOrigin.x, dy: textView.textContainerOrigin.y)
    }

    /// The atom under a point (text view coordinates), with its storage index and rect.
    func atomHit(at point: NSPoint) -> (index: Int, atom: EditorAtom, rect: NSRect)? {
        guard let layout = layoutManager, let container = textView.textContainer, storage.length > 0 else { return nil }
        let origin = textView.textContainerOrigin
        let local = NSPoint(x: point.x - origin.x, y: point.y - origin.y)
        let glyph = layout.glyphIndex(for: local, in: container, fractionOfDistanceThroughGlyph: nil)
        let index = layout.characterIndexForGlyph(at: glyph)
        guard index < storage.length, let atom = map.atom(atStorage: index), let rect = glyphRect(atStorage: index), rect.contains(point) else { return nil }
        return (index, atom, rect)
    }

    /// Text view rect → screen. Without a window (tests, offscreen) the rect stays in view space.
    func screenRect(_ rect: NSRect) -> CGRect {
        guard let window = textView.window else { return rect }
        return window.convertToScreen(textView.convert(rect, to: nil))
    }

    /// Text view rect → the editor's own (SwiftUI, top-left origin) coordinate space.
    func editorRect(_ rect: NSRect) -> CGRect {
        let inScroll = textView.convert(rect, to: scrollView)
        return scrollView.isFlipped ? inScroll : CGRect(x: inScroll.minX, y: scrollView.bounds.height - inScroll.maxY, width: inScroll.width, height: inScroll.height)
    }

    /// Caret rect at a source position, in screen coordinates (`coordsAt`).
    public func coordsAt(_ position: Int) -> CGRect? {
        caretRect(atStorage: map.storageOffset(position, bias: .before)).map(screenRect)
    }

    /// Caret rect at a source position in the editor's coordinate space, for SwiftUI popovers.
    public func localRect(at position: Int) -> CGRect? {
        caretRect(atStorage: map.storageOffset(position, bias: .before)).map(editorRect)
    }

    /// Caret rect at a source position, in text view coordinates.
    public func viewRect(at position: Int) -> CGRect? {
        caretRect(atStorage: map.storageOffset(position, bias: .before))
    }

    /// The chip's rect for a tag starting at `position`, in text view coordinates.
    public func chipRect(at position: Int) -> CGRect? {
        let index = map.storageOffset(position, bias: .before)
        guard map.atom(atStorage: index) != nil else { return nil }
        return glyphRect(atStorage: index)
    }
}

// MARK: - Line height

extension TemplateTextController: NSLayoutManagerDelegate {
    /// Every line is at least `lineHeight` tall with the extra space split above and below the
    /// text (CSS half-leading); taller lines (images) keep their natural height.
    public nonisolated func layoutManager(_ layoutManager: NSLayoutManager, shouldSetLineFragmentRect lineFragmentRect: UnsafeMutablePointer<NSRect>, lineFragmentUsedRect: UnsafeMutablePointer<NSRect>, baselineOffset: UnsafeMutablePointer<CGFloat>, in textContainer: NSTextContainer, forGlyphRange glyphRange: NSRange) -> Bool {
        let line = KZTextStyleEditorLineHeight.value
        let natural = lineFragmentRect.pointee.height
        guard natural < line else { return false }
        let extra = line - natural
        lineFragmentRect.pointee.size.height = line
        lineFragmentUsedRect.pointee.size.height = line
        baselineOffset.pointee += extra / 2
        return true
    }
}

/// The editor line height, readable from the layout manager's nonisolated delegate callback.
enum KZTextStyleEditorLineHeight {
    static let value: CGFloat = 13 * 1.7
}
