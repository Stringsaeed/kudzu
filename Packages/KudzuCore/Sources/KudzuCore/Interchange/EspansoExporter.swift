import Foundation
import Yams

/// Exports a `KudzuLibrary` as a dictionary of relative file paths to Espanso YAML match files.
public enum EspansoExporter {
    /// Converts `library` into a mapping of relative YAML file paths (e.g. `"base.yml"`, `"work/emails.yml"`) to YAML text.
    public static func export(_ library: KudzuLibrary) -> [String: String] {
        let groupsByID = Dictionary(uniqueKeysWithValues: library.groups.map { ($0.id, $0) })
        let snippetsByID = Dictionary(uniqueKeysWithValues: library.snippets.map { ($0.id, $0) })

        // Group snippets by their groupID (nil goes to "base")
        var snippetsByGroupID: [UUID?: [SnippetRecord]] = [:]
        for snippet in library.snippets {
            let gid = (snippet.groupID != nil && groupsByID[snippet.groupID!] != nil) ? snippet.groupID : nil
            snippetsByGroupID[gid, default: []].append(snippet)
        }

        // Compute relative file path for each group
        func relativeFilePath(for groupID: UUID?) -> String {
            guard let groupID, let group = groupsByID[groupID] else {
                return "base.yml"
            }
            var components: [String] = [sanitizePathComponent(group.name)]
            var visited: Set<UUID> = [groupID]
            var currentParentID = group.parentID
            while let pid = currentParentID, !visited.contains(pid), let parent = groupsByID[pid] {
                visited.insert(pid)
                components.insert(sanitizePathComponent(parent.name), at: 0)
                currentParentID = parent.parentID
            }
            let dirPart = components.dropLast().joined(separator: "/")
            let fileName = "\(components.last ?? "base").yml"
            return dirPart.isEmpty ? fileName : "\(dirPart)/\(fileName)"
        }

        // Collect ordered group keys: library.groups in order, plus nil if ungrouped snippets exist
        var orderedGroupIDs: [UUID?] = library.groups.map { Optional($0.id) }
        if snippetsByGroupID[nil] != nil {
            orderedGroupIDs.append(nil)
        }

        var outputFiles: [String: String] = [:]

        for gid in orderedGroupIDs {
            guard let groupSnippets = snippetsByGroupID[gid], !groupSnippets.isEmpty else {
                continue
            }
            let relPath = relativeFilePath(for: gid)
            var matchNodes: [Yams.Node] = []
            for snippet in groupSnippets {
                if let matchNode = makeMatchNode(for: snippet, snippetsByID: snippetsByID) {
                    matchNodes.append(matchNode)
                }
            }
            guard !matchNodes.isEmpty else { continue }

            let rootNode = Yams.Node.mapping(Yams.Node.Mapping([
                (Yams.Node("matches"), Yams.Node.sequence(Yams.Node.Sequence(matchNodes)))
            ]))
            if let yaml = try? Yams.serialize(node: rootNode) {
                outputFiles[relPath] = yaml
            }
        }

        return outputFiles
    }

    // MARK: - Match Node Construction

    private static func makeMatchNode(
        for snippet: SnippetRecord,
        snippetsByID: [UUID: SnippetRecord]
    ) -> Yams.Node? {
        guard !snippet.triggers.isEmpty else { return nil }

        var pairs: [(Yams.Node, Yams.Node)] = []

        if snippet.triggers.count == 1 {
            pairs.append((Yams.Node("trigger"), Yams.Node(snippet.triggers[0])))
        } else {
            let triggerNodes = snippet.triggers.map { Yams.Node($0) }
            pairs.append((Yams.Node("triggers"), Yams.Node.sequence(Yams.Node.Sequence(triggerNodes))))
        }

        if !snippet.name.isEmpty && snippet.name != snippet.triggers[0] {
            pairs.append((Yams.Node("label"), Yams.Node(snippet.name)))
        }

        let segments = TemplateTagFormatter.parseSegments(snippet.content)
        let hasFormTags = segments.contains { seg in
            if case .tag(let name, _, _) = seg {
                return name == "field" || name == "choice"
            }
            return false
        }
        let hasNonFormVariableTags = segments.contains { seg in
            if case .tag(let name, _, _) = seg {
                return name == "date" || name == "time" || name == "clipboard" || name == "snippet"
            }
            return false
        }

        // Use `form:` shorthand when the snippet contains form tags and no non-form variable tags
        if hasFormTags && !hasNonFormVariableTags {
            let (formString, formFieldsNode) = buildFormShorthand(from: segments)
            pairs.append((Yams.Node("form"), Yams.Node(formString)))
            if let formFieldsNode {
                pairs.append((Yams.Node("form_fields"), formFieldsNode))
            }
        } else {
            let (replaceString, varsNodes) = buildReplaceAndVars(from: segments, snippetsByID: snippetsByID)
            pairs.append((Yams.Node("replace"), Yams.Node(replaceString)))
            if !varsNodes.isEmpty {
                pairs.append((Yams.Node("vars"), Yams.Node.sequence(Yams.Node.Sequence(varsNodes))))
            }
        }

        if snippet.fireMode == .wholeWord {
            pairs.append((Yams.Node("word"), boolNode(true)))
        }

        if snippet.caseMode == .propagate {
            pairs.append((Yams.Node("propagate_case"), boolNode(true)))
        }

        if let insertion = snippet.insertion {
            switch insertion {
            case .paste:
                pairs.append((Yams.Node("force_mode"), Yams.Node("clipboard")))
            case .type:
                pairs.append((Yams.Node("force_mode"), Yams.Node("keys")))
            case .automatic:
                break
            }
        }

        return Yams.Node.mapping(Yams.Node.Mapping(pairs))
    }

