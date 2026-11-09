import Foundation

/// Small RFC 4180 reader, including quoted newlines, doubled quotes and locale delimiters.
enum CSV {
    static func parse(_ text: String) -> [[String]] {
        let c = text.drop(while: { $0 == "\u{feff}" }).unicodeScalars.map { Character(String($0)) }; var counts: [Character:Int] = [",":0,";":0,"\t":0], quoted = false
        for ch in c { if ch == "\"" { quoted.toggle() }; if (ch == "\n" || ch == "\r") && !quoted { break }; if !quoted && counts[ch] != nil { counts[ch,default:0] += 1 } }
        let commas = counts[","] ?? 0, semis = counts[";"] ?? 0, tabs = counts["\t"] ?? 0
        let delimiter: Character = tabs > commas && tabs >= semis ? "\t" : semis > commas ? ";" : ","
        var rows: [[String]] = [], row: [String] = [], field = "", started = false, i = 0; quoted = false
        func finish() {
            if !row.isEmpty || started || !field.isEmpty { row.append(field); field = "" }; started = false
            if !row.isEmpty && !(row.count == 1 && row[0].isEmpty) { rows.append(row) }; row = []
        }
        while i < c.count {
            let ch = c[i]; i += 1
            if quoted {
                if ch == "\"" && i < c.count && c[i] == "\"" { field.append(ch); i += 1 }
                else if ch == "\"" { quoted = false } else { field.append(ch) }; continue
            }
            if ch == "\"" && !started { quoted = true; started = true }
            else if ch == delimiter { row.append(field); field = ""; started = false }
            else if ch == "\r" || ch == "\n" || ch == "\r\n" { if ch == "\r" && i < c.count && c[i] == "\n" { i += 1 }; finish() }
            else { field.append(ch); started = true }
        }; finish(); return rows
    }
}
