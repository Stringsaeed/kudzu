import AppKit

// Inspect the real AppKit HTML importer without touching the user's clipboard or documents.
struct Fixture: Decodable {
    let html: String
    let text: String
    let attachments: Int
}
let fixture = try JSONDecoder().decode(Fixture.self, from: FileHandle.standardInput.readDataToEndOfFile())
let result = try NSAttributedString(
    data: Data(fixture.html.utf8),
    options: [.documentType: NSAttributedString.DocumentType.html],
    documentAttributes: nil
)
precondition(result.string == fixture.text, "Unexpected native text: \(String(reflecting: result.string))")
var attachments = 0
result.enumerateAttribute(.attachment, in: NSRange(location: 0, length: result.length)) { value, _, _ in
    if let attachment = value as? NSTextAttachment {
        precondition((attachment.fileWrapper?.regularFileContents?.count ?? 0) > 0)
        attachments += 1
    }
}
precondition(attachments == fixture.attachments)
let font = result.attribute(.font, at: 0, effectiveRange: nil) as? NSFont
precondition(font.map { NSFontManager.shared.traits(of: $0).contains(.boldFontMask) } == true)
print("Native formatting, text length, and image attachments verified")