    private static func buildFormShorthand(
        from segments: [TemplateTagFormatter.Segment]
    ) -> (String, Yams.Node?) {
        var formText = ""
        var fieldConfigs: [(String, Yams.Node)] = []
        var seenFields: Set<String> = []

        for segment in segments {
            switch segment {
            case .literal(let text):
                formText += escapeEspansoBraces(text)
            case .tag(let name, let attrs, let raw):
                switch name {
                case "cursor":
                    formText += "$|$"
                case "field":
                    let fieldName = attrs["name"] ?? "field"
                    formText += "[[\(fieldName)]]"
                    if !seenFields.contains(fieldName) {
                        seenFields.insert(fieldName)
                        var fieldPairs: [(Yams.Node, Yams.Node)] = []
                        if attrs["multiline"] == "true" {
                            fieldPairs.append((Yams.Node("multiline"), boolNode(true)))
                        }
                        if let def = attrs["default"], !def.isEmpty {
                            fieldPairs.append((Yams.Node("default"), Yams.Node(def)))
                        }
                        if !fieldPairs.isEmpty {
                            fieldConfigs.append((fieldName, Yams.Node.mapping(Yams.Node.Mapping(fieldPairs))))
                        }
                    }
                case "choice":
                    let fieldName = attrs["name"] ?? "choice"
                    formText += "[[\(fieldName)]]"
                    if !seenFields.contains(fieldName) {
                        seenFields.insert(fieldName)
                        let options = TemplateTagFormatter.splitChoiceOptions(attrs["options"] ?? "")
                        var fieldPairs: [(Yams.Node, Yams.Node)] = [
                            (Yams.Node("type"), Yams.Node("choice")),
                            (Yams.Node("values"), Yams.Node.sequence(Yams.Node.Sequence(options.map { Yams.Node($0) }))),
                        ]
                        if let def = attrs["default"], !def.isEmpty {
                            fieldPairs.append((Yams.Node("default"), Yams.Node(def)))
                        }
                        fieldConfigs.append((fieldName, Yams.Node.mapping(Yams.Node.Mapping(fieldPairs))))
                    }
                default:
                    formText += escapeEspansoBraces(raw)
                }
            }
        }

        let formFieldsNode: Yams.Node?
        if fieldConfigs.isEmpty {
            formFieldsNode = nil
        } else {
            formFieldsNode = Yams.Node.mapping(Yams.Node.Mapping(
                fieldConfigs.map { (Yams.Node($0.0), $0.1) }
            ))
        }
        return (formText, formFieldsNode)
    }

