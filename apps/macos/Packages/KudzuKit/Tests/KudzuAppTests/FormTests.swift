import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

/// Form behaviour ported from `form-panel.tsx`: which key submits, and how remembered values fill
/// the fields. `FormMemory` itself is tested in KudzuCore.
@Suite struct FormTests {
    @Test @MainActor func olderPreviewCannotReplaceNewerValues() async {
        let payload = request([FormFieldSpec(name: "Name", kind: .text)])
        let session = FormSession()
        let renderer = HeldFormRenderer()
        session.sync(payload, library: .empty)
        session.binding("Name").wrappedValue = "Older"
        let older = Task { await session.refreshPreview(payload) { await renderer.render($0) } }
        await renderer.waitFor("Older")
        session.binding("Name").wrappedValue = "Newer"
        let newer = Task { await session.refreshPreview(payload) { await renderer.render($0) } }
        await renderer.waitFor("Newer")
        await renderer.finish("Newer")
        await newer.value
        #expect(session.preview?.text == "Newer")
        await renderer.finish("Older")
        await older.value
        #expect(session.preview?.text == "Newer")
    }

    private let snippetID = UUID()

    private func request(_ fields: [FormFieldSpec], remembered: [String: String] = [:]) -> FormPayload {
        FormPayload(token: 1, snippetId: snippetID, snippetName: "Note", fields: fields, initialValues: remembered, content: "body")
    }

    @Test func returnSubmitsExceptInsideAMultilineField() {
        #expect(FormKeys.effect(.return(command: false), field: .text) == .submit)
        #expect(FormKeys.effect(.return(command: false), field: .choice) == .submit)
        #expect(FormKeys.effect(.return(command: false), field: .multiline) == .newline)
        #expect(FormKeys.effect(.return(command: true), field: .multiline) == .submit)
        #expect(FormKeys.effect(.return(command: false), field: nil) == .submit)
        #expect(FormKeys.effect(.escape, field: .multiline) == .cancel)
    }

    @Test func controlReturnSubmitsAMultilineField() {
        #expect(FormKeys.effect(.return(command: false, control: true), field: .multiline) == .submit)
    }

    @Test func focusedTextFieldKeepsAVisibleBoundary() {
        #expect(FormFieldChrome.needsFocusBoundary(focused: true))
        #expect(!FormFieldChrome.needsFocusBoundary(focused: false))
    }

    @Test func seedUsesPayloadValuesWithoutReinterpretingThem() {
        let fields = [
            FormFieldSpec(name: "Name", kind: .text, defaultValue: "Ada"),
            FormFieldSpec(name: "Platform", kind: .choice, options: ["macOS", "Linux"]),
        ]
        let values = ["Platform": "Windows", "Extra": "retained"]
        #expect(FormValues.seed(request(fields, remembered: values)) == values)
        #expect(FormValues.seed(request(fields)).isEmpty)
    }

    @Test func reportedHeightMatchesTheWebCap() {
        #expect(FormLayout.reportedHeight(content: 200, vibrant: false) == 224)
        #expect(FormLayout.reportedHeight(content: 200.2, vibrant: true) == 201)
        #expect(FormLayout.reportedHeight(content: 900, vibrant: true) == 640)
        #expect(FormLayout.reportedHeight(content: 0, vibrant: false) == 24)
    }
}

private actor HeldFormRenderer {
    private var renders: [String: CheckedContinuation<RenderResult, Never>] = [:]
    private var waiters: [String: CheckedContinuation<Void, Never>] = [:]

    func render(_ values: [String: String]) async -> RenderResult {
        let value = values["Name"] ?? ""
        return await withCheckedContinuation { continuation in
            renders[value] = continuation
            waiters.removeValue(forKey: value)?.resume()
        }
    }

    func waitFor(_ value: String) async {
        if renders[value] != nil { return }
        await withCheckedContinuation { waiters[value] = $0 }
    }

    func finish(_ value: String) {
        renders.removeValue(forKey: value)?.resume(returning: RenderResult(text: value))
    }
}
