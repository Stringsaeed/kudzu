import KudzuCore
import KudzuDesign
import SwiftUI

/// Compatibility alias for `OnboardingScreen`.
public typealias OnboardingView = OnboardingScreen

/// First-run guide (`onboarding.tsx`). Access is skipped on Windows and Linux.
public struct OnboardingScreen: View {
    @Environment(AppModel.self) private var model

    @State private var step: OnboardingLogic.Step
    @State private var practiceText = ""
    @State private var espanso: [EspansoDetection] = []
    @State private var detection: Detection = .looking
    @State private var outcomes: [Source: OnboardingLogic.ImportOutcome] = [:]
    @State private var failures: [Source: String] = [:]
    @State private var completedEspanso: Set<String> = []
    @State private var imported = 0
    @State private var working: Source?
    @State private var picking = false
    @State private var finishing = false
    @State private var finishError: String?
    @State private var sparkle = 0
    @FocusState private var practiceFocused: Bool

    public init() {
        self.init(initialStep: .welcome)
    }

    init(initialStep: OnboardingLogic.Step) {
        self._step = State(initialValue: initialStep)
    }

    private enum Source: Hashable {
        case espanso, raycast, textexpander
    }

    private enum Detection {
        case looking, done, failed
    }

    private var steps: [OnboardingLogic.Step] {
        let planned = OnboardingLogic.steps(for: model.platform)
        return planned.contains(step) ? planned : planned + [step]
    }

    private var index: Int { OnboardingLogic.index(of: step, in: steps) }
    private var busy: Bool { finishing || picking || working != nil }
    private var espansoCount: Int { espanso.reduce(0) { $0 + $1.snippetCount } }

    // The web's layout: a scroller filling the window with the step column (`max-w-[480px] px-6
    // py-24`, so 432 pt of content) centred in it (`my-auto`), and the header and a 48 pt drag strip
    // laid over the top (`absolute`), taking no space.
    public var body: some View {
        ZStack {
            Color.kz.background
            atmosphere
            GeometryReader { geo in
                ScrollView {
                    column
                        .padding(.horizontal, 24)
                        .padding(.vertical, 96)
                        .frame(maxWidth: 480)
                        .frame(maxWidth: .infinity, minHeight: geo.size.height)
                }
                .scrollBounceBehavior(.basedOnSize)
            }
        }
        .overlay(alignment: .top) { WindowDragArea().frame(height: 48) }
        .overlay(alignment: .top) { header }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task { await detectEspanso() }
        // The step region takes focus on change, so VoiceOver reads "Setup: Access".
        .onChange(of: step) { _, next in
            AccessibilityNotification.Announcement("Setup: \(next.label)").post()
        }
        // The import step's `role="status"` line.
        .onChange(of: imported) { _, count in
            guard count > 0 else { return }
            AccessibilityNotification.Announcement("Imported \(count) snippet\(count == 1 ? "" : "s")").post()
        }
    }

    private var column: some View {
        VStack(spacing: 32) {
            // `key={step}` + `animate-rise`: the old step goes at once and the new one rises.
            stepBody
                .rise()
                .id(step)
            if let finishError {
                Text(finishError)
                    .font(.system(size: 12))
                    .foregroundStyle(.kz.destructive)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(Color.kz.mix(.destructive, alpha: 0.1), in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous))
            }
            progress
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Setup: \(step.label)")
    }

