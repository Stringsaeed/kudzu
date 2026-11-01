import Foundation
import Yams

/// Imports Espanso match directories and YAML files into a `KudzuLibrary`.
public enum EspansoImporter {
    /// Default Espanso configuration locations on macOS (modern Application Support first, then legacy `~/.config/espanso`).
    public static func defaultConfigDirectories(homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser) -> [URL] {
        [
            homeDirectory.appendingPathComponent("Library/Application Support/espanso", isDirectory: true),
            homeDirectory.appendingPathComponent(".config/espanso", isDirectory: true),
        ]
    }

    /// Imports an Espanso directory (either the root config directory containing `match/` or a `match/` directory itself).
    public static func importDirectory(_ url: URL, fileManager: FileManager = .default) throws -> EspansoImportResult {
        let resolvedRoot = resolveMatchRoot(url, fileManager: fileManager)
        let discoveredFiles = try discoverAutoLoadedMatchFiles(in: resolvedRoot, fileManager: fileManager)
        return try performImport(
            initialFiles: discoveredFiles,
            baseDirectory: resolvedRoot,
            fileManager: fileManager
        )
    }

    /// Imports an explicit list of Espanso match YAML files.
    public static func importFiles(_ urls: [URL], fileManager: FileManager = .default) throws -> EspansoImportResult {
        let baseDir = commonParentDirectory(for: urls)
        return try performImport(
            initialFiles: urls,
            baseDirectory: baseDir,
            fileManager: fileManager
        )
    }

    // MARK: - Internal types

    struct RawFormFieldConfig {
        var type: String // "text", "choice", "list"
        var multiline: Bool
        var defaultValue: String?
        var values: [String]
    }

    struct RawVariable {
        var name: String
        var type: String
        var injectVars: Bool
        var params: [String: Any]
        var line: Int?
    }

    struct RawMatch {
        var triggers: [String]
        var regex: String?
        var label: String?
        var replace: String?
        var markdown: String?
        var html: String?
        var imagePath: String?
        var form: String?
        var formFields: [String: RawFormFieldConfig]
        var vars: [RawVariable]
        var word: Bool
        var leftWord: Bool
        var rightWord: Bool
        var propagateCase: Bool
        var forceMode: String?
        var matchIndex: Int
        var line: Int?
    }

    struct ParsedMatchFile {
        var url: URL
        var relativePath: String
        var directoryComponents: [String]
        var groupName: String
        var imports: [String]
        var globalVars: [RawVariable]
        var matches: [RawMatch]
    }

    // MARK: - Discovery & Loading

    private static func resolveMatchRoot(_ url: URL, fileManager: FileManager) -> URL {
        let matchSubdir = url.appendingPathComponent("match", isDirectory: true)
        var isDir: ObjCBool = false
        if fileManager.fileExists(atPath: matchSubdir.path, isDirectory: &isDir), isDir.boolValue {
            return matchSubdir.standardizedFileURL
        }
        return url.standardizedFileURL
    }

    private static func discoverAutoLoadedMatchFiles(in root: URL, fileManager: FileManager) throws -> [URL] {
        var results: [URL] = []

        func walk(directory: URL) throws {
            let contents = try fileManager.contentsOfDirectory(
                at: directory,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsHiddenFiles]
            )
            let sorted = contents.sorted {
                $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending
            }
            for item in sorted {
                let name = item.lastPathComponent
                // Files or folders prefixed with `_` are not auto-loaded unless explicitly imported.
                if name.hasPrefix("_") {
                    continue
                }
                let resourceValues = try item.resourceValues(forKeys: [.isDirectoryKey])
                if resourceValues.isDirectory == true {
                    try walk(directory: item)
                } else if isYAMLFile(item) {
                    results.append(item.standardizedFileURL)
                }
            }
        }

