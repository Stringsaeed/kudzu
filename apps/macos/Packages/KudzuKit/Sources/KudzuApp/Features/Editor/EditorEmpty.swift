import KudzuDesign
import SwiftUI

public struct EditorEmpty: View {
    @Environment(AppModel.self) private var model
    @State private var error: String?
    public init() {}
    public var body: some View {
        VStack(spacing: 16) {
            Icon.cornerDownLeft.icon(20).foregroundStyle(Color.kz.mutedForeground)
                .frame(width: 48, height: 48).depth(.hairline, fill: Color.kz.muted, in: RoundedRectangle(cornerRadius: Radius.xl2))
            VStack(spacing: 4) {
                Text("No snippet selected").fontWeight(.medium)
                Text("Pick one from the list, or start a new one.").font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
            }
            Button {
                Task { do { try await model.newSnippet() } catch { self.error = error.localizedDescription } }
            } label: {
                HStack { Icon.plus.icon(14); Text("New snippet"); Shortcut("mod+n") }
            }.buttonStyle(.kz(size: .sm)).keyboardShortcut("n")
        }.padding(32).frame(maxWidth: .infinity, maxHeight: .infinity)
            .alert("Couldn't create snippet", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
                Button("OK", role: .cancel) { error = nil }
            } message: { Text(error ?? "") }
    }
}
