import Link from "next/link";
import type { Metadata } from "next";
import { CopyBlock } from "@/components/copy-block";
import { DocsNav } from "@/components/docs-nav";
import { pageMetadata } from "@/lib/metadata";
import { site } from "@/lib/site";

const description =
  "Connect Claude Code and other AI assistants to Kudzu with MCP. They can find, expand, and write your snippets, and your passwords and keys stay on your Mac.";

export const metadata: Metadata = pageMetadata({
  title: "Use your snippets with AI agents (MCP)",
  description,
  path: "/docs/ai-agents",
});

const sections = [
  { id: "how-it-works", label: "How it works" },
  { id: "turn-on", label: "Turn it on" },
  { id: "claude-code", label: "Connect Claude Code" },
  { id: "skill", label: "Add the skill" },
  { id: "other-apps", label: "Other apps" },
  { id: "try-it", label: "Try it" },
  { id: "tools", label: "What agents can do" },
  { id: "secrets", label: "Passwords and keys" },
  { id: "troubleshooting", label: "Troubleshooting" },
];

const config = JSON.stringify(
  { mcpServers: { kudzu: { command: site.mcp.binary, args: ["--mcp"] } } },
  null,
  2,
);

const tools = [
  { name: "list_snippets", does: "Search your snippets by name, trigger, and content." },
  { name: "get_snippet", does: "Read one snippet’s template, triggers, and fill-in fields." },
  {
    name: "expand_snippet",
    does: "Get a snippet’s final text, with dates, nested snippets, and your fill-in values.",
  },
  { name: "list_groups", does: "See how your snippets are organized." },
  {
    name: "create_snippet",
    does: "Add a snippet. Kudzu rejects triggers that are taken and creates missing groups.",
  },
  {
    name: "update_snippet",
    does: "Change a snippet’s name, triggers, content, group, or settings.",
  },
  { name: "list_clipboard", does: "Search what you copied recently, newest first." },
  { name: "get_clipboard_item", does: "Read one clipboard item in full." },
];