    /// `radial-gradient(ellipse at 50% 30%, iris 7%, transparent 60%)`. CSS sizes the ellipse to the
    /// farthest corner keeping the farthest-side aspect, so the gradient ends at 0.6·√2 × (half the
    /// width, 70% of the height) from the centre.
    private var atmosphere: some View {
        GeometryReader { geo in
            let w = max(1, geo.size.width), h = max(1, geo.size.height)
            let rx = 0.6 * 2.squareRoot() * 0.5 * w
            let ry = 0.6 * 2.squareRoot() * 0.7 * h
            RadialGradient(
                colors: [Color.kz.mix(.iris, alpha: 0.07), Color.clear],
                center: UnitPoint(x: 0.5, y: 0.3),
                startRadius: 0,
                endRadius: ry
            )
            .scaleEffect(x: rx / ry, y: 1, anchor: UnitPoint(x: 0.5, y: 0.3))
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private var header: some View {
        HStack {
            Wordmark(.small)
                .frame(width: 80)
                .foregroundStyle(.kz.mutedForeground)
                .opacity(step == .welcome ? 0 : 1)
                .allowsHitTesting(false)
                .accessibilityHidden(step == .welcome)
            Spacer()
            if index > 0, let previous = OnboardingLogic.neighbour(of: step, in: steps, offset: -1) {
                Button {
                    transitionTo(previous)
                } label: {
                    ButtonLabel("Back", icon: Icon.arrowLeft)
                }
                .buttonStyle(.kz(.ghost, size: .sm))
                .disabled(busy)
            }
        }
        .padding(.horizontal, 32)
        .padding(.top, 56)
    }

    @ViewBuilder
    private var stepBody: some View {
        switch step {
        case .welcome: welcome
        case .permissions: permissions
        case .importStep: importStep
        case .practice: practice
        }
    }

    private var welcome: some View {
        VStack(spacing: 24) {
            Wordmark(.full)
                .frame(width: 224)
                .padding(.bottom, 8)
            VStack(spacing: 8) {
                Text(BrandCopy.tagline)
                    .font(.system(size: 28, weight: .semibold))
                    .tracking(-0.7)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.kz.foreground)
                Text(BrandCopy.description)
                    .font(.system(size: 14))
                    .lineSpacing(5)
                    .multilineTextAlignment(.center)
                    .mutedForeground()
            }
            HStack(alignment: .center, spacing: 12) {
                Text(";sig")
                    .font(.kz.mono(12))
                    .foregroundStyle(.kz.irisForeground)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.kz.irisSoft, in: RoundedRectangle(cornerRadius: Radius.md, style: .continuous))
                Icon.arrowRight.icon(14).foregroundStyle(.kz.faint)
                VStack(alignment: .leading, spacing: 0) {
                    Text("Best regards,")
                    Text("Your Name").foregroundStyle(.kz.foreground)
                }
                .font(.system(size: 12))
                .mutedForeground()
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .depth(.card, fill: Color.kz.mix(.surface, alpha: 0.7), in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: Radius.xl, style: .continuous)
                    .strokeBorder(Color.kz.hairline, lineWidth: 1)
            }

            Button {
                advance()
            } label: {
                ButtonLabel("Get started", trailing: Icon.arrowRight)
            }
            .buttonStyle(.kz(.default, size: .xl))
            Text("A few small steps, then back to your words.")
                .font(.system(size: 11))
                .foregroundStyle(.kz.faint)
                .padding(.top, -12)
        }
        .frame(maxWidth: .infinity)
    }

