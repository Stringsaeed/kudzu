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
        VStack(alignment: .leading, spacing: 0) {
            headerSection
                .padding(.bottom, 20)
            WorkspaceHairline()
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
            .padding(.vertical, 20)
            WorkspaceHairline()
            footerNavigation
                .padding(.top, 16)
        }
        .padding(.horizontal, WorkspaceSurface.pagePadding)
        .padding(.top, 16)
        .padding(.bottom, WorkspaceSurface.pagePadding)
        .frame(width: 600, height: 520)
        .background(WorkspaceTheme.paper.ignoresSafeArea())
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
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                appIconView
                VStack(alignment: .leading, spacing: 4) {
                    Text("Welcome to Kudzu")
                        .font(WorkspaceTheme.title)
                        .foregroundStyle(WorkspaceTheme.ink)
                    Text("Step \(currentStep.rawValue + 1) of \(Step.allCases.count) — \(currentStep.title)")
                        .font(WorkspaceTheme.body)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
                Spacer()
            }

            HStack(spacing: 8) {
                ForEach(Step.allCases) { step in
                    VStack(alignment: .leading, spacing: 4) {
                        Rectangle()
                            .fill(step.rawValue <= currentStep.rawValue ? WorkspaceTheme.accent : WorkspaceTheme.line)
                            .frame(height: 3)
                        Text("\(step.rawValue + 1). \(step.title)")
                            .font(step == currentStep ? WorkspaceTheme.label : WorkspaceTheme.caption)
                            .foregroundStyle(step == currentStep ? WorkspaceTheme.ink : WorkspaceTheme.secondary)
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
                .frame(width: 44, height: 44)
        } else {
            Text("🌿")
                .font(.system(size: 28))
                .frame(width: 44, height: 44)
                .accessibilityLabel("Kudzu")
        }
    }

    private static func loadAppIcon() -> NSImage? {
        NSApp.applicationIconImage
    }

    // MARK: - Step 1: Permissions

    private var permissionsStep: some View {
        VStack(alignment: .leading, spacing: 20) {
            WorkspaceSection(
                "Privacy permissions",
                description: "Kudzu needs two macOS privacy permissions to detect your triggers and expand snippets."
            ) {
                WorkspaceSettingRow(
                    "Input Monitoring",
                    description: "So Kudzu can notice your triggers as you type."
                ) {
                    permissionTrailing(
                        granted: model.permissions.inputMonitoring,
                        grant: { model.permissions.request(.inputMonitoring) }
                    )
                }
                WorkspaceSettingRow(
                    "Accessibility",
                    description: "So Kudzu can type the expansion and place forms at your cursor. Post Event: \(model.permissions.postEvent ? "Granted" : "Covered by Accessibility grant")."
                ) {
                    permissionTrailing(
                        granted: model.permissions.accessibility,
                        grant: { model.permissions.request(.accessibility) }
                    )
                }
            }

            #if APPSTORE
            VStack(alignment: .leading, spacing: 6) {
                Text("Sandboxed Mac App Store build")
                    .font(WorkspaceTheme.label)
                    .foregroundStyle(WorkspaceTheme.ink)
                Text("This build runs inside the macOS App Sandbox. Global keystroke monitoring and synthetic event posting require explicit Input Monitoring and Accessibility approval in System Settings, and reading external folders (such as Espanso configs) requires selecting the folder manually.")
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(WorkspaceTheme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.top, 8)
            #endif
        }
    }

    @ViewBuilder
    private func permissionTrailing(granted: Bool, grant: @escaping () -> Void) -> some View {
        HStack(spacing: 10) {
            WorkspaceStatusTag(title: granted ? "Granted" : "Needed", tone: granted ? .accent : .caution)
            if granted {
                Text("Active")
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.accent)
            } else {
                Button("Grant…", action: grant)
                    .buttonStyle(.borderedProminent)
                    .tint(WorkspaceTheme.accent)
                    .controlSize(.small)
            }
        }
    }

    // MARK: - Step 2: Import from Espanso

    private var importStep: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Already using Espanso? Import your existing YAML matches, variables, and forms in one click, or skip this step.")
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.secondary)

                EspansoImportView()
            }
        }
    }

    // MARK: - Step 3: Try It

    private var tryItStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("We’ve added a “Getting Started” group with a few starter snippets (;kudzu, ;date, ;sig). Give it a try below.")
                .font(WorkspaceTheme.body)
                .foregroundStyle(WorkspaceTheme.secondary)

            VStack(alignment: .leading, spacing: 12) {
                Text("Type ;kudzu here")
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(WorkspaceTheme.secondary)
                TextField("Type ;kudzu here", text: $practiceText)
                    .textFieldStyle(.roundedBorder)
                    .focused($practiceFocused)
                    .onChange(of: practiceText) { _, newValue in
                        if !newValue.contains(ImportCoordinator.sampleExpansion.trimmingCharacters(in: .whitespaces)) {
                            isDemoPreview = false
                        }
                    }

                practiceStatusRow
            }
            .padding(.vertical, 4)

            WorkspaceSection("Starter snippets in “Getting Started”") {
                starterRow(trigger: ";kudzu", expansion: ImportCoordinator.sampleExpansion)
                starterRow(trigger: ";date", expansion: "Today’s formatted date")
                starterRow(trigger: ";sig", expansion: "Sample two-line signature")
            }

            if let starterError {
                Text(starterError)
                    .font(WorkspaceTheme.caption)
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
            HStack(spacing: 8) {
                WorkspaceStatusTag(title: "Working", tone: .accent)
                Text("Your trigger expanded into “\(ImportCoordinator.sampleExpansion)”.")
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.secondary)
            }
            .transition(.opacity)
        case .demoPreview(let blocker):
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    WorkspaceStatusTag(title: "Demo preview — not verified", tone: .caution)
                    Text("Simulated sample output only. Live expansion was not used.")
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
                if let blocker {
                    blockerActionsRow(reason: blocker)
                }
            }
        case .awaitingLiveInput:
            Text("Tip: Type ;kudzu in the field above to watch it expand into “\(ImportCoordinator.sampleExpansion)”.")
                .font(WorkspaceTheme.caption)
                .foregroundStyle(WorkspaceTheme.secondary)
        case .blocked(let reason, let typedTriggerWithoutExpansion):
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    WorkspaceStatusTag(
                        title: typedTriggerWithoutExpansion ? "Not expanded" : "Live expansion off",
                        tone: .caution
                    )
                    Text(reason)
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
                blockerActionsRow(reason: reason)
            }
        }
    }

    private func blockerActionsRow(reason: String) -> some View {
        HStack(spacing: 8) {
            if case .missingPermissions = model.engine.readiness {
                Button("Configure permissions") { currentStep = .permissions }
                    .controlSize(.small)
            }
            if case .paused = model.engine.readiness {
                Button("Resume Kudzu") {
                    model.engine.isPaused = false
                    model.engine.refreshNow()
                }
                .controlSize(.small)
            }
            Button("Recheck") { model.engine.refreshNow() }
                .controlSize(.small)
            Button("Preview demo only") {
                isDemoPreview = true
                initialCelebrated = false
                practiceText = ImportCoordinator.sampleExpansion
            }
            .buttonStyle(.borderless)
            .controlSize(.small)
        }
    }

    private func starterRow(trigger: String, expansion: String) -> some View {
        HStack(spacing: 12) {
            WorkspaceTriggerLabel(trigger: trigger)
            Text(expansion)
                .font(WorkspaceTheme.body)
                .foregroundStyle(WorkspaceTheme.secondary)
            Spacer()
        }
        .padding(.vertical, 8)
        .overlay(alignment: .bottom) { WorkspaceHairline() }
    }

    // MARK: - Footer Navigation

    private var footerNavigation: some View {
        HStack(spacing: 12) {
            if currentStep != .permissions {
                Button("Back") {
                    if let previous = Step(rawValue: currentStep.rawValue - 1) {
                        currentStep = previous
                    }
                }
            }

            Spacer()

            if currentStep != .tryIt {
                Button("Skip") { advanceStep() }
                    .buttonStyle(.borderless)
                Button("Continue", action: advanceStep)
                    .buttonStyle(.borderedProminent)
                    .tint(WorkspaceTheme.accent)
                    .keyboardShortcut(.defaultAction)
            } else {
                Button("Skip") { completeOnboarding() }
                    .buttonStyle(.borderless)
                Button("Finish", action: completeOnboarding)
                    .buttonStyle(.borderedProminent)
                    .tint(WorkspaceTheme.accent)
                    .keyboardShortcut(.defaultAction)
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