    private static func buildReplaceAndVars(
        from segments: [TemplateTagFormatter.Segment],
        snippetsByID: [UUID: SnippetRecord]
    ) -> (String, [Yams.Node]) {
        var replaceText = ""
        var varsNodes: [Yams.Node] = []
        var counter = 0

        // If a snippet mixes form fields and other variables, we also construct a `form1` verbose form variable
        var formLayoutParts: [String] = []
        var formFieldsPairs: [(Yams.Node, Yams.Node)] = []
        var seenFormFields: Set<String> = []

        for segment in segments {
            switch segment {
            case .literal(let text):
                replaceText += escapeEspansoBraces(text)
            case .tag(let name, let attrs, let raw):
                switch name {
                case "cursor":
                    replaceText += "$|$"

                case "date", "time":
                    counter += 1
                    let varName = "date_\(counter)"
                    let defaultICU = (name == "time") ? "HH:mm" : "yyyy-MM-dd"
                    let icuFormat = attrs["format"] ?? defaultICU
                    let strftimeFormat = StrftimeConverter.toStrftime(icuFormat)

                    var paramPairs: [(Yams.Node, Yams.Node)] = [
                        (Yams.Node("format"), Yams.Node(strftimeFormat))
                    ]
                    if let offsetStr = attrs["offset"],
                       let seconds = StrftimeConverter.kudzuOffsetToSeconds(offsetStr),
                       seconds != 0 {
                        paramPairs.append((Yams.Node("offset"), intNode(seconds)))
                    }
                    if let localeStr = attrs["locale"], !localeStr.isEmpty {
                        paramPairs.append((Yams.Node("locale"), Yams.Node(localeStr)))
                    }

                    let varNode = Yams.Node.mapping(Yams.Node.Mapping([
                        (Yams.Node("name"), Yams.Node(varName)),
                        (Yams.Node("type"), Yams.Node("date")),
                        (Yams.Node("params"), Yams.Node.mapping(Yams.Node.Mapping(paramPairs))),
                    ]))
                    varsNodes.append(varNode)
                    replaceText += "{{\(varName)}}"

                case "clipboard":
                    counter += 1
                    let varName = "clipboard_\(counter)"
                    let varNode = Yams.Node.mapping(Yams.Node.Mapping([
                        (Yams.Node("name"), Yams.Node(varName)),
                        (Yams.Node("type"), Yams.Node("clipboard")),
                    ]))
                    varsNodes.append(varNode)
                    replaceText += "{{\(varName)}}"

                case "snippet":
                    if let idStr = attrs["id"],
                       let uuid = UUID(uuidString: idStr),
                       let targetSnippet = snippetsByID[uuid],
                       let targetTrigger = targetSnippet.triggers.first {
                        counter += 1
                        let varName = "match_\(counter)"
                        let varNode = Yams.Node.mapping(Yams.Node.Mapping([
                            (Yams.Node("name"), Yams.Node(varName)),
                            (Yams.Node("type"), Yams.Node("match")),
                            (Yams.Node("params"), Yams.Node.mapping(Yams.Node.Mapping([
                                (Yams.Node("trigger"), Yams.Node(targetTrigger))
                            ]))),
                        ]))
                        varsNodes.append(varNode)
                        replaceText += "{{\(varName)}}"
                    } else {
                        replaceText += escapeEspansoBraces(raw)
                    }

                case "field":
                    let fieldName = attrs["name"] ?? "field"
                    replaceText += "{{form1.\(fieldName)}}"
                    if !seenFormFields.contains(fieldName) {
                        seenFormFields.insert(fieldName)
                        formLayoutParts.append("\(fieldName): [[\(fieldName)]]")
                        var fieldPairs: [(Yams.Node, Yams.Node)] = []
                        if attrs["multiline"] == "true" {
                            fieldPairs.append((Yams.Node("multiline"), boolNode(true)))
                        }
                        if let def = attrs["default"], !def.isEmpty {
                            fieldPairs.append((Yams.Node("default"), Yams.Node(def)))
                        }
                        if !fieldPairs.isEmpty {
                            formFieldsPairs.append((Yams.Node(fieldName), Yams.Node.mapping(Yams.Node.Mapping(fieldPairs))))
                        }
                    }

                case "choice":
                    let fieldName = attrs["name"] ?? "choice"
                    replaceText += "{{form1.\(fieldName)}}"
                    if !seenFormFields.contains(fieldName) {
                        seenFormFields.insert(fieldName)
                        formLayoutParts.append("\(fieldName): [[\(fieldName)]]")
                        let options = TemplateTagFormatter.splitChoiceOptions(attrs["options"] ?? "")
                        var fieldPairs: [(Yams.Node, Yams.Node)] = [
                            (Yams.Node("type"), Yams.Node("choice")),
                            (Yams.Node("values"), Yams.Node.sequence(Yams.Node.Sequence(options.map { Yams.Node($0) }))),
                        ]
                        if let def = attrs["default"], !def.isEmpty {
                            fieldPairs.append((Yams.Node("default"), Yams.Node(def)))
                        }
                        formFieldsPairs.append((Yams.Node(fieldName), Yams.Node.mapping(Yams.Node.Mapping(fieldPairs))))
                    }

                default:
                    replaceText += escapeEspansoBraces(raw)
                }
            }
        }

        if !seenFormFields.isEmpty {
            var formParams: [(Yams.Node, Yams.Node)] = [
                (Yams.Node("layout"), Yams.Node(formLayoutParts.joined(separator: "\n")))
            ]
            if !formFieldsPairs.isEmpty {
                formParams.append((Yams.Node("fields"), Yams.Node.mapping(Yams.Node.Mapping(formFieldsPairs))))
            }
            let formVarNode = Yams.Node.mapping(Yams.Node.Mapping([
                (Yams.Node("name"), Yams.Node("form1")),
                (Yams.Node("type"), Yams.Node("form")),
                (Yams.Node("params"), Yams.Node.mapping(Yams.Node.Mapping(formParams))),
            ]))
            varsNodes.insert(formVarNode, at: 0)
        }

        return (replaceText, varsNodes)
    }

    private static func escapeEspansoBraces(_ literal: String) -> String {
        literal.replacingOccurrences(of: "{{", with: "\\{\\{")
    }

    private static func sanitizePathComponent(_ name: String) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "/", with: "_")
        let withoutLeadingUnderscores = String(trimmed.drop(while: { $0 == "_" }))
        return withoutLeadingUnderscores.isEmpty ? "group" : withoutLeadingUnderscores
    }

    private static func boolNode(_ value: Bool) -> Yams.Node {
        Yams.Node.scalar(Yams.Node.Scalar(value ? "true" : "false", Yams.Tag(.bool)))
    }

    private static func intNode(_ value: Int) -> Yams.Node {
        Yams.Node.scalar(Yams.Node.Scalar(String(value), Yams.Tag(.int)))
    }
}
