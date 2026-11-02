import AppKit
import SwiftUI

/// One piece of editor content: literal text or an inline tag.
public enum KzEditorSegment: Equatable, Sendable {
    /// Literal text, set in `content`.
    case text(String)
    /// A dynamic tag such as `{clipboard}`, drawn with `KzEditorTag`.
    case tag(String)
}

/// A small `NSTextView` view that shows segments with `KzEditorStyle` and `KzEditorTag`.
///
/// It is the reference for how the app's chip editor adopts the look, and is good for galleries and
/// read-only previews. It is not the production editor: it has no slash menu, inspectors or
/// template serialisation.
public struct KzEditorTextView: View {
    private let segments: [KzEditorSegment]
    private let isEditable: Bool
    private let onFocusChange: (@MainActor (Bool) -> Void)?
    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    @Environment(\.colorScheme) private var colorScheme

    /// Creates the view.
    /// - Parameters:
    ///   - segments: The initial content.
    ///   - isEditable: `false` for a read-only preview.
    ///   - onFocusChange: Called when the text view gains or loses first-responder status; feed `KzEditorFrame.isFocused`.
    public init(_ segments: [KzEditorSegment], isEditable: Bool = true, onFocusChange: (@MainActor (Bool) -> Void)? = nil) {
        self.segments = segments
        self.isEditable = isEditable
        self.onFocusChange = onFocusChange
    }

    /// Content as an attributed string: text in the `content` style and tags as attachments.
    @MainActor
    public static func attributedString(for segments: [KzEditorSegment]) -> NSAttributedString {
        let result = NSMutableAttributedString()
        let attributes = KzEditorStyle.contentAttributes()
        for segment in segments {
            switch segment {
            case .text(let text):
                result.append(NSAttributedString(string: text, attributes: attributes))
            case .tag(let title):
                let attachment = NSTextAttachment()
                attachment.attachmentCell = KzEditorTagCell(title: title)
                let tag = NSMutableAttributedString(attachment: attachment)
                tag.addAttributes(attributes, range: NSRange(location: 0, length: tag.length))
                result.append(tag)
            }
        }
        return result
    }

    /// Builds the TextKit 1 text view the app's editor also uses, laid out for `width`.
    @MainActor
    public static func makeTextView(_ segments: [KzEditorSegment], width: CGFloat, isEditable: Bool = true) -> NSTextView {
        let storage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)
        let container = NSTextContainer(size: NSSize(width: width, height: .greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layoutManager.addTextContainer(container)
        let textView = FocusReportingTextView(frame: NSRect(x: 0, y: 0, width: width, height: 100), textContainer: container)
        textView.isRichText = true
        textView.isEditable = isEditable
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        KzEditorStyle.configure(textView)
        textView.textStorage?.setAttributedString(attributedString(for: segments))
        layoutManager.ensureLayout(for: container)
        return textView
    }

    /// The text view drawn into an image of `size`, in the given appearance (for offscreen snapshots).
    @MainActor
    public static func image(_ segments: [KzEditorSegment], size: CGSize, dark: Bool, scale: CGFloat = 2) -> NSImage {
        let textView = makeTextView(segments, width: size.width, isEditable: false)
        textView.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        textView.frame = NSRect(origin: .zero, size: size)
        let image = NSImage(size: size)
        if let rep = textView.bitmapImageRepForCachingDisplay(in: textView.bounds) {
            textView.cacheDisplay(in: textView.bounds, to: rep)
            image.addRepresentation(rep)
        }
        return image
    }

    // Offscreen rendering can't host AppKit views, so snapshots draw the text view into an image.
    public var body: some View {
        if snapshotRendering {
            GeometryReader { proxy in
                Image(nsImage: Self.image(segments, size: proxy.size, dark: colorScheme == .dark))
            }
        } else {
            Representable(segments: segments, isEditable: isEditable, onFocusChange: onFocusChange)
        }
    }

    private struct Representable: NSViewRepresentable {
        let segments: [KzEditorSegment]
        let isEditable: Bool
        let onFocusChange: (@MainActor (Bool) -> Void)?

        func makeNSView(context: Context) -> NSScrollView {
            let textView = KzEditorTextView.makeTextView(segments, width: 400, isEditable: isEditable)
            (textView as? FocusReportingTextView)?.onFocusChange = onFocusChange
            let scrollView = NSScrollView()
            scrollView.drawsBackground = false
            scrollView.hasVerticalScroller = true
            scrollView.autohidesScrollers = true
            scrollView.documentView = textView
            return scrollView
        }

        func updateNSView(_ scrollView: NSScrollView, context: Context) {
            guard let textView = scrollView.documentView as? FocusReportingTextView else { return }
            textView.onFocusChange = onFocusChange
            textView.isEditable = isEditable
        }
    }

    final class FocusReportingTextView: NSTextView {
        var onFocusChange: (@MainActor (Bool) -> Void)?

        override func becomeFirstResponder() -> Bool {
            let result = super.becomeFirstResponder()
            if result { onFocusChange?(true) }
            return result
        }

        override func resignFirstResponder() -> Bool {
            let result = super.resignFirstResponder()
            if result { onFocusChange?(false) }
            return result
        }
    }
}