        if fileManager.fileExists(atPath: root.path) {
            try walk(directory: root)
        }
        return results
    }

    private static func isYAMLFile(_ url: URL) -> Bool {
        let ext = url.pathExtension.lowercased()
        return ext == "yml" || ext == "yaml"
    }

    private static func commonParentDirectory(for urls: [URL]) -> URL? {
        guard let first = urls.first else { return nil }
        var components = first.standardizedFileURL.deletingLastPathComponent().pathComponents
        for url in urls.dropFirst() {
            let other = url.standardizedFileURL.deletingLastPathComponent().pathComponents
            let commonCount = zip(components, other).prefix(while: { $0 == $1 }).count
            components = Array(components.prefix(commonCount))
        }
        guard !components.isEmpty else { return nil }
        return NSURL.fileURL(withPathComponents: components)?.standardizedFileURL
    }

    private static func computeRelativePath(for fileURL: URL, baseDirectory: URL?) -> (relative: String, dirComponents: [String]) {
        let standardizedFile = fileURL.standardizedFileURL
        if let base = baseDirectory?.standardizedFileURL {
            let baseComponents = base.pathComponents
            let fileComponents = standardizedFile.pathComponents
            if fileComponents.starts(with: baseComponents), fileComponents.count > baseComponents.count {
                let relComponents = Array(fileComponents.dropFirst(baseComponents.count))
                let dirComponents = Array(relComponents.dropLast())
                return (relComponents.joined(separator: "/"), dirComponents)
            }
        }
        return (standardizedFile.lastPathComponent, [])
    }

    // MARK: - Main Import Pipeline

    private static func performImport(
        initialFiles: [URL],
        baseDirectory: URL?,
        fileManager: FileManager
    ) throws -> EspansoImportResult {
        var warnings: [ImportWarning] = []
        var parsedFiles: [ParsedMatchFile] = []
        var visitedPaths: Set<String> = []

        func loadFile(_ fileURL: URL) {
            let stdURL = fileURL.standardizedFileURL
            let canonicalPath = stdURL.resolvingSymlinksInPath().path
            guard !visitedPaths.contains(canonicalPath) else { return }
            visitedPaths.insert(canonicalPath)

            let (relPath, dirComponents) = computeRelativePath(for: stdURL, baseDirectory: baseDirectory)
            guard fileManager.fileExists(atPath: stdURL.path) else {
                warnings.append(ImportWarning(
                    file: relPath,
                    message: "Match file not found at '\(relPath)'."
                ))
                return
            }

            let yamlText: String
            do {
                yamlText = try String(contentsOf: stdURL, encoding: .utf8)
            } catch {
                warnings.append(ImportWarning(
                    file: relPath,
                    message: "Failed to read file: \(error.localizedDescription)"
                ))
                return
            }

            guard let parsed = parseYAMLFile(
                yamlText: yamlText,
                url: stdURL,
                relativePath: relPath,
                directoryComponents: dirComponents,
                warnings: &warnings
            ) else {
                return
            }

            parsedFiles.append(parsed)

            // Follow `imports:` relative to the current file's directory
            let parentDir = stdURL.deletingLastPathComponent()
            for importPath in parsed.imports {
                let resolvedURL: URL
                if importPath.hasPrefix("/") {
                    resolvedURL = URL(fileURLWithPath: importPath)
                } else {
                    resolvedURL = parentDir.appendingPathComponent(importPath)
                }
                loadFile(resolvedURL)
            }
        }

        for url in initialFiles {
            loadFile(url)
        }

        // Collect all global_vars across all loaded files (later files can add globals; file-local global_vars take priority if duplicated)
        var libraryGlobalVars: [String: RawVariable] = [:]
        for file in parsedFiles {
            for gvar in file.globalVars {
                libraryGlobalVars[gvar.name] = gvar
            }
        }

        // Pass 1: Create GroupRecords and allocate SnippetRecord UUIDs + trigger map for nested `type: match` resolution.
        var groups: [GroupRecord] = []
        var directoryGroupIDs: [String: UUID] = [:]
        var nextSortIndexByParent: [UUID?: Int] = [:]

        func ensureDirectoryGroup(components: [String]) -> UUID? {
            guard !components.isEmpty else { return nil }
            var currentPath = ""
            var parentID: UUID? = nil
            for comp in components {
                currentPath = currentPath.isEmpty ? comp : "\(currentPath)/\(comp)"
                if let existingID = directoryGroupIDs[currentPath] {
                    parentID = existingID
                } else {
                    let sortIdx = nextSortIndexByParent[parentID, default: 0]
                    nextSortIndexByParent[parentID] = sortIdx + 1
                    let group = GroupRecord(
                        name: comp,
                        parentID: parentID,
                        sortIndex: sortIdx
                    )
                    groups.append(group)
                    directoryGroupIDs[currentPath] = group.id
                    parentID = group.id
                }
            }
            return parentID
        }

        struct PendingSnippet {
            var id: UUID
            var groupID: UUID
            var file: ParsedMatchFile
            var match: RawMatch
            var contentSource: String
        }

        var pendingSnippets: [PendingSnippet] = []
        var triggerToSnippetID: [String: UUID] = [:]

        for file in parsedFiles {
            let parentGroupID = ensureDirectoryGroup(components: file.directoryComponents)
            let dirPathKey = (file.directoryComponents + [file.groupName]).joined(separator: "/")

            let groupID: UUID
            if let existingID = directoryGroupIDs[dirPathKey] {
                groupID = existingID
            } else {
                let sortIdx = nextSortIndexByParent[parentGroupID, default: 0]
                nextSortIndexByParent[parentGroupID] = sortIdx + 1
                let group = GroupRecord(
                    name: file.groupName,
                    parentID: parentGroupID,
                    sortIndex: sortIdx
                )
                groups.append(group)
                directoryGroupIDs[dirPathKey] = group.id
                groupID = group.id
            }

            for match in file.matches {
                let primaryTrigger = match.triggers.first ?? match.regex

                // Skip regex matches with warning (v2)
                if let regex = match.regex {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: regex,
                        message: "Regex trigger '\(regex)' is not supported in v1 and was skipped."
                    ))
                    continue
                }

                guard !match.triggers.isEmpty else {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: nil,
                        message: "Match is missing a valid 'trigger' or 'triggers' list and was skipped."
                    ))
                    continue
                }

                // Skip image_path matches with warning (v2)
                if match.imagePath != nil {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: primaryTrigger,
                        message: "Image match ('image_path') is not supported in v1 and was skipped."
                    ))
                    continue
                }

                // Determine content source (replace, form, markdown, html)
                let contentSource: String
                if let replace = match.replace {
                    contentSource = replace
                } else if let form = match.form {
                    contentSource = form
                } else if let markdown = match.markdown {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: primaryTrigger,
                        message: "Markdown match imported as plain text; rich text formatting is not supported in v1."
                    ))
                    contentSource = markdown
                } else if let html = match.html {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: primaryTrigger,
                        message: "HTML match imported as plain text; rich text formatting is not supported in v1."
                    ))
                    contentSource = html
                } else {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: primaryTrigger,
                        message: "Match has no replacement content ('replace', 'form', 'markdown', or 'html') and was skipped."
                    ))
                    continue
                }

                if (match.leftWord || match.rightWord) && !match.word {
                    warnings.append(ImportWarning(
                        file: file.relativePath,
                        line: match.line,
                        matchIndex: match.matchIndex,
                        trigger: primaryTrigger,
                        message: "Partial word boundary ('left_word'/'right_word') mapped to whole-word mode."
                    ))
                }

                let snippetID = UUID()
                for trig in match.triggers {
                    if triggerToSnippetID[trig] == nil {
                        triggerToSnippetID[trig] = snippetID
                    }
                }

                pendingSnippets.append(PendingSnippet(
                    id: snippetID,
                    groupID: groupID,
                    file: file,
                    match: match,
                    contentSource: contentSource
                ))
            }
        }

        // Pass 2: Convert content templates and resolve variables & nested matches
        var snippets: [SnippetRecord] = []
        let timestamp = Date.now

        for pending in pendingSnippets {
            let match = pending.match
            let file = pending.file
            let primaryTrigger = match.triggers.first

            // Merge global_vars with file global_vars and match-local vars
            var effectiveVars = libraryGlobalVars
            for gvar in file.globalVars {
                effectiveVars[gvar.name] = gvar
            }
            for lvar in match.vars {
                if lvar.type == "global" {
                    // `type: global` in local vars explicitly references a global variable
                    continue
                }
                effectiveVars[lvar.name] = lvar
            }

            let convertedContent: String
            if match.form != nil && match.replace == nil {
                // Shorthand `form:` match
                convertedContent = convertFormShorthandText(
                    pending.contentSource,
                    formFields: match.formFields,
                    vars: effectiveVars,
                    triggerToSnippetID: triggerToSnippetID,
                    file: file.relativePath,
                    matchIndex: match.matchIndex,
                    line: match.line,
                    trigger: primaryTrigger,
                    warnings: &warnings
                )
            } else {
                convertedContent = convertReplaceText(
                    pending.contentSource,
                    vars: effectiveVars,
                    triggerToSnippetID: triggerToSnippetID,
                    file: file.relativePath,
                    matchIndex: match.matchIndex,
                    line: match.line,
                    trigger: primaryTrigger,
                    warnings: &warnings
                )
            }

            let name: String
            if let label = match.label?.trimmingCharacters(in: .whitespacesAndNewlines), !label.isEmpty {
                name = label
            } else {
                name = match.triggers[0]
            }

            let fireMode: FireMode = (match.word || match.leftWord || match.rightWord) ? .wholeWord : .immediate
            let caseMode: CaseMode = match.propagateCase ? .propagate : .exact
            let insertion: InsertionMethod?
            switch match.forceMode?.lowercased() {
            case "clipboard", "paste":
                insertion = .paste
            case "keys", "type":
                insertion = .type
            case "auto":
                insertion = .automatic
            default:
                insertion = nil
            }

            let snippet = SnippetRecord(
                id: pending.id,
                name: name,
                triggers: match.triggers,
                content: convertedContent,
                groupID: pending.groupID,
                fireMode: fireMode,
                caseMode: caseMode,
                insertion: insertion,
                rule: nil,
                isFavorite: false,
                isEnabled: true,
                createdAt: timestamp,
                updatedAt: timestamp
            )
            snippets.append(snippet)
        }

        let library = KudzuLibrary(
            formatVersion: KudzuLibrary.currentFormatVersion,
            groups: groups,
            snippets: snippets,
            usage: []
        )
        return EspansoImportResult(library: library, warnings: warnings)
    }

    // MARK: - Template Conversion

    private static func convertFormShorthandText(
        _ rawText: String,
        formFields: [String: RawFormFieldConfig],
        vars: [String: RawVariable],
        triggerToSnippetID: [String: UUID],
        file: String,
        matchIndex: Int,
        line: Int?,
        trigger: String?,
        warnings: inout [ImportWarning]
    ) -> String {
        // In `form:` shorthand, placeholders are `[[field_name]]`, plus `$|$` cursor hints and optional `{{var}}` injections.
        // We scan left-to-right for `$|$`, `[[...]]`, `\{\{`, and `{{...}}`.
        var output = ""
        let chars = Array(rawText)
        var i = 0

        while i < chars.count {
            // Cursor hint `$|$`
            if chars[i] == "$", i + 2 < chars.count, chars[i + 1] == "|", chars[i + 2] == "$" {
                output += TemplateTagFormatter.cursorTag
                i += 3
                continue
            }

            // Escaped Espanso brackets `\{\{`
            if chars[i] == "\\", i + 3 < chars.count,
               chars[i + 1] == "{", chars[i + 2] == "\\", chars[i + 3] == "{" {
                output += "\\{{"
                i += 4
                continue
            }

            // Form field placeholder `[[name]]`
            if chars[i] == "[", i + 1 < chars.count, chars[i + 1] == "[" {
                if let closeIdx = findClosingDoubleBracket(chars, from: i + 2) {
                    let fieldName = String(chars[(i + 2)..<closeIdx]).trimmingCharacters(in: .whitespaces)
                    if !fieldName.isEmpty {
                        output += makeFormFieldTag(
                            fieldName: fieldName,
                            config: formFields[fieldName],
                            vars: vars,
                            file: file,
                            matchIndex: matchIndex,
                            line: line,
                            trigger: trigger,
                            warnings: &warnings
                        )
                        i = closeIdx + 2
                        continue
                    }
                }
            }

            // Variable reference `{{var}}`
            if chars[i] == "{", i + 1 < chars.count, chars[i + 1] == "{" {
                if let closeIdx = findClosingDoubleBrace(chars, from: i + 2) {
                    let varRef = String(chars[(i + 2)..<closeIdx]).trimmingCharacters(in: .whitespaces)
                    output += resolveVariableReference(
                        varRef,
                        vars: vars,
                        triggerToSnippetID: triggerToSnippetID,
                        file: file,
                        matchIndex: matchIndex,
                        line: line,
                        trigger: trigger,
                        warnings: &warnings
                    )
                    i = closeIdx + 2
                    continue
                } else {
                    output += "\\{{"
                    i += 2
                    continue
                }
            }

            output.append(chars[i])
            i += 1
        }

        return output
    }

    private static func convertReplaceText(
        _ rawText: String,
        vars: [String: RawVariable],
        triggerToSnippetID: [String: UUID],
        file: String,
        matchIndex: Int,
        line: Int?,
        trigger: String?,
        warnings: inout [ImportWarning]
    ) -> String {
        var output = ""
        let chars = Array(rawText)
        var i = 0

        while i < chars.count {
            // Cursor hint `$|$`
            if chars[i] == "$", i + 2 < chars.count, chars[i + 1] == "|", chars[i + 2] == "$" {
                output += TemplateTagFormatter.cursorTag
                i += 3
                continue
                }

            // Escaped Espanso brackets `\{\{` -> Kudzu escaped `\{{`
            if chars[i] == "\\", i + 3 < chars.count,
               chars[i + 1] == "{", chars[i + 2] == "\\", chars[i + 3] == "{" {
                output += "\\{{"
                i += 4
                continue
            }

            // Also handle `\{{` already escaped with a single backslash
            if chars[i] == "\\", i + 2 < chars.count,
               chars[i + 1] == "{", chars[i + 2] == "{" {
                output += "\\{{"
                i += 3
                continue
            }

            // Variable reference `{{...}}`
            if chars[i] == "{", i + 1 < chars.count, chars[i + 1] == "{" {
                if let closeIdx = findClosingDoubleBrace(chars, from: i + 2) {
                    let varRef = String(chars[(i + 2)..<closeIdx]).trimmingCharacters(in: .whitespaces)
                    output += resolveVariableReference(
                        varRef,
                        vars: vars,
                        triggerToSnippetID: triggerToSnippetID,
                        file: file,
                        matchIndex: matchIndex,
                        line: line,
                        trigger: trigger,
                        warnings: &warnings
                    )
                    i = closeIdx + 2
                    continue
                } else {
                    output += "\\{{"
                    i += 2
                    continue
                }
            }

            output.append(chars[i])
            i += 1
        }

        return output
    }

    private static func resolveVariableReference(
        _ varRef: String,
        vars: [String: RawVariable],
        triggerToSnippetID: [String: UUID],
        file: String,
        matchIndex: Int,
        line: Int?,
        trigger: String?,
        warnings: inout [ImportWarning]
    ) -> String {
        // Check for verbose form reference: `formName.fieldName`
        if let dotIndex = varRef.firstIndex(of: ".") {
            let formVarName = String(varRef[..<dotIndex])
            let fieldName = String(varRef[varRef.index(after: dotIndex)...])
            if let formVar = vars[formVarName], formVar.type == "form" {
                let fieldsMap = parseFormFieldsMap(from: formVar.params["fields"])
                return makeFormFieldTag(
                    fieldName: fieldName,
                    config: fieldsMap[fieldName],
                    vars: vars,
                    file: file,
                    matchIndex: matchIndex,
                    line: formVar.line ?? line,
                    trigger: trigger,
                    warnings: &warnings
                )
            }
        }

        guard let variable = vars[varRef] else {
            warnings.append(ImportWarning(
                file: file,
                line: line,
                matchIndex: matchIndex,
                trigger: trigger,
                message: "Undefined variable '{{\(varRef)}}' kept as literal text."
            ))
            return "\\{{\(varRef)}}"
        }

        switch variable.type {
        case "date":
            let rawFormat = (variable.params["format"] as? String) ?? "%Y-%m-%d"
            let conversion = StrftimeConverter.toICU(rawFormat)
            if !conversion.unsupportedSpecifiers.isEmpty {
                let list = conversion.unsupportedSpecifiers.joined(separator: ", ")
                warnings.append(ImportWarning(
                    file: file,
                    line: variable.line ?? line,
                    matchIndex: matchIndex,
                    trigger: trigger,
                    message: "Unsupported strftime specifier(s) \(list) in date variable '\(variable.name)' kept as literal."
                ))
            }
            let offsetSeconds = parseIntParam(variable.params["offset"])
            let offsetStr = offsetSeconds.flatMap { StrftimeConverter.secondsToKudzuOffset($0) }
            let localeStr = variable.params["locale"] as? String
            if variable.params["tz"] != nil {
                warnings.append(ImportWarning(
                    file: file,
                    line: variable.line ?? line,
                    matchIndex: matchIndex,
                    trigger: trigger,
                    message: "Date variable '\(variable.name)' specifies 'tz' which is not supported in v1 and was ignored."
                ))
            }
            return TemplateTagFormatter.dateTag(
                format: conversion.icuFormat,
                offset: offsetStr,
                locale: localeStr
            )

        case "clipboard":
            return TemplateTagFormatter.clipboardTag()

        case "match":
            guard let nestedTrigger = variable.params["trigger"] as? String, !nestedTrigger.isEmpty else {
                warnings.append(ImportWarning(
                    file: file,
                    line: variable.line ?? line,
                    matchIndex: matchIndex,
                    trigger: trigger,
                    message: "Nested match variable '\(variable.name)' is missing 'params.trigger'; kept as literal text."
                ))
                return "\\{{\(varRef)}}"
            }
            if let targetID = triggerToSnippetID[nestedTrigger] {
                return TemplateTagFormatter.snippetTag(id: targetID)
            } else {
                warnings.append(ImportWarning(
                    file: file,
                    line: variable.line ?? line,
                    matchIndex: matchIndex,
                    trigger: trigger,
                    message: "Unresolved nested match trigger '\(nestedTrigger)' for variable '\(variable.name)'; kept as literal text."
                ))
                return "\\{{\(varRef)}}"
            }

        case "form":
            // Direct `{{formVar}}` reference (rare, usually `{{formVar.field}}`, but if layout is present, expand layout)
            if let layout = variable.params["layout"] as? String {
                let fieldsMap = parseFormFieldsMap(from: variable.params["fields"])
                return convertFormShorthandText(
                    layout,
                    formFields: fieldsMap,
                    vars: vars,
                    triggerToSnippetID: triggerToSnippetID,
                    file: file,
                    matchIndex: matchIndex,
                    line: variable.line ?? line,
                    trigger: trigger,
                    warnings: &warnings
                )
            }
            warnings.append(ImportWarning(
                file: file,
                line: variable.line ?? line,
                matchIndex: matchIndex,
                trigger: trigger,
                message: "Form variable '\(variable.name)' referenced directly without a field name; kept as literal text."
            ))
            return "\\{{\(varRef)}}"

        default:
            warnings.append(ImportWarning(
                file: file,
                line: variable.line ?? line,
                matchIndex: matchIndex,
                trigger: trigger,
                message: "Unsupported variable type '\(variable.type)' for variable '\(variable.name)'; kept as literal text."
            ))
            return "\\{{\(varRef)}}"
        }
    }

    private static func makeFormFieldTag(
        fieldName: String,
        config: RawFormFieldConfig?,
        vars: [String: RawVariable],
        file: String,
        matchIndex: Int,
        line: Int?,
        trigger: String?,
        warnings: inout [ImportWarning]
    ) -> String {
        guard let config else {
            return TemplateTagFormatter.fieldTag(name: fieldName)
        }
        switch config.type.lowercased() {
        case "choice", "list":
            return TemplateTagFormatter.choiceTag(
                name: fieldName,
                options: config.values,
                defaultValue: config.defaultValue
            )
        case "text", "":
            return TemplateTagFormatter.fieldTag(
                name: fieldName,
                defaultValue: config.defaultValue,
                multiline: config.multiline
            )
        default:
            warnings.append(ImportWarning(
                file: file,
                line: line,
                matchIndex: matchIndex,
                trigger: trigger,
                message: "Unknown form field control type '\(config.type)' for field '\(fieldName)'; imported as text field."
            ))
            return TemplateTagFormatter.fieldTag(
                name: fieldName,
                defaultValue: config.defaultValue,
                multiline: config.multiline
            )
        }
    }

    private static func findClosingDoubleBrace(_ chars: [Character], from start: Int) -> Int? {
        var i = start
        while i + 1 < chars.count {
            if chars[i] == "}", chars[i + 1] == "}" {
                return i
            }
            i += 1
        }
        return nil
    }

    private static func findClosingDoubleBracket(_ chars: [Character], from start: Int) -> Int? {
        var i = start
        while i + 1 < chars.count {
            if chars[i] == "]", chars[i + 1] == "]" {
                return i
            }
            i += 1
        }
        return nil
    }

    // MARK: - YAML Node Parsing

    private static func parseYAMLFile(
        yamlText: String,
        url: URL,
        relativePath: String,
        directoryComponents: [String],
        warnings: inout [ImportWarning]
    ) -> ParsedMatchFile? {
        let groupName = url.deletingPathExtension().lastPathComponent
        let rootNode: Yams.Node?
        do {
            rootNode = try Yams.compose(yaml: yamlText)
        } catch {
            warnings.append(ImportWarning(
                file: relativePath,
                message: "YAML syntax error: \(error.localizedDescription)"
            ))
            return nil
        }

        guard let root = rootNode else {
            // Empty YAML file
            return ParsedMatchFile(
                url: url,
                relativePath: relativePath,
                directoryComponents: directoryComponents,
                groupName: groupName,
                imports: [],
                globalVars: [],
                matches: []
            )
        }

        guard let mapping = root.mapping else {
            warnings.append(ImportWarning(
                file: relativePath,
                line: root.mark?.line,
                message: "Expected top-level YAML mapping in match file."
            ))
            return nil
        }

        var imports: [String] = []
        if let importsNode = mapping["imports"]?.sequence {
            for item in importsNode {
                if let str = item.string {
                    imports.append(str)
                }
            }
        }

        var globalVars: [RawVariable] = []
        if let globalVarsSeq = mapping["global_vars"]?.sequence {
            for varNode in globalVarsSeq {
                if let parsedVar = parseVariableNode(varNode) {
                    globalVars.append(parsedVar)
                }
            }
        }

        var matches: [RawMatch] = []
        if let matchesSeq = mapping["matches"]?.sequence {
            for (idx, matchNode) in matchesSeq.enumerated() {
                if let parsedMatch = parseMatchNode(matchNode, index: idx) {
                    matches.append(parsedMatch)
                } else {
                    warnings.append(ImportWarning(
                        file: relativePath,
                        line: matchNode.mark?.line,
                        matchIndex: idx,
                        message: "Malformed match entry at index \(idx) was skipped."
                    ))
                }
            }
        }

        return ParsedMatchFile(
            url: url,
            relativePath: relativePath,
            directoryComponents: directoryComponents,
            groupName: groupName,
            imports: imports,
            globalVars: globalVars,
            matches: matches
        )
    }

    private static func parseMatchNode(_ node: Yams.Node, index: Int) -> RawMatch? {
        guard let map = node.mapping else { return nil }

        var triggers: [String] = []
        if let singleTrigger = scalarString(map["trigger"]), !singleTrigger.isEmpty {
            triggers.append(singleTrigger)
        }
        if let triggersSeq = map["triggers"]?.sequence {
            for item in triggersSeq {
                if let t = scalarString(item), !t.isEmpty {
                    triggers.append(t)
                }
            }
        }

        let regex = scalarString(map["regex"])
        let label = scalarString(map["label"])
        let replace = scalarString(map["replace"])
        let markdown = scalarString(map["markdown"])
        let html = scalarString(map["html"])
        let imagePath = scalarString(map["image_path"])
        let form = scalarString(map["form"])

        let word = scalarBool(map["word"]) ?? false
        let leftWord = scalarBool(map["left_word"]) ?? false
        let rightWord = scalarBool(map["right_word"]) ?? false
        let propagateCase = scalarBool(map["propagate_case"]) ?? false
        let forceMode = scalarString(map["force_mode"])

        var vars: [RawVariable] = []
        if let varsSeq = map["vars"]?.sequence {
            for vNode in varsSeq {
                if let parsedVar = parseVariableNode(vNode) {
                    vars.append(parsedVar)
                }
            }
        }

        let formFields = parseFormFieldsNode(map["form_fields"])

        return RawMatch(
            triggers: triggers,
            regex: regex,
            label: label,
            replace: replace,
            markdown: markdown,
            html: html,
            imagePath: imagePath,
            form: form,
            formFields: formFields,
            vars: vars,
            word: word,
            leftWord: leftWord,
            rightWord: rightWord,
            propagateCase: propagateCase,
            forceMode: forceMode,
            matchIndex: index,
            line: node.mark?.line
        )
    }

    private static func parseVariableNode(_ node: Yams.Node) -> RawVariable? {
        guard let map = node.mapping,
              let name = scalarString(map["name"]),
              let type = scalarString(map["type"]) else {
            return nil
        }
        let injectVars = scalarBool(map["inject_vars"]) ?? true
        var params: [String: Any] = [:]
        if let paramsNode = map["params"], let dict = nodeToSwiftValue(paramsNode) as? [String: Any] {
            params = dict
        }
        return RawVariable(
            name: name,
            type: type,
            injectVars: injectVars,
            params: params,
            line: node.mark?.line
        )
    }

    private static func parseFormFieldsNode(_ node: Yams.Node?) -> [String: RawFormFieldConfig] {
        guard let node, let dict = nodeToSwiftValue(node) as? [String: Any] else {
            return [:]
        }
        return parseFormFieldsMap(from: dict)
    }

    private static func parseFormFieldsMap(from anyValue: Any?) -> [String: RawFormFieldConfig] {
        guard let dict = anyValue as? [String: Any] else { return [:] }
        var result: [String: RawFormFieldConfig] = [:]
        for (fieldName, rawConfig) in dict {
            guard let configMap = rawConfig as? [String: Any] else { continue }
            let type = (configMap["type"] as? String) ?? "text"
            let multiline = (configMap["multiline"] as? Bool) ?? false
            let defaultValue = configMap["default"].flatMap { valueToString($0) }
            let trimValues = (configMap["trim_string_values"] as? Bool) ?? true

            var values: [String] = []
            if let arr = configMap["values"] as? [Any] {
                values = arr.compactMap { valueToString($0) }
            } else if let strValues = configMap["values"] as? String {
                let lines = strValues.components(separatedBy: .newlines)
                for line in lines {
                    let processed = trimValues ? line.trimmingCharacters(in: .whitespaces) : line
                    if !processed.isEmpty {
                        values.append(processed)
                    }
                }
            }

            result[fieldName] = RawFormFieldConfig(
                type: type,
                multiline: multiline,
                defaultValue: defaultValue,
                values: values
            )
        }
        return result
    }

    private static func nodeToSwiftValue(_ node: Yams.Node) -> Any {
        switch node {
        case .scalar(let scalar):
            if let b = scalarBool(node) { return b }
            if let i = Int(scalar.string) { return i }
            return scalar.string
        case .mapping(let mapping):
            var dict: [String: Any] = [:]
            for (k, v) in mapping {
                if let key = scalarString(k) {
                    dict[key] = nodeToSwiftValue(v)
                }
            }
            return dict
        case .sequence(let sequence):
            return sequence.map { nodeToSwiftValue($0) }
        case .alias:
            return ""
        }
    }

    private static func scalarString(_ node: Yams.Node?) -> String? {
        guard let scalar = node?.scalar else { return nil }
        return scalar.string
    }

    private static func scalarBool(_ node: Yams.Node?) -> Bool? {
        guard let str = node?.scalar?.string.lowercased() else { return nil }
        switch str {
        case "true", "yes": return true
        case "false", "no": return false
        default: return nil
        }
    }

    private static func parseIntParam(_ any: Any?) -> Int? {
        if let i = any as? Int { return i }
        if let str = any as? String { return Int(str.trimmingCharacters(in: .whitespaces)) }
        return nil
    }

    private static func valueToString(_ any: Any) -> String? {
        if let s = any as? String { return s }
        if let i = any as? Int { return String(i) }
        if let d = any as? Double { return String(d) }
        if let b = any as? Bool { return String(b) }
        return nil
    }
}
