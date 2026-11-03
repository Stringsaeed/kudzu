import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Honest status of the Onboarding Step 3 practice card.
enum OnboardingPracticeState: Equatable, Sendable {
    /// Live expansion succeeded through the real engine.
    case verified
    /// User explicitly previewed sample output without claiming live expansion works.
    case demoPreview(blocker: String?)
    /// Live expansion is ready and waiting for the user to type `;kudzu`.
    case awaitingLiveInput
    /// Live expansion is blocked (permissions missing, paused, Secure Input, or inactive tap).
    case blocked(reason: String, typedTriggerWithoutExpansion: Bool)

    static func evaluate(
        practiceText: String,
        readiness: ExpansionReadiness,
        verifiedExpansionCount: Int,
        baselineCount: Int,
        initialCelebrated: Bool = false,
        isDemoPreview: Bool = false
    ) -> OnboardingPracticeState {
        if initialCelebrated || (readiness.isReady && verifiedExpansionCount > baselineCount && practiceText.contains("Kudzu is working!")) {
            return .verified
        }
        if isDemoPreview {
            return .demoPreview(blocker: readiness.blockerSummary)
        }
        if let blocker = readiness.blockerSummary {
            let typedTrigger = practiceText.range(of: ImportCoordinator.sampleTrigger, options: .caseInsensitive) != nil
            return .blocked(reason: blocker, typedTriggerWithoutExpansion: typedTrigger)
        }
        return .awaitingLiveInput
    }
}

/// First-launch flow: permissions, Espanso import, and a live practice field.
struct OnboardingView: View {
    enum Step: Int, CaseIterable, Identifiable {
        case permissions = 0
        case importEspanso = 1
        case tryIt = 2

        var id: Int { rawValue }

        var title: String {
            switch self {
            case .permissions: "Permissions"
            case .importEspanso: "Import from Espanso"
            case .tryIt: "Try it"
            }
        }
    }

    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var currentStep: Step
    @State private var practiceText: String
    @State private var initialCelebrated: Bool
    @State private var isDemoPreview = false
    @State private var baselinePracticeCount = 0
    @State private var starterError: String?
    @FocusState private var practiceFocused: Bool

    init(initialStep: Step = .permissions, initialPracticeText: String = "", initialCelebrated: Bool = false) {
        _currentStep = State(initialValue: initialStep)
        _practiceText = State(initialValue: initialPracticeText)
        _initialCelebrated = State(initialValue: initialCelebrated)
    }

