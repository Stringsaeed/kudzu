import KudzuUI
import SwiftUI

struct EditorPage: GalleryPage {
    static let id = "editor"
    static let title = "Editor"
    static let section = GallerySection.structure
    static let icon = KzIconName.braces

    @State private var lastAction = "Choose a tag from the Insert menu."

    private var insertButton: some View {
        KzButton("Insert", variant: .ghost, size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Insert tag") {
            GalleryMenus.insertTag { lastAction = "Inserted \($0)" }
        }
    }

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Chrome only: an Insert slot and a hint bar over the content area. The real editor is the app's NSTextView chip editor; KzEditorStyle and KzEditorTag give it the content text style and the inline tag look. The text below is a reference KzEditorTextView.") {
            SpecimenSection("Frame", note: "Rest, then focused (the 2px vine-ink edge).") {
                VStack(spacing: KzSpace.s5) {
                    KzEditorFrame(minHeight: 100, insert: { insertButton }) {
                        KzEditorTextView([.text("Hi "), .tag("{clipboard}"), .text("! Thanks for reaching out on "), .tag("{date:%A}"), .text(".\nI'll follow up at "), .tag("{time}"), .text(".")], isEditable: false)
                    }
                    KzEditorFrame(minHeight: 50, isFocused: true, insert: { insertButton }) {
                        KzEditorTextView([.text("Focused "), .tag("{app}")], isEditable: false)
                    }
                    Text(lastAction).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                }
            }
            SpecimenSection("KzEditorStyle", note: "Content attributes: Geist 14, fixed 22pt lines, baseline offset that centres glyphs like CSS half-leading. Container inset 14 × 12, line-fragment padding 0.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s4) {
                        ForEach(["{date}", "{clipboard}", "{cursor}"], id: \.self) { tag in
                            Text(tag).kzTextStyle(.code).foregroundStyle(KzColor.vineInk)
                                .padding(.horizontal, 5).frame(height: 20)
                                .background(KzColor.vineSoft, in: RoundedRectangle(cornerRadius: KzRadius.xs))
                        }
                    }
                }
            }
        }
    }
}
