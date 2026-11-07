import Link from "next/link";
import type { Metadata } from "next";
import { CopyCommand } from "@/components/copy-command";
import { DocsNav } from "@/components/docs-nav";
import { DownloadAction } from "@/components/download-action";
import { pageMetadata } from "@/lib/metadata";

const description =
  "Install Kudzu, approve its permission, write your first shortcut, and learn the keyboard shortcuts.";

export const metadata: Metadata = pageMetadata({
  title: "Getting started with Kudzu, the free Mac text expander",
  description,
  path: "/docs",
  absolute: true,
});

const sections = [
  { id: "install", label: "Install" },
  { id: "permissions", label: "Permissions" },
  { id: "first-shortcut", label: "Your first shortcut" },
  { id: "everyday", label: "Everyday shortcuts" },
  { id: "clipboard", label: "Clipboard history" },
  { id: "your-data", label: "Your data" },
  { id: "troubleshooting", label: "Troubleshooting" },
];

export default function DocsPage() {
  return (
    <main id="main" className="page wrap docs-layout">
      <DocsNav current="/docs" sections={sections} />
      <div>
        <header className="page-heading">
          <span className="eyebrow">DOCS</span>
          <h1>Getting started.</h1>
          <p>{description}</p>
        </header>
        <div className="prose">
          <h2 id="install">Install</h2>
          <p>
            Kudzu needs macOS 14 Sonoma or later and runs on Apple Silicon and Intel. Download the
            DMG and drag Kudzu to Applications, or install it with Homebrew.
          </p>
          <div className="docs-install">
            <DownloadAction />
            <CopyCommand />
          </div>

          <h2 id="permissions">Permissions</h2>
          <p>
            Kudzu needs Accessibility access to recognize your shortcuts and insert text. macOS 27
            and later call this Device Control and Data Access. A guided helper opens during
            onboarding and stays beside System Settings. Drag the Kudzu icon into the list, then
            turn it on. macOS owns this switch, and Kudzu notices as soon as you flip it.
          </p>
          <p>
            You can reopen the helper from <strong>Settings → General</strong>.{" "}
            <Link href="/#permissions">Here’s what Kudzu can see</Link>.
          </p>

          <h2 id="first-shortcut">Your first shortcut</h2>
          <ol>
            <li>
              Press <kbd>⌘N</kbd> to make a new snippet and give it a name.
            </li>
            <li>
              Add a trigger, like <code>;sig</code>. Starting with a character such as{" "}
              <code>;</code> keeps it from firing mid-sentence.
            </li>
            <li>Type what it should expand to. Changes save automatically.</li>
            <li>
              Type the trigger in any app. Press <kbd>⌫</kbd> right after to change your mind.
            </li>
          </ol>
          <p>
            Want dates, fill-in fields, or nested snippets? See the{" "}
            <Link href="/docs/templates">template guide</Link>.
          </p>

          <h2 id="everyday">Everyday shortcuts</h2>
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th scope="col">Shortcut</th>
                  <th scope="col">What it does</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <th scope="row">
                    <kbd>⌃⌥Space</kbd>
                  </th>
                  <td>Quick search, from any app</td>
                </tr>
                <tr>
                  <th scope="row">
                    <kbd>⌃⌥V</kbd>
                  </th>
                  <td>Clipboard history, from any app</td>
                </tr>
                <tr>
                  <th scope="row">
                    <kbd>⌘K</kbd>
                  </th>
                  <td>Command menu, inside Kudzu</td>
                </tr>
                <tr>
                  <th scope="row">
                    <kbd>⌘/</kbd>
                  </th>
                  <td>Every keyboard shortcut, inside Kudzu</td>
                </tr>
                <tr>
                  <th scope="row">
                    <kbd>⌘⇧P</kbd>
                  </th>
                  <td>Pause or resume expansion, inside Kudzu</td>
                </tr>
              </tbody>
            </table>
          </div>

          <h2 id="clipboard">Clipboard history</h2>
          <p>
            Kudzu records text, code, links, colors, images, and files as you copy them. Press{" "}
            <kbd>⌃⌥V</kbd> to search, and <kbd>↵</kbd> to paste into the app you were in. Press{" "}
            <kbd>⇧↵</kbd> to paste as plain text.
          </p>
          <p>
            History is kept for a week by default. Choose anything from a day to forever. It keeps up
            to 5,000 unpinned items, pinned items never expire, and nothing leaves your Mac. Copies an
            app marks concealed or transient are skipped, and so is anything from a password manager.
            Add more apps to skip in <strong>Settings → Clipboard</strong>.
          </p>

          <h2 id="your-data">Your data</h2>
          <p>
            Everything here lives in <strong>Settings → Backups & data</strong>, except sync, which
            is in <strong>Settings → General</strong>.
          </p>
          <ul>
            <li>
              <strong>Import:</strong> from Espanso (detected automatically) or a Kudzu JSON file.
              Kudzu takes a backup first.
            </li>
            <li>
              <strong>Export:</strong> to Kudzu JSON or Espanso YAML. Espanso export writes plain
              text, so use JSON to keep formatting and images.
            </li>
            <li>
              <strong>Backups:</strong> taken daily and before every import or restore. The last 14
              are kept.
            </li>
            <li>
              <strong>Sync:</strong> your snippet library syncs through your private iCloud when your
              Mac is signed in. It’s on by default, and you can turn it off. Clipboard history never
              syncs.
            </li>
          </ul>

          <h2 id="troubleshooting">Troubleshooting</h2>
          <h3>A shortcut doesn’t expand</h3>
          <ul>
            <li>Check that Kudzu isn’t paused and the snippet is turned on.</li>
            <li>
              Check for macOS Secure Input, which pauses expansion. Password fields and some apps
              turn it on.
            </li>
            <li>Check the snippet’s app rules. Password managers are excluded by default.</li>
            <li>If you just granted access, restarting Kudzu can reconnect the keyboard.</li>
          </ul>
          <h3>Text arrives wrong in one app</h3>
          <p>
            Short text is typed and long text is pasted. If an app handles one badly, set its
            insertion method in the snippet’s or group’s rules.
          </p>
          <h3>Still stuck?</h3>
          <p>
            <a href="https://github.com/Stringsaeed/kudzu/issues">Open an issue on GitHub</a> and
            we’ll take a look.
          </p>
        </div>
      </div>
    </main>
  );
}
