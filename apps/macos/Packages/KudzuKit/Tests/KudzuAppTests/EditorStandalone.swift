#if KUDZU_EDITOR_STANDALONE
import Darwin
import Testing

/// Runs these same tests when unrelated feature work blocks the shared SwiftPM test target.
@main struct EditorStandalone {
    static func main() async {
        let result: CInt = await Testing.__swiftPMEntryPoint()
        exit(result)
    }
}
#endif
