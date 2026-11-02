import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

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
    @State private var practiceText = ""
    @State private var didCelebrateExpansion = false
    @State private var starterError: String?

    init(initialStep: Step = .permissions, initialPracticeText: String = "", initialCelebrated: Bool = false) {
        _currentStep = State(initialValue: initialStep)
        _practiceText = State(initialValue: initialPracticeText)
        _didCelebrateExpansion = State(initialValue: initialCelebrated)
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
        .padding(KzSpace.s6)
        .frame(width: 600, height: 520)
        .background(KzColor.surface)
        .task {
            NSApplication.shared.activate()
            while !Task.isCancelled {
                model.permissions.refresh()
                if model.permissions.accessibility && model.permissions.inputMonitoring {
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
        if let url = Bundle.main.url(forResource: "kudzu-app-icon", withExtension: "png"),
           let image = NSImage(contentsOf: url) {
            return image
        }
        let fallback = URL(fileURLWithPath: "Kudzu/Resources/kudzu-app-icon.png")
        if let image = NSImage(contentsOf: fallback) {
            return image
        }
        return nil
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
                    KzTextInput("Type ;kudzu here", text: $practiceText)
                        .onChange(of: practiceText) { _, newValue in
                            handlePracticeTextChange(newValue)
                        }
                }

                if didCelebrateExpansion {
                    HStack(spacing: KzSpace.s2) {
                        KzBadge("Working", tone: .vine)
                        Text("Your trigger expanded into “\(ImportCoordinator.sampleExpansion)”.")
                            .kzTextStyle(.callout)
                            .foregroundStyle(KzColor.inkMuted)
                    }
                    .transition(.opacity)
                } else {
                    Text("Tip: Type ;kudzu in the field above to watch it expand into “\(ImportCoordinator.sampleExpansion)”.")
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                }
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
            do {
                _ = try ImportCoordinator.ensureGettingStartedSnippets(in: model.store)
            } catch {
                starterError = error.localizedDescription
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

    private func handlePracticeTextChange(_ newValue: String) {
        if newValue.contains("Kudzu is working!") {
            withAnimation(.spring(duration: 0.35)) {
                didCelebrateExpansion = true
            }
            return
        }

        // If the system-wide event tap isn't active yet (e.g. permissions skipped in onboarding),
        // expand `;kudzu` inside the practice field after a brief moment so the user can still try it.
        if newValue.range(of: ImportCoordinator.sampleTrigger, options: .caseInsensitive) != nil {
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(150))
                if let range = practiceText.range(of: ImportCoordinator.sampleTrigger, options: .caseInsensitive) {
                    practiceText.replaceSubrange(range, with: ImportCoordinator.sampleExpansion)
                    withAnimation(.spring(duration: 0.35)) {
                        didCelebrateExpansion = true
                    }
                }
            }
        }
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
        UserDefaults.standard.set(true, forKey: AppModel.onboardingCompletedDefaultsKey)
        dismiss()
    }
}
