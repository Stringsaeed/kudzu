// A ghost button that starts muted and takes another tone on hover: shadcn's ghost button with
// `className="text-muted-foreground"` (hover returns to `text-foreground`), or the chip editor's
// trash (`hover:text-destructive`). KZButtonStyle colours its label itself, so the tone has to
// be set on the label, from hover tracked on the whole button.

import KudzuDesign
import SwiftUI

struct MutedGhostButton<Label: View>: View {
    var size: ButtonSize
    var hoverTone: KZToken = .foreground
    var action: () -> Void
    @ViewBuilder var label: () -> Label
    @State private var hovered = false
    @Environment(\.isEnabled) private var isEnabled

    var body: some View {
        Button(action: action) {
            label().foregroundStyle(Color.kz[hovered && isEnabled ? hoverTone : .mutedForeground])
        }
        .buttonStyle(.kz(.ghost, size: size))
        .onHover { hovered = $0 }
        .kzAnimation(.kz.hover, value: hovered)
    }
}