export default function AiAgentsPage() {
  return (
    <main id="main" className="page wrap docs-layout">
      <DocsNav current="/docs/ai-agents" sections={sections} />
      <div>
        <header className="page-heading">
          <Link className="eyebrow" href="/docs">
            DOCS
          </Link>
          <h1>AI agents.</h1>
          <p>{description}</p>
        </header>
        <div className="prose">
          <h2 id="how-it-works">How it works</h2>
          <p>
            Kudzu speaks the{" "}
            <a href="https://modelcontextprotocol.io">Model Context Protocol</a> (MCP), the
            standard way AI assistants use tools. Once you connect an assistant, it can look up the
            replies, signatures, and boilerplate you already keep in Kudzu, and save new ones when
            you ask.
          </p>
          <ol className="agent-flow">
            <li>
              <strong>Your assistant</strong> starts Kudzu’s MCP server, a small helper inside
              Kudzu.app.
            </li>
            <li>
              <strong>The helper</strong> passes each request to Kudzu through a private
              connection on your Mac. Nothing goes through the internet.
            </li>
            <li>
              <strong>Kudzu</strong> answers from your library and clipboard history, and hides
              anything that looks like a password or key first.
            </li>
          </ol>
          <p>
            Agent access is off until you turn it on, and you can turn it off at any time. While
            it’s off, connected assistants are told Kudzu is unavailable.
          </p>

          <h2 id="turn-on">1. Turn it on</h2>
          <p>
            Open Kudzu and go to <strong>Settings → AI agents</strong>. Turn on{" "}
            <strong>Allow AI agents</strong>. Two more switches decide what assistants can do:
          </p>
          <ul>
            <li>
              <strong>Create and edit snippets:</strong> assistants can add snippets and change
              existing ones. They can never delete anything.
            </li>
            <li>
              <strong>Share clipboard history:</strong> assistants can search what you’ve copied,
              such as a link or an error message.
            </li>
          </ul>

          <h2 id="claude-code">2. Connect Claude Code</h2>
          <p>Run this once in Terminal:</p>
          <CopyBlock label="Claude Code command" value={site.mcp.claudeCommand} />
          <p>
            Start a new Claude Code session and run <code>/mcp</code>. You should see{" "}
            <code>kudzu</code> connected. If Kudzu isn’t in your Applications folder, copy the
            command from <strong>Settings → AI agents</strong> instead, because it has the right
            path for your Mac. A client that still starts <code>kudzu-desktop --mcp</code> keeps
            working: the app ships that name as a symlink to <code>Kudzu</code>.
          </p>

          <h2 id="skill">3. Add the skill</h2>
          <p>
            The Kudzu skill teaches Claude when to reach for your snippets and how to write
            Kudzu templates, triggers, and groups. In <strong>Settings → AI agents</strong>, click{" "}
            <strong>Install skill</strong>. It goes in{" "}
            <code>~/.claude/skills/kudzu/SKILL.md</code>. After a Kudzu update brings a new
            version, the button reads <strong>Update skill</strong>.
          </p>
          <p>
            Other assistants can use it too. It ships inside the app at{" "}
            <code>{site.mcp.skillDir}</code>, so you can link that folder into another
            assistant’s skills folder. Connected assistants can also read it from Kudzu’s MCP
            server as <code>kudzu://skill/SKILL.md</code>.
          </p>

          <h2 id="other-apps">Other apps</h2>
          <p>
            Claude Desktop, Cursor, and most other MCP apps take the same configuration. Add it to
            the app’s MCP settings. For Claude Desktop, that’s{" "}
            <code>~/Library/Application Support/Claude/claude_desktop_config.json</code>. For
            Cursor, it’s <code>~/.cursor/mcp.json</code>.
          </p>
          <CopyBlock label="MCP configuration" value={config} />

          <h2 id="try-it">Try it</h2>
          <p>Ask your assistant things like:</p>
          <ul>
            <li>“Reply to this email with my standard pricing answer.”</li>
            <li>“Save this paragraph as a snippet called Refund policy, triggered by ;refund.”</li>
            <li>“Which of my snippets mention the old office address? Update them.”</li>
            <li>“What was the link I copied from Slack this morning?”</li>
          </ul>

          <h2 id="tools">What agents can do</h2>
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th scope="col">Tool</th>
                  <th scope="col">What it does</th>
                </tr>
              </thead>
              <tbody>
                {tools.map((tool) => (
                  <tr key={tool.name}>
                    <th scope="row">
                      <code>{tool.name}</code>
                    </th>
                    <td>{tool.does}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p>
            The last two appear only when you share clipboard history, and the write tools only
            when you allow editing. Changes an assistant makes save, sync, and back up like your
            own, and they show up in Kudzu right away.
          </p>

          <h2 id="secrets">Passwords and keys stay on your Mac</h2>
          <p>
            Before anything reaches an assistant, Kudzu checks it on your Mac. Text that looks like
            a password, API key, access token, or private key becomes{" "}
            <code>[redacted: API key]</code>, or a similar note. A clipboard item that’s only a
            secret isn’t shared at all.
          </p>
          <ul>
            <li>Assistants can’t find a secret by searching for it.</li>
            <li>
              Assistants can’t change the text of a snippet that holds a secret, so a redacted copy
              never replaces the real one.
            </li>
            <li>
              Kudzu recognizes keys from services like GitHub, OpenAI, Anthropic, AWS, Stripe, and
              Slack. It also catches labelled values such as <code>password=…</code>, passwords in
              links, and random-looking strings.
            </li>
          </ul>
          <p>
            The same check protects your clipboard history in Kudzu. A secret shows as dots until
            you hover over it. Kudzu’s detection is a best guess, and you can correct it:
          </p>
          <ul>
            <li>
              Choose <strong>Not a secret?</strong> on an item Kudzu hid by mistake. It shows
              normally, and assistants can read it.
            </li>
            <li>
              Choose <strong>Hide as a secret</strong> in an item’s toolbar to hide something Kudzu
              missed. Assistants can’t see it.
            </li>
            <li>
              Turn on <strong>Settings → Clipboard → Show secrets</strong> to stop hiding them in
              Kudzu. Assistants still don’t see them.
            </li>
          </ul>
          <p>
            A short, plain password can look like an ordinary word. Keep passwords in a password
            manager. Kudzu never records what you copy from one.
          </p>

          <h2 id="troubleshooting">Troubleshooting</h2>
          <h3>“Kudzu isn’t running, or AI agent access is turned off”</h3>
          <p>
            Open Kudzu and check that <strong>Allow AI agents</strong> is on. Kudzu has to be
            running, and it usually is, in the menu bar.
          </p>
          <h3>The clipboard or editing tools are missing</h3>
          <p>
            Turn on <strong>Share clipboard history</strong> or{" "}
            <strong>Create and edit snippets</strong>, then start a new session in your assistant.
          </p>
          <h3>It stopped working after I moved Kudzu</h3>
          <p>
            The command includes Kudzu’s location. Remove the old server (
            <code>claude mcp remove kudzu</code> in Claude Code) and add it again with the command
            from <strong>Settings → AI agents</strong>.
          </p>
          <h3>Still stuck?</h3>
          <p>
            <a href={site.issues}>Open an issue on GitHub</a> and we’ll take a look.
          </p>
        </div>
      </div>
    </main>
  );
}