    private var permissions: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Let Kudzu type for you")
                    .font(.system(size: 20, weight: .semibold))
                    .tracking(-0.5)
                    .foregroundStyle(.kz.foreground)
                Text("Allow access once in macOS. We’ll guide you through it and detect when you’re ready.")
                    .font(.system(size: 14))
                    .mutedForeground()
            }
            PermissionSetupView()
            HStack {
                Button("Skip for now") { advance() }
                    .buttonStyle(.kz(.ghost))
                Spacer()
                Button {
                    advance()
                } label: {
                    ButtonLabel("Continue", trailing: Icon.arrowRight)
                }
                .buttonStyle(.kz(.default))
                .disabled(!PermissionsLogic.isReady(model.status))
            }
        }
    }

    private var importStep: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Bring your snippets?")
                    .font(.system(size: 20, weight: .semibold))
                    .tracking(-0.5)
                    .foregroundStyle(.kz.foreground)
                Text("Switching from another expander? Forms, dates and clipboard variables come along; anything Kudzu can't run yet is kept as text. A backup is saved first.")
                    .font(.system(size: 14))
                    .mutedForeground()
            }
            VStack(spacing: 8) {
                sourceRow(
                    .espanso,
                    name: "Espanso",
                    detail: espansoDetail
                ) {
                    if espanso.isEmpty {
                        Button {
                            Task { await chooseEspansoFolder() }
                        } label: {
                            actionLabel("Choose folder…", working: working == .espanso)
                        }
                        .buttonStyle(.kz(.secondary, size: .sm, minWidth: Self.actionWidth))
                        .disabled(busy || detection == .looking)
                        .accessibilityLabel("Choose folder… for Espanso")
                    } else {
                        let title = espansoImported ? "Imported" : (failures[.espanso] == nil ? "Import" : "Retry")
                        Button {
                            Task { await importDetected() }
                        } label: {
                            actionLabel(title, working: working == .espanso, icon: Icon.download)
                        }
                        .buttonStyle(.kz(espansoImported ? .secondary : .default, size: .sm, minWidth: Self.actionWidth))
                        .disabled(busy || espansoImported)
                        .accessibilityLabel(espansoImported ? "Espanso imported" : "\(title) Espanso snippets")
                    }
                }
                sourceRow(.raycast, name: "Raycast", detail: DataActions.raycastHint) {
                    fileButton(.raycast, name: "Raycast")
                }
                sourceRow(.textexpander, name: "TextExpander", detail: DataActions.textExpanderHint) {
                    fileButton(.textexpander, name: "TextExpander")
                }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Import sources")
            HStack {
                Spacer()
                if imported > 0 {
                    Button {
                        advance()
                    } label: {
                        ButtonLabel("Continue", trailing: Icon.arrowRight)
                    }
                    .buttonStyle(.kz(.default))
                    .disabled(busy)
                } else {
                    Button("Skip for now") { advance() }
                        .buttonStyle(.kz(.ghost))
                        .disabled(busy)
                }
            }
        }
    }

    private var espansoDetail: String {
        switch detection {
        case .looking: return "Looking on this Mac…"
        case .failed where outcomes[.espanso] == nil:
            return "Couldn’t check this Mac. Choose your Espanso folder instead."
        case .done, .failed:
            if !espanso.isEmpty { return "Found \(espansoCount) snippets on this Mac." }
            if outcomes[.espanso] != nil { return "From the folder you chose." }
            return "Not found on this Mac. If you keep it elsewhere, choose its folder."
        }
    }

    private var espansoImported: Bool { outcomes[.espanso] != nil && failures[.espanso] == nil }

    /// `ACTION_WIDTH = "min-w-28"`: every import action is at least 112 pt wide.
    private static let actionWidth: CGFloat = 112

    /// An import action's label: the spinner while that source is working (`Loader2 animate-spin`),
    /// otherwise its icon, if any (`data-icon="inline-start"`).
    private func actionLabel(_ title: String, working: Bool, icon: Image? = nil) -> some View {
        HStack(spacing: ButtonSize.sm.gap) {
            if working {
                ProgressView()
                    .controlSize(.small)
                    .scaleEffect(ButtonSize.sm.iconSize / 16)
                    .frame(width: ButtonSize.sm.iconSize, height: ButtonSize.sm.iconSize)
            } else if let icon {
                icon.icon()
            }
            Text(title)
        }
        .padding(.leading, (working || icon != nil) ? -ButtonSize.sm.inlineIconInset : 0)
    }

    private func fileButton(_ source: Source, name: String) -> some View {
        let failed = failures[source] != nil
        let title = failed ? "Try again…" : "Choose file…"
        return Button {
            Task { await importFile(source) }
        } label: {
            actionLabel(title, working: working == source)
        }
        .buttonStyle(.kz(.secondary, size: .sm, minWidth: Self.actionWidth))
        .disabled(busy)
        .accessibilityLabel("\(title) for \(name)")
    }

    private func sourceRow<Control: View>(_ source: Source, name: String, detail: String, @ViewBuilder control: () -> Control) -> some View {
        let outcome = outcomes[source]
        return VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .center, spacing: 16) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(name)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(.kz.foreground)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(detail)
                        .font(.system(size: 12))
                        .mutedForeground()
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                control()
                    .fixedSize()
            }
            if let outcome {
                HStack(spacing: 6) {
                    if outcome.count > 0 { Icon.check.icon(14).foregroundStyle(.kz.success).checkIn() }
                    Text(outcome.count > 0 ? "Imported \(outcome.count) snippet\(outcome.count == 1 ? "" : "s")" : "No new snippets to add")
                        .font(.system(size: 12))
                        .foregroundStyle(outcome.count > 0 ? Color.kz.success : Color.kz.mutedForeground)
                }
            }
            if let outcome, outcome.changed > 0 {
                Text("\(outcome.changed) item\(outcome.changed == 1 ? "" : "s") need\(outcome.changed == 1 ? "s" : "") a look — e.g. \(outcome.firstChange ?? "")")
                    .font(.system(size: 12))
                    .mutedForeground()
            }
            if let failure = failures[source] {
                Text(failure)
                    .font(.system(size: 12))
                    .foregroundStyle(.kz.destructive)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .depth(.card, fill: Color.kz.mix(.surface, alpha: 0.7), in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: Radius.xl, style: .continuous)
                .strokeBorder(Color.kz.hairline, lineWidth: 1)
        }
    }

    private var practice: some View {
        let success = practiceText.contains(OnboardingLogic.practiceExpansion)
        return VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Try a preview")
                    .font(.system(size: 20, weight: .semibold))
                    .tracking(-0.5)
                    .foregroundStyle(.kz.foreground)
                HStack(spacing: 4) {
                    Text("Type")
                    Text(OnboardingLogic.practiceTrigger)
                        .font(.kz.mono(12))
                        .foregroundStyle(.kz.irisForeground)
                        .padding(.horizontal, 4)
                        .background(Color.kz.irisSoft, in: RoundedRectangle(cornerRadius: 4, style: .continuous))
                    Text("below to see how expansion feels.")
                }
                .font(.system(size: 14))
                .mutedForeground()
            }
            ZStack(alignment: .topLeading) {
                if practiceText.isEmpty {
                    Text("Type \(OnboardingLogic.practiceTrigger) here…")
                        .font(.system(size: 15))
                        .foregroundStyle(.kz.faint)
                        .padding(16)
                        .allowsHitTesting(false)
                }
                TextEditor(
                    text: Binding(
                        get: { practiceText },
                        set: { practiceText = OnboardingLogic.evaluatePracticeInput($0).text }
                    )
                )
                .font(.system(size: 15))
                .scrollContentBackground(.hidden)
                // `p-4`: the text view adds its own 5 pt line padding.
                .padding(.horizontal, 11)
                .padding(.vertical, 16)
                .focused($practiceFocused)
                .disabled(finishing)
                .accessibilityLabel("Practice field")
                // `useEffect(() => ref.current?.focus(), [])`: focus once the editor is in the window.
                .onAppear { DispatchQueue.main.async { practiceFocused = true } }
            }
            .frame(height: 112)
            .depth(.card, fill: Color.kz.surface, in: practiceShape)
            // `transition-shadow` to `0 0 0 1px var(--success), 0 0 24px -6px var(--success)`.
            .background {
                practiceShape
                    .fill(Color.kz.surface)
                    .shadow(color: Color.kz.success.opacity(success ? 0.6 : 0), radius: 9)
                    .kzAnimation(.kz.colors, value: success)
            }
            .overlay {
                practiceShape
                    .strokeBorder(Color.kz.success, lineWidth: 1)
                    .opacity(success ? 1 : 0)
                    .kzAnimation(.kz.colors, value: success)
            }

            if success {
                VStack(alignment: .leading, spacing: 16) {
                    HStack(spacing: 8) {
                        Icon.sparkles.icon(16)
                            .foregroundStyle(.kz.success)
                            .starBurst(sparkle)
                            .onAppear { sparkle += 1 }
                        Text("That's how a snippet expands.")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(.kz.success)
                    }
                    Text(model.platform == .web
                        ? "In the desktop app this works in every app."
                        : "When expansion is enabled, your library’s triggers work in Notes, Mail, your browser, and other apps.")
                        .font(.system(size: 12))
                        .mutedForeground()
                    HStack(spacing: 6) {
                        Text("Forgot a trigger? Press")
                        Shortcut(keys: Keys.acceleratorLabels(model.settings.paletteHotkey), small: false)
                        Text("anywhere to search your snippets.")
                    }
                    .font(.system(size: 12))
                    .mutedForeground()
                    HStack {
                        Spacer()
                        Button {
                            Task { await finish() }
                        } label: {
                            ButtonLabel(finishing ? "Opening…" : "Open library", trailing: Icon.arrowRight)
                        }
                        .buttonStyle(.kz(.default))
                        .disabled(finishing)
                    }
                }
                .rise()
            } else {
                HStack {
                    Spacer()
                    Button("Skip") { Task { await finish() } }
                        .buttonStyle(.kz(.ghost))
                        .disabled(finishing)
                }
            }
        }
    }

    private var practiceShape: RoundedRectangle { RoundedRectangle(cornerRadius: Radius.xl, style: .continuous) }

    private var progress: some View {
        HStack(spacing: 20) {
            ForEach(Array(steps.enumerated()), id: \.element.id) { offset, item in
                HStack(spacing: 6) {
                    if offset < index {
                        Icon.check.icon(12).foregroundStyle(.kz.iris).checkIn()
                    } else {
                        Circle()
                            .fill(offset == index ? Color.kz.iris : Color.kz.mix(.foreground, alpha: 0.15))
                            .frame(width: 6, height: 6)
                    }
                    Text(item.label)
                        .font(.system(size: 10))
                        .foregroundStyle(offset == index ? Color.kz.irisForeground : Color.kz.faint)
                        .kzAnimation(.kz.colors, value: offset == index)
                }
                .accessibilityAddTraits(offset == index ? .isSelected : AccessibilityTraits())
            }
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Setup progress")
    }

    private func advance() {
        if let next = OnboardingLogic.neighbour(of: step, in: steps, offset: 1) {
            transitionTo(next)
        } else {
            Task { await finish() }
        }
    }

    /// The step swaps without a transaction animation; the new step's `.rise()` is the motion.
    private func transitionTo(_ next: OnboardingLogic.Step) {
        step = next
    }

    private func finish() async {
        finishing = true
        finishError = nil
        defer { finishing = false }
        do {
            try await model.updateSettings(SettingsPatch { $0.onboardingCompleted = true })
        } catch {
            finishError = "We couldn’t finish setup. Your library is safe; please try again."
        }
    }

    private func detectEspanso() async {
        do {
            espanso = try await model.backend.detectEspanso()
            detection = .done
        } catch {
            detection = .failed
        }
    }

    private func run(_ source: Source, _ work: () async -> Void) async {
        working = source
        failures[source] = nil
        await work()
        working = nil
    }

    private func record(_ source: Source, _ summary: ImportSummary, accumulate: Bool = false) {
        imported += OnboardingLogic.newSnippetCount(summary)
        outcomes[source] = OnboardingLogic.recording(summary, onto: outcomes[source], accumulate: accumulate)
    }

    private func importDetected() async {
        await run(.espanso) {
            for source in espanso {
                if completedEspanso.contains(source.path) { continue }
                do {
                    let summary = try await model.importDetectedEspanso(path: source.path)
                    record(.espanso, summary, accumulate: true)
                    completedEspanso.insert(source.path)
                } catch {
                    failures[.espanso] = OnboardingLogic.espansoFailure(
                        AppModel.errorMessage(error),
                        completed: completedEspanso.count,
                        total: espanso.count
                    )
                    return
                }
            }
        }
    }

    private func chooseEspansoFolder() async {
        picking = true
        let path = DataActions.pickEspansoFolder()
        picking = false
        guard let path else { return }
        await run(.espanso) {
            do {
                let summary = try await model.backend.importEspanso(path: path)
                await model.refreshState()
                record(.espanso, summary)
            } catch {
                failures[.espanso] = AppModel.errorMessage(error)
            }
        }
    }

    private func importFile(_ source: Source) async {
        let kind: DataActions.ImportKind = source == .raycast ? .raycast : .textexpander
        picking = true
        let path = DataActions.pickImportFile(kind)
        picking = false
        guard let path else { return }
        await run(source) {
            do {
                let summary = try await model.backend.importFile(path: path)
                await model.refreshState()
                record(source, summary)
            } catch {
                failures[source] = AppModel.errorMessage(error)
            }
        }
    }
}

#Preview("Onboarding — Welcome") {
    let model = AppModel(backend: MockBackend(), isDev: true)
    OnboardingScreen(initialStep: .welcome)
        .environment(model)
        .frame(width: 820, height: 700)
        .task { await model.start() }
}
