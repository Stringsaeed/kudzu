import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// AI agents settings section (`settings.tsx` `AgentsSettings` + `CopyBlock`).
struct AgentsSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var setup: AgentSetup?
    @State private var installingSkill: Bool = false

    var body: some View {
        let enabled = model.settings.agentAccess
        let clipboardEnabled = model.settings.clipboardEnabled
        let skillCopy = AgentCopy.skill(setup)

        VStack(alignment: .leading, spacing: 32) {
            SettingsCardGroup(
                "AI agents",
                description: "Let assistants like Claude Code look up your snippets, expand them and save new ones, using the Model Context Protocol (MCP). Agents connect to Kudzu on this Mac; Kudzu doesn't upload anything."
            ) {
                SettingsRow(
                    "Allow AI agents",
                    description: AgentCopy.access(on: enabled, running: enabled ? setup?.running : nil)
                ) {
                    Toggle(
                        "Allow AI agents",
                        isOn: Binding(
                            get: { enabled },
                            set: { next in Task { await model.patchSettings { $0.agentAccess = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }

                Hairline()

                SettingsRow(
                    "Create and edit snippets",
                    description: "Agents can add snippets and change existing ones. They can't delete anything."
                ) {
                    Toggle(
                        "Create and edit snippets",
                        isOn: Binding(
                            get: { model.settings.agentWrite },
                            set: { next in Task { await model.patchSettings { $0.agentWrite = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                    .disabled(!enabled)
                }

                Hairline()

                SettingsRow(
                    "Share clipboard history",
                    description: AgentCopy.clipboard(enabled: clipboardEnabled)
                ) {
                    Toggle(
                        "Share clipboard history",
                        isOn: Binding(
                            get: { model.settings.agentClipboard && clipboardEnabled },
                            set: { next in Task { await model.patchSettings { $0.agentClipboard = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                    .disabled(!enabled || !clipboardEnabled)
                }
            }
            .stagger(0)

            SettingsCardGroup("Passwords and keys") {
                HStack(alignment: .top, spacing: 12) {
                    Icon.shieldCheck
                        .icon(16)
                        .foregroundStyle(.kz.success)
                        .padding(.top, 2)
                    VStack(alignment: .leading, spacing: 6) {
                        Text("\(Text("Secrets are withheld from agents.").fontWeight(.medium).foregroundStyle(Color.kz.foreground)) Before anything reaches an agent, Kudzu replaces text that looks like a password, API key, access token or private key with \(Text("[redacted]").font(.kz.mono(12))). Clipboard items that are only a secret aren’t shared at all, and agents can’t search for them.")
                        Text("Detection is a best guess. A short, plain password can look like an ordinary word, so keep passwords in a password manager; Kudzu never records copies from one. In clipboard history, you can mark an item as a secret, or as not one, and agents follow your choice.")
                    }
                    .font(.system(size: 12))
                    .mutedForeground()
                    .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.vertical, 14)
            }
            .stagger(1)

            // `Connect an agent` is always there; the copy blocks wait for the setup to load.
            SettingsCardGroup(
                "Connect an agent",
                description: "Do this once per app. The agent starts Kudzu's MCP server itself; Kudzu just needs to be running."
            ) {
                VStack(alignment: .leading, spacing: 0) {
                    Text("Claude Code")
                        .font(.system(size: 13, weight: .medium))
                    Text("Run this in Terminal:")
                        .font(.system(size: 12))
                        .mutedForeground()
                    if let setup {
                        CopyBlockView(label: "command", text: setup.claudeCommand)
                            .padding(.top, 8)
                    }
                }
                .padding(.vertical, 14)

                Hairline()

                SettingsRow(
                    "Claude Code skill",
                    description: skillCopy.description
                ) {
                    if let buttonTitle = skillCopy.button {
                        Button {
                            Task {
                                installingSkill = true
                                defer { installingSkill = false }
                                // Reload either way, like the web's `finally { reload() }`.
                                _ = await model.installSkill()
                                setup = await model.loadAgentSetup()
                            }
                        } label: {
                            HStack(spacing: 6) {
                                if installingSkill { ProgressView().controlSize(.mini) }
                                Text(buttonTitle)
                            }
                        }
                        .buttonStyle(.kz(.secondary, size: .sm))
                        .disabled(AgentCopy.skillButtonDisabled(setup: setup, installing: installingSkill))
                    } else {
                        HStack(spacing: 6) {
                            Icon.check
                                .icon(14)
                                .foregroundStyle(.kz.success)
                            Text("Installed")
                                .font(.system(size: 12))
                                .mutedForeground()
                        }
                    }
                }

                Hairline()

                VStack(alignment: .leading, spacing: 0) {
                    Text("Other apps")
                        .font(.system(size: 13, weight: .medium))
                    Text("Claude Desktop, Cursor and other MCP apps: add this to their MCP server settings.")
                        .font(.system(size: 12))
                        .mutedForeground()
                        .fixedSize(horizontal: false, vertical: true)
                    if let setup {
                        CopyBlockView(label: "configuration", text: setup.configJson)
                            .padding(.top, 8)
                        if let bundled = setup.bundledSkillDir {
                            Text("For other agents, the skill is also in \(Text(bundled).font(.kz.mono(12)).foregroundStyle(Color.kz.mix(.foreground, alpha: 0.85))), and connected agents can read it as \(Text("kudzu://skill/SKILL.md").font(.kz.mono(12))).")
                                .font(.system(size: 12))
                                .mutedForeground()
                                .fixedSize(horizontal: false, vertical: true)
                                .textSelection(.enabled)
                                .padding(.top, 8)
                        }
                    }
                }
                .padding(.vertical, 14)
            }
            .stagger(2)
        }
        .task(id: model.settings.agentAccess) {
            setup = await model.loadAgentSetup()
        }
    }
}

/// `settings.tsx` `CopyBlock`: a well with the command or config and an icon copy button inside.
private struct CopyBlockView: View {
    var label: String
    var text: String

    var body: some View {
        HStack(alignment: .top, spacing: 4) {
            ScrollView(.horizontal, showsIndicators: false) {
                Text(text)
                    .font(.kz.mono(11.5))
                    .lineSpacing(5)
                    .textSelection(.enabled)
                    .fixedSize()
                    .padding(.vertical, 10)
                    .padding(.leading, 12)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            Button {
                copyToPasteboard()
            } label: {
                Icon.copy.icon(12)
            }
            .buttonStyle(.kz(.ghost, size: .iconXs))
            .accessibilityLabel("Copy \(label)")
            .padding(.top, 6)
            .padding(.trailing, 6)
        }
        .background(Color.kz.mix(.foreground, alpha: 0.025), in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous))
        .depth(.well, fill: Color.clear, in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous))
    }

    /// `copyText`: "Copied {what}", or "Couldn't copy".
    private func copyToPasteboard() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        if pasteboard.setString(text, forType: .string) {
            toast.success("Copied \(label)")
        } else {
            toast.error("Couldn’t copy")
        }
    }
}
