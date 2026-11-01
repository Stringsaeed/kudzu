import AppKit
import KudzuCore
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
            case .tryIt: "Try It"
            }
        }
    }

    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var currentStep: Step = .permissions
    @State private var practiceText = ""
    @State private var didCelebrateExpansion = false
    @State private var starterError: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            headerSection
            Divider()

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

            Divider()
            footerNavigation
        }
        .padding(24)
        .frame(width: 600, height: 500)
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
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(systemName: "text.badge.plus")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Welcome to Kudzu")
                        .font(.title2.bold())
                    Text("Step \(currentStep.rawValue + 1) of \(Step.allCases.count) — \(currentStep.title)")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }

            HStack(spacing: 8) {
                ForEach(Step.allCases) { step in
                    VStack(alignment: .leading, spacing: 4) {
                        Capsule()
                            .fill(step.rawValue <= currentStep.rawValue ? Color.accentColor : Color.secondary.opacity(0.25))
                            .frame(height: 5)
                        Text("\(step.rawValue + 1). \(step.title)")
                            .font(.caption)
                            .fontWeight(step == currentStep ? .semibold : .regular)
                            .foregroundStyle(step == currentStep ? .primary : .secondary)
                    }
                }
            }
        }
    }

    // MARK: - Step 1: Permissions

    private var permissionsStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Kudzu needs two macOS privacy permissions to detect your triggers and expand snippets.")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            permissionCard(
                title: "Input Monitoring",
                description: "So Kudzu can notice your triggers as you type.",
                granted: model.permissions.inputMonitoring,
                extraStatus: nil
            ) {
                model.permissions.request(.inputMonitoring)
            }

            permissionCard(
                title: "Accessibility",
                description: "So Kudzu can type the expansion and place forms at your cursor.",
                granted: model.permissions.accessibility,
                extraStatus: "Post Event: \(model.permissions.postEvent ? "Granted" : "Covered by Accessibility grant")"
            ) {
                model.permissions.request(.accessibility)
            }

            #if APPSTORE
            GroupBox {
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "shield.lefthalf.filled")
                        .foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Sandboxed Mac App Store Build")
                            .font(.caption.bold())
                        Text("This build runs inside the macOS App Sandbox. Global keystroke monitoring and synthetic event posting require explicit Input Monitoring and Accessibility approval in System Settings, and reading external folders (such as Espanso configs) requires selecting the folder manually.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(4)
            }
            #endif
        }
    }

    private func permissionCard(
        title: String,
        description: String,
        granted: Bool,
        extraStatus: String?,
        action: @escaping () -> Void
    ) -> some View {
        GroupBox {
            HStack(alignment: .center, spacing: 14) {
                Image(systemName: granted ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(granted ? .green : .orange)

                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Text(title)
                            .font(.headline)
                        Text(granted ? "Granted" : "Required")
                            .font(.caption.weight(.medium))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(
                                (granted ? Color.green : Color.orange).opacity(0.15),
                                in: Capsule()
                            )
                            .foregroundStyle(granted ? .green : .orange)
                    }
                    Text(description)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    if let extraStatus {
                        Text(extraStatus)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                if granted {
                    Label("Active", systemImage: "checkmark")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(.green)
                } else {
                    Button("Grant…", action: action)
                        .buttonStyle(.borderedProminent)
                }
            }
            .padding(8)
        }
    }

    // MARK: - Step 2: Import from Espanso

    private var importStep: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Already using Espanso? Import your existing YAML matches, variables, and forms in one click, or skip this step.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)

                EspansoImportView()
            }
        }
    }

    // MARK: - Step 3: Try It

    private var tryItStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("We’ve added a “Getting Started” group with a few starter snippets (`;kudzu`, `;date`, `;sig`). Give it a try below!")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Type ;kudzu here")
                        .font(.headline)

                    TextField("Type ;kudzu here", text: $practiceText)
                        .textFieldStyle(.roundedBorder)
                        .font(.body)
                        .onChange(of: practiceText) { _, newValue in
                            handlePracticeTextChange(newValue)
                        }

                    if didCelebrateExpansion {
                        HStack(spacing: 8) {
                            Image(systemName: "checkmark.seal.fill")
                                .foregroundStyle(.green)
                                .font(.title2)
                                .symbolEffect(.bounce, value: didCelebrateExpansion)
                            VStack(alignment: .leading, spacing: 2) {
                                Text("It works!")
                                    .font(.headline)
                                    .foregroundStyle(.green)
                                Text("Your trigger expanded into “\(ImportCoordinator.sampleExpansion)”.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .transition(.scale.combined(with: .opacity))
                    } else {
                        Text("Tip: Type `;kudzu` in the field above to watch it expand into “\(ImportCoordinator.sampleExpansion)”.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(8)
            }

            GroupBox("Starter Snippets in “Getting Started”") {
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                    GridRow {
                        Text(";kudzu").font(.callout.monospaced().bold())
                        Text(ImportCoordinator.sampleExpansion).foregroundStyle(.secondary)
                    }
                    GridRow {
                        Text(";date").font(.callout.monospaced().bold())
                        Text("Today’s formatted date").foregroundStyle(.secondary)
                    }
                    GridRow {
                        Text(";sig").font(.callout.monospaced().bold())
                        Text("Sample two-line signature").foregroundStyle(.secondary)
                    }
                }
                .padding(6)
            }

            if let starterError {
                Text(starterError)
                    .font(.caption)
                    .foregroundStyle(.red)
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
        HStack {
            if currentStep != .permissions {
                Button("Back") {
                    if let previous = Step(rawValue: currentStep.rawValue - 1) {
                        currentStep = previous
                    }
                }
            }

            Spacer()

            if currentStep != .tryIt {
                Button("Skip") {
                    advanceStep()
                }
                Button("Continue") {
                    advanceStep()
                }
                .buttonStyle(.borderedProminent)
            } else {
                Button("Skip") {
                    completeOnboarding()
                }
                Button("Finish") {
                    completeOnboarding()
                }
                .buttonStyle(.borderedProminent)
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
