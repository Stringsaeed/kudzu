import type { Metadata } from "next";
import { site } from "@/lib/site";

const description = "What the Kudzu app and this website do, and don’t do, with your data.";

export const metadata: Metadata = {
  title: "Privacy",
  description,
  alternates: { canonical: "/privacy" },
};

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
            Passwords and anything marked concealed or transient are skipped, and password managers
            are excluded by default. You choose how long it’s kept.
          </li>
          <li>
            <strong>iCloud sync:</strong> optional. If you turn it on, your snippets sync through
            your private iCloud database, which Kudzu’s developer can’t see. Clipboard history
            doesn’t sync.
          </li>
          <li>
            <strong>Updates:</strong> Kudzu checks GitHub for new versions. That check sends nothing
            about you beyond what any web request includes.
          </li>
          <li>
            <strong>Usage insights:</strong> counted on your device and not uploaded.
          </li>
        </ul>
        <p>
          Don’t take our word for it: <a href={site.github}>the code is open source</a>.
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