    private var practiceState: OnboardingPracticeState {
        OnboardingPracticeState.evaluate(
            practiceText: practiceText,
            readiness: model.engine.readiness,
            verifiedExpansionCount: model.engine.verifiedPracticeExpansionCount,
            baselineCount: baselinePracticeCount,
            initialCelebrated: initialCelebrated,
            isDemoPreview: isDemoPreview
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s5) {
            headerSection
            Rectangle().fill(KzColor.line).frame(height: 1)

            Group {
                switch currentStep {
                case .permissions:
                    permissionsStep
                case .importEspanso:
                    importStep
                case .tryIt:
                    tryItStep
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            Rectangle().fill(KzColor.line).frame(height: 1)
            footerNavigation
        }
        .padding(.horizontal, KzSpace.s6)
        .padding(.top, KzSpace.s3)
        .padding(.bottom, KzSpace.s6)
        .frame(width: 600, height: 520)
        // The hidden titlebar sits above the content; fill it with the same paper so the window reads
        // as one surface, with only the traffic lights at its corner.
        .background(KzColor.surface.ignoresSafeArea())
        .background(WelcomeWindowChrome())
        .onChange(of: currentStep) { _, newStep in
            model.engine.allowsPracticeInOwnWindow = (newStep == .tryIt && practiceFocused)
            if newStep == .tryIt {
                baselinePracticeCount = model.engine.verifiedPracticeExpansionCount
                model.engine.refreshNow()
            }
        }
        .onChange(of: practiceFocused) { _, focused in
            model.engine.allowsPracticeInOwnWindow = currentStep == .tryIt && focused
        }
        .onDisappear {
            model.engine.allowsPracticeInOwnWindow = false
        }
        .task {
            NSApplication.shared.activate()
            model.engine.allowsPracticeInOwnWindow = (currentStep == .tryIt && practiceFocused)
            baselinePracticeCount = model.engine.verifiedPracticeExpansionCount
            while !Task.isCancelled {
                model.permissions.refresh()
                if model.permissions.accessibility || model.permissions.inputMonitoring {
                    model.engine.start()
                }
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    // MARK: - Header & Progress Indicator

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: KzSpace.s4) {
            HStack(spacing: KzSpace.s3) {
                appIconView
                VStack(alignment: .leading, spacing: 2) {
                    Text("Welcome to Kudzu")
                        .kzTextStyle(.titleLg)
                        .foregroundStyle(KzColor.ink)
                    Text("Step \(currentStep.rawValue + 1) of \(Step.allCases.count) — \(currentStep.title)")
                        .kzTextStyle(.callout)
                        .foregroundStyle(KzColor.inkMuted)
                }
                Spacer()
            }

            HStack(spacing: KzSpace.s2) {
                ForEach(Step.allCases) { step in
                    VStack(alignment: .leading, spacing: KzSpace.s1) {
                        Capsule()
                            .fill(step.rawValue <= currentStep.rawValue ? KzColor.vine : KzColor.surfaceSunken)
                            .frame(height: 5)
                        Text("\(step.rawValue + 1). \(step.title)")
                            .kzTextStyle(step == currentStep ? .bodyStrong : .caption)
                            .foregroundStyle(step == currentStep ? KzColor.ink : KzColor.inkMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }

    @ViewBuilder
    private var appIconView: some View {
        if let image = Self.loadAppIcon() {
            Image(nsImage: image)
                .resizable()
                .interpolation(.high)
                .frame(width: 48, height: 48)
        } else {
            ZStack {
                RoundedRectangle(cornerRadius: KzRadius.md, style: .continuous)
                    .fill(KzColor.vineSoft)
                    .frame(width: 44, height: 44)
                KzIcon(.leaf, size: 22)
                    .foregroundStyle(KzColor.vineInk)
            }
        }
    }

    private static func loadAppIcon() -> NSImage? {
        // The compiled app icon (from design/app-icon/Kudzu.icon), so the welcome window always matches the Dock.
        NSApp.applicationIconImage
    }

    // MARK: - Step 1: Permissions

    private var permissionsStep: some View {
        VStack(alignment: .leading, spacing: KzSpace.s4) {
            KzSettingsGroup(
                "Privacy permissions",
                description: "Kudzu needs two macOS privacy permissions to detect your triggers and expand snippets."
            ) {
                KzSettingsRow(
                    description: "So Kudzu can notice your triggers as you type.",
                    icon: .keyboard
                ) {
                    HStack(spacing: KzSpace.s2) {
                        Text("Input Monitoring")
                        KzBadge(
                            model.permissions.inputMonitoring ? "Granted" : "Needed",
                            tone: model.permissions.inputMonitoring ? .vine : .warning
                        )
                    }
                } control: {
                    if model.permissions.inputMonitoring {
                        HStack(spacing: KzSpace.s1) {
                            KzIcon(.check, size: 14)
                                .foregroundStyle(KzColor.vineInk)
                            Text("Active")
                                .kzTextStyle(.callout)
                                .foregroundStyle(KzColor.vineInk)
                        }
                    } else {
                        KzButton("Grant…", variant: .primary, size: .sm) {
                            model.permissions.request(.inputMonitoring)
                        }
                    }
                }

                KzSettingsRow(
                    description: "So Kudzu can type the expansion and place forms at your cursor. Post Event: \(model.permissions.postEvent ? "Granted" : "Covered by Accessibility grant").",
                    icon: .shield
                ) {
                    HStack(spacing: KzSpace.s2) {
                        Text("Accessibility")
                        KzBadge(
                            model.permissions.accessibility ? "Granted" : "Needed",
                            tone: model.permissions.accessibility ? .vine : .warning
                        )
                    }
                } control: {
                    if model.permissions.accessibility {
                        HStack(spacing: KzSpace.s1) {
                            KzIcon(.check, size: 14)
                                .foregroundStyle(KzColor.vineInk)
                            Text("Active")
                                .kzTextStyle(.callout)
                                .foregroundStyle(KzColor.vineInk)
                        }
                    } else {
                        KzButton("Grant…", variant: .primary, size: .sm) {
                            model.permissions.request(.accessibility)
                        }
                    }
                }
            }

            #if APPSTORE
            let noticeShape = RoundedRectangle(cornerRadius: KzRadius.md, style: .continuous)
            HStack(alignment: .top, spacing: KzSpace.s3) {
                KzIcon(.shield, size: 16)
                    .foregroundStyle(KzColor.inkMuted)
                VStack(alignment: .leading, spacing: KzSpace.s1) {
                    Text("Sandboxed Mac App Store build")
                        .kzTextStyle(.bodyStrong)
                        .foregroundStyle(KzColor.ink)
                    Text("This build runs inside the macOS App Sandbox. Global keystroke monitoring and synthetic event posting require explicit Input Monitoring and Accessibility approval in System Settings, and reading external folders (such as Espanso configs) requires selecting the folder manually.")
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(KzSpace.s3)
            .background(KzColor.surfaceRaised, in: noticeShape)
            .overlay(noticeShape.strokeBorder(KzColor.line))
            #endif
        }
    }

    // MARK: - Step 2: Import from Espanso

    private var importStep: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: KzSpace.s3) {
                Text("Already using Espanso? Import your existing YAML matches, variables, and forms in one click, or skip this step.")
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)

                EspansoImportView()
            }
        }
    }

    // MARK: - Step 3: Try It

    private var tryItStep: some View {
        VStack(alignment: .leading, spacing: KzSpace.s4) {
            Text("We’ve added a “Getting Started” group with a few starter snippets (;kudzu, ;date, ;sig). Give it a try below.")
                .kzTextStyle(.callout)
                .foregroundStyle(KzColor.inkMuted)

            let cardShape = RoundedRectangle(cornerRadius: KzRadius.lg, style: .continuous)
            VStack(alignment: .leading, spacing: KzSpace.s3) {
                KzField("Type ;kudzu here") {
                    KzTextInput("Type ;kudzu here", text: $practiceText, focus: $practiceFocused)
                        .onChange(of: practiceText) { _, newValue in
                            if !newValue.contains(ImportCoordinator.sampleExpansion.trimmingCharacters(in: .whitespaces)) {
                                isDemoPreview = false
                            }
                        }
                }

                practiceStatusRow
            }
            .padding(KzSpace.s4)
            .background(KzColor.surfaceRaised, in: cardShape)
            .overlay(cardShape.strokeBorder(KzColor.line))

            KzSettingsGroup("Starter snippets in “Getting Started”") {
                starterRow(trigger: ";kudzu", expansion: ImportCoordinator.sampleExpansion)
                starterRow(trigger: ";date", expansion: "Today’s formatted date")
                starterRow(trigger: ";sig", expansion: "Sample two-line signature")
            }

            if let starterError {
                Text(starterError)
                    .kzTextStyle(.caption)
                    .foregroundStyle(KzColor.danger)
            }
        }
        .onAppear {
            model.engine.allowsPracticeInOwnWindow = practiceFocused
            baselinePracticeCount = model.engine.verifiedPracticeExpansionCount
            do {
                _ = try ImportCoordinator.ensureGettingStartedSnippets(in: model.store)
                model.engine.refreshNow()
            } catch {
                starterError = error.localizedDescription
            }
        }
    }

    @ViewBuilder
    private var practiceStatusRow: some View {
        switch practiceState {
        case .verified:
            HStack(spacing: KzSpace.s2) {
                KzBadge("Working", tone: .vine)
                Text("Your trigger expanded into “\(ImportCoordinator.sampleExpansion)”.")
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)
            }
            .transition(.opacity)
        case .demoPreview(let blocker):
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                HStack(spacing: KzSpace.s2) {
                    KzBadge("Demo Preview — Not Verified", tone: .warning)
                    Text("Simulated sample output only. Live expansion was not used.")
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                }
                if let blocker {
                    blockerActionsRow(reason: blocker)
                }
            }
        case .awaitingLiveInput:
            Text("Tip: Type ;kudzu in the field above to watch it expand into “\(ImportCoordinator.sampleExpansion)”.")
                .kzTextStyle(.caption)
                .foregroundStyle(KzColor.inkMuted)
        case .blocked(let reason, let typedTriggerWithoutExpansion):
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                HStack(spacing: KzSpace.s2) {
                    KzBadge(typedTriggerWithoutExpansion ? "Not Expanded" : "Live Expansion Off", tone: .warning)
                    Text(reason)
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                }
                blockerActionsRow(reason: reason)
            }
        }
    }

    private func blockerActionsRow(reason: String) -> some View {
        HStack(spacing: KzSpace.s2) {
            if case .missingPermissions = model.engine.readiness {
                KzButton("Configure Permissions", variant: .secondary, size: .sm) {
                    currentStep = .permissions
                }
            }
            if case .paused = model.engine.readiness {
                KzButton("Resume Kudzu", variant: .secondary, size: .sm) {
                    model.engine.isPaused = false
                    model.engine.refreshNow()
                }
            }
            KzButton("Recheck", variant: .secondary, size: .sm) {
                model.engine.refreshNow()
            }
            KzButton("Preview Demo Only", variant: .ghost, size: .sm) {
                isDemoPreview = true
                initialCelebrated = false
                practiceText = ImportCoordinator.sampleExpansion
            }
        }
    }

    private func starterRow(trigger: String, expansion: String) -> some View {
        HStack(spacing: KzSpace.s3) {
            KzTriggerChip(trigger)
            Text(expansion)
                .kzTextStyle(.callout)
                .foregroundStyle(KzColor.inkMuted)
            Spacer()
        }
        .padding(.horizontal, KzSpace.s4)
        .padding(.vertical, KzSpace.s2)
    }

    // MARK: - Footer Navigation

    private var footerNavigation: some View {
        HStack(spacing: KzSpace.s3) {
            if currentStep != .permissions {
                KzButton("Back", variant: .secondary) {
                    if let previous = Step(rawValue: currentStep.rawValue - 1) {
                        currentStep = previous
                    }
                }
            }

            Spacer()

            if currentStep != .tryIt {
                KzButton("Skip", variant: .ghost) {
                    advanceStep()
                }
                KzButton("Continue", variant: .primary, isDefault: true) {
                    advanceStep()
                }
            } else {
                KzButton("Skip", variant: .ghost) {
                    completeOnboarding()
                }
                KzButton("Finish", variant: .primary, isDefault: true) {
                    completeOnboarding()
                }
            }
        }
    }

    private func advanceStep() {
        if let next = Step(rawValue: currentStep.rawValue + 1) {
            currentStep = next
        } else {
            completeOnboarding()
        }
    }

    private func completeOnboarding() {
        model.engine.allowsPracticeInOwnWindow = false
        UserDefaults.standard.set(true, forKey: AppModel.onboardingCompletedDefaultsKey)
        dismiss()
    }
}
