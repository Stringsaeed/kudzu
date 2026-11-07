import type { Metadata } from "next";
import Link from "next/link";
import { pageMetadata } from "@/lib/metadata";
import { site } from "@/lib/site";

const description = "What the Kudzu app and this website do, and don’t do, with your data.";

export const metadata: Metadata = pageMetadata({ title: "Privacy", description, path: "/privacy" });

export default function PrivacyPage() {
  return (
    <main id="main" className="page wrap">
      <header className="page-heading">
        <span className="eyebrow">PRIVACY</span>
        <h1>Short version: it’s yours.</h1>
        <p>{description}</p>
      </header>
      <div className="prose prose-narrow">
        <h2>The Kudzu app</h2>
        <p>
          Kudzu has no accounts, no analytics, and no Kudzu servers. Your snippets live on your Mac.
        </p>
        <ul>
          <li>
            <strong>What you type:</strong> Kudzu keeps a short rolling buffer of recent keys in
            memory to spot your shortcuts. It never stores what you type.
          </li>
          <li>
            <strong>Clipboard history:</strong> stored on your Mac and never sent anywhere.
            Anything an app marks concealed or transient is skipped, and password managers are
            excluded by default. Other copies are kept, including a password copied from a document,
            so add apps under <strong>Settings → Clipboard</strong> or clear history whenever you
            like. You choose how long it’s kept.
          </li>
          <li>
            <strong>iCloud sync:</strong> on by default when your Mac is signed in to iCloud. Your
            snippets sync through your private iCloud database, which Kudzu’s developer can’t see.
            Clipboard history never syncs. Turn it off in{" "}
            <strong>Settings → General → Sync with iCloud</strong>.
          </li>
          <li>
            <strong>Updates:</strong> about 20 seconds after launch and every 6 hours, Kudzu asks
            GitHub for a newer version and downloads it quietly. It never restarts without you. The
            request carries only what any web request does, such as your IP address, and GitHub
            handles it under{" "}
            <a href="https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement">
              its privacy statement
            </a>
            . Turn it off in <strong>Settings → About → Check for updates automatically</strong>.
          </li>
          <li>
            <strong>Usage insights:</strong> counted on your device and not uploaded.
          </li>
        </ul>
        <p>
          Don’t take our word for it: <a href={site.github}>the code is open source</a>.{" "}
          <Link href="/#permissions">Here’s what Kudzu can see</Link>, and{" "}
          <Link href="/docs#your-data">where your data lives</Link>.
        </p>

        <h2>This website</h2>
        <ul>
          <li>No analytics, advertising, or tracking cookies.</li>
          <li>Your theme choice is saved in your browser and goes no further.</li>
          <li>The interactive previews run entirely in your browser. Nothing you type is sent.</li>
          <li>
            The site is hosted on Vercel, which handles the usual request data needed to serve a
            page, such as your IP address.
          </li>
          <li>Downloads come from GitHub, which handles those requests under its own policies.</li>
        </ul>

        <h2>Questions</h2>
        <p>
          <a href={site.issues}>Open an issue on GitHub</a> and we’ll answer there.
        </p>
      </div>
    </main>
  );
}
