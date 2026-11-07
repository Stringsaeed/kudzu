import Link from "next/link";
import Image from "next/image";
import { AppGallery } from "@/components/app-gallery";
import { DownloadAction } from "@/components/download-action";
import { Icon } from "@/components/icons";
import { PlayOnView } from "@/components/play-on-view";
import { switching } from "@/lib/compare";
import { getLatestRelease, releaseAnchor } from "@/lib/releases";
import { site } from "@/lib/site";
import { worstCase, worstClipboardItems, worstSnippets } from "@/lib/worst-case";
import { SnippetPlayground } from "@/components/snippet-playground";
import { ClipboardPreview } from "@/components/clipboard-preview";
import { TemplateExamples } from "@/components/template-examples";
import { CopyCommand } from "@/components/copy-command";

export const revalidate = 3600;

const moreFeatures = [
  {
    title: "Formatted snippets",
    body: "Bold, links, lists, and embedded images, pasted as rich text where apps support it.",
  },
  {
    title: "Quick search",
    body: "Press ⌃⌥Space in any app. Search names, triggers, and content, then insert.",
  },
  {
    title: "Per-app rules",
    body: "Allow or block snippets by app, and choose how each one inserts its text.",
  },
  {
    title: "Insights",
    body: "See expansions, characters, and time saved. What you type is never stored.",
  },
  {
    title: "iCloud sync",
    body: "Your snippet library follows you between Macs through your private iCloud.",
  },
  {
    title: "Backups",
    body: "Daily local backups, with the last 14 kept and one click to restore.",
  },
];

export default async function Home() {
  const release = await getLatestRelease();
  const worst = await worstCase();
  const softwareSchema = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "@id": `${site.url}/#software`,
    name: site.name,
    url: site.url,
    description: site.description,
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "macOS 14 or later",
    license: site.license,
    sameAs: [site.github],
    downloadUrl: `${site.url}${site.download}`,
    ...(release ? { softwareVersion: release.version } : {}),
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  }).replace(/</g, "\\u003c");
  return (
    <>
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: softwareSchema }} />
      <main id="main">
        <section className="hero wrap" aria-labelledby="hero-title">
          <Link
            className="release-pill"
            href={release ? `/changelog#${releaseAnchor(release.version)}` : "/changelog"}
          >
            <span className="status-dot"></span>
            {release ? `Kudzu ${release.version} is here. See what’s new` : "See what’s new in Kudzu"}
            <Icon name="arrow" />
          </Link>
          <h1 id="hero-title">
            For things
            <br />
            worth <span className="hero-accent">repeating.</span>
          </h1>
          <p className="hero-description">
            <strong>A free text expander and clipboard manager for your Mac.</strong>
            <br />
            Your best words, a few keystrokes away.
          </p>
          <DownloadAction
            secondary={
              <a className="button button-text" href="#playground">
                Give it a spin
              </a>
            }
          />
          <p className="fine-print">
            Free & open source<span>·</span>macOS 14+<span>·</span>Apple Silicon & Intel
            <span>·</span>Notarized by Apple
          </p>

          <SnippetPlayground snippets={worst ? worstSnippets : undefined} />
        </section>

        <section className="intro-section wrap" id="features">
          <span className="eyebrow">LESS BUSYWORK. MORE HEADSPACE.</span>
          <h2>
            You’ve written it before.
            <br />
            <span>You shouldn’t have to again.</span>
          </h2>
          <p>
            The same introduction. That link you’re always looking for. The thing you copied five
            minutes ago. Kudzu keeps the little things close, so you can get on with the good stuff.
          </p>
        </section>

        <section className="feature-row wrap" aria-labelledby="shortcut-section-title">
          <div className="feature-copy">
            <span className="feature-number">01 / TEXT SHORTCUTS</span>
            <h2 id="shortcut-section-title">
              A few keys.
              <br />A whole lot said.
            </h2>
            <p>
              Turn the things you type all the time into shortcuts you’ll actually remember. Email
              replies, addresses, code, your very particular sign-off. Write it once. Make it yours.
            </p>
            <a className="inline-link" href="#playground">
              Try your first shortcut
              <Icon name="arrow" />
            </a>
            <div className="feature-detail">
              <svg className="icon">
                <use href="#i-check" />
              </svg>
              Changed your mind? Backspace undoes an expansion.
            </div>
          </div>
          <PlayOnView className="shortcut-art feature-art" labelledBy="shortcut-title">
            <h3 className="visually-hidden" id="shortcut-title">
              A short trigger becomes a complete email
            </h3>
            <div className="shortcut-bubble">
              <span className="mini-avatar">You</span>
              <span>
                Just a little <code>;hello</code>
              </span>
              <span className="bubble-time">now</span>
            </div>
            <div className="shortcut-path" aria-hidden="true">
              <Icon name="arrow-down" />
              <span>Kudzu takes it from here</span>
            </div>
            <div className="email-paper">
              <div className="paper-heading">
                <span>New message</span>
              </div>
              <div className="paper-recipient">
                To: <span>someone you’ll be glad you met</span>
              </div>
              <p>Hey there!</p>
              <p>
                So lovely to hear from you. Thanks for reaching out — I’m excited to see what we can
                make together.
              </p>
              <p>
                Let’s get into it.<span className="text-caret" aria-hidden="true"></span>
              </p>
              <div className="paper-footer">
                <span>Same you. Fewer keystrokes.</span>
                <svg className="icon">
                  <use href="#i-check" />
                </svg>
              </div>
            </div>
            <div className="floating-key key-semicolon" aria-hidden="true">
              <span>;</span>
            </div>
            <div className="floating-key key-h" aria-hidden="true">
              <span>h</span>
            </div>
          </PlayOnView>
        </section>

        <section className="feature-row reverse wrap" id="clipboard" aria-labelledby="clipboard-title">
          <div className="feature-copy">
            <span className="feature-number">02 / CLIPBOARD HISTORY</span>
            <h2 id="clipboard-title">
              Copied it.
              <br />
              Kept it. Found it.
            </h2>
            <p>
              Your clipboard has a memory now. Find that link, color, image, or perfect turn of
              phrase. Search what you’ve copied, pin the keepers, and paste right where you left
              off.
            </p>
            <div className="shortcut-explainer">
              <kbd>⌃</kbd>
              <kbd>⌥</kbd>
              <kbd>V</kbd>
              <span>Your recent copies, one shortcut away.</span>
            </div>
            <div className="feature-detail">
              <svg className="icon">
                <use href="#i-lock" />
              </svg>
              Your clipboard history stays on this Mac.
            </div>
          </div>
          <ClipboardPreview items={worst ? worstClipboardItems : undefined} />
        </section>

        <TemplateExamples />

        <section className="more-features wrap" aria-labelledby="more-title">
          <h2 className="visually-hidden" id="more-title">
            More in Kudzu
          </h2>
          {moreFeatures.map((feature) => (
            <article key={feature.title}>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </article>
          ))}
        </section>

        <AppGallery />

        <section className="privacy-section" id="privacy">
          <div className="wrap privacy-inner">
            <div className="privacy-copy">
              <span className="eyebrow">A SMALL APP. SOME FIRM BELIEFS.</span>
              <h2>
                Your words.
                <br />
                Your Mac.
                <br />
                <em>End of story.</em>
              </h2>
              <p>
                No account to create. No subscription to cancel. No analytics watching over your
                shoulder. Just a useful little app, doing its thing.
              </p>
              <a className="inline-link" href={site.github} target="_blank" rel="noopener noreferrer">
                Open source. Come have a look.
                <Icon name="arrow" />
              </a>
            </div>
            <div className="privacy-notes">
              <article>
                <span>01</span>
                <div>
                  <h3>Local feels right.</h3>
                  <p>
                    Your clipboard history stays on your Mac. Password managers are excluded by
                    default, and copies an app marks private are skipped.
                  </p>
                </div>
              </article>
              <article>
                <span>02</span>
                <div>
                  <h3>Your Macs, in sync.</h3>
                  <p>
                    Your snippet library travels between your Macs through your private iCloud
                    database, which Kudzu’s developer can’t see. Rather not? Switch it off.
                  </p>
                </div>
              </article>
              <article>
                <span>03</span>
                <div>
                  <h3>Always a way out.</h3>
                  <p>
                    Import from Espanso. Export to JSON or YAML. Daily local backups keep a little
                    extra peace of mind.
                  </p>
                </div>
              </article>
            </div>
          </div>
          <div className="wrap trust-section" id="permissions">
            <div className="trust-heading">
              <h3>What Kudzu can see, and what it does with it.</h3>
              <p>
                A text expander has to watch for your shortcuts, so macOS asks you to approve
                Accessibility access (called Device Control and Data Access on macOS 27 and later).
                Here’s exactly what that’s for.
              </p>
            </div>
            <div className="trust-grid">
              <article>
                <h4>What it does</h4>
                <ul>
                  <li>
                    <Icon name="check" />
                    Keeps a short rolling buffer of recent keys in memory, just enough to spot a
                    shortcut.
                  </li>
                  <li>
                    <Icon name="check" />
                    Inserts your snippet into the app you’re using, restoring your clipboard if it
                    pasted.
                  </li>
                  <li>
                    <Icon name="check" />
                    Records what you copy for clipboard history, on this Mac, for as long as you
                    choose.
                  </li>
                </ul>
              </article>
              <article>
                <h4>What it never does</h4>
                <ul>
                  <li>
                    <Icon name="close" />
                    Store what you type. Insights only count expansions.
                  </li>
                  <li>
                    <Icon name="close" />
                    Record copies an app marks private, or anything from a password manager unless
                    you allow it.
                  </li>
                  <li>
                    <Icon name="close" />
                    Phone home. There are no Kudzu servers. It only goes online to check for updates
                    and to sync through your iCloud, and you can turn off either one.
                  </li>
                </ul>
              </article>
            </div>
            <p className="trust-footnote">
              You stay in charge: macOS owns the permission, and you can revoke it in System
              Settings at any time.{" "}
              <a href={site.keyBufferSource} target="_blank" rel="noopener noreferrer">
                Read the key buffer
              </a>{" "}
              or the{" "}
              <a href={site.engineSource} target="_blank" rel="noopener noreferrer">
                whole engine
              </a>
              . <Link href="/privacy">Privacy details</Link>
            </p>
          </div>
        </section>

        <section className="switch-section wrap" aria-labelledby="switch-title">
          <div className="section-heading">
            <div>
              <span className="eyebrow">ALREADY HAVE A SYSTEM?</span>
              <h2 id="switch-title">Coming from somewhere else.</h2>
            </div>
            <p>
              An honest look at how Kudzu compares,{" "}
              <br />
              and what moving over really takes. <Link href="/compare">See all comparisons</Link>
            </p>
          </div>
          <div className="switch-grid">
            {switching.map((item) => (
              <Link key={item.slug} href={`/compare/${item.slug}`}>
                <strong>{item.name}</strong>
                <span>{item.migration}</span>
                <em>
                  Compare
                  <Icon name="arrow" />
                </em>
              </Link>
            ))}
          </div>
        </section>

        <section className="faq-section wrap" id="faq" aria-labelledby="faq-title">
          <div>
            <span className="eyebrow">THE LAST FEW THINGS</span>
            <h2 id="faq-title">Glad you asked.</h2>
            <p>Little app. Straight answers.</p>
          </div>
          <div className="faq-list">
            <details>
              <summary>
                Actually free?<Icon name="plus" />
              </summary>
              <p>
                Yes. Kudzu is free and open source under the MIT license. No subscription, no
                account, and no paid tier to unlock the features you came for.
              </p>
            </details>
            <details>
              <summary>
                Does Kudzu read what I type?<Icon name="plus" />
              </summary>
              <p>
                Only enough to spot your shortcuts. Kudzu keeps a short rolling buffer of recent
                keys in memory and never stores what you type. <a href="#permissions">See exactly
                what it can see</a>, or read the source code.
              </p>
            </details>
            <details>
              <summary>
                Will it work on my Mac?<Icon name="plus" />
              </summary>
              <p>
                Kudzu needs macOS 14 Sonoma or later and runs on both Apple Silicon and Intel Macs.
                Download the universal DMG, move Kudzu to Applications, and follow the setup guide.
              </p>
            </details>
            <details>
              <summary>
                Where do my shortcuts work?<Icon name="plus" />
              </summary>
              <p>
                Kudzu expands text across your Mac, wherever an app supports text insertion. It
                includes per-app rules and insertion settings for apps that behave differently.
                Password managers are excluded by default, and macOS Secure Input can pause
                expansion.
              </p>
            </details>
            <details>
              <summary>
                Why does it need Accessibility permission?<Icon name="plus" />
              </summary>
              <p>
                Kudzu needs macOS permissions to recognize your shortcuts and insert their text into
                other apps. Onboarding walks you through the setup. Typed text is not stored; your
                snippet library and local clipboard history are separate.
              </p>
            </details>
            <details>
              <summary>
                Can I bring my Espanso snippets?<Icon name="plus" />
              </summary>
              <p>
                Absolutely. Kudzu can detect and import an existing Espanso library, including
                supported dates, clipboard variables, forms, and nested snippets. Unsupported script
                and rich-text features are imported as plain text with warnings so you can review
                them.
              </p>
            </details>
            <details>
              <summary>
                How do updates work?<Icon name="plus" />
              </summary>
              <p>
                Kudzu checks a few times a day, downloads new versions quietly, and waits for you to
                restart. Turn automatic checks off, or check by hand, in Settings → About. Your
                library gets a backup before an update migration.
              </p>
            </details>
          </div>
        </section>

        <section className="download-section wrap" id="download">
          <div className="download-card">
            <Image
              className="download-icon"
              src="/assets/kudzu-icon.png"
              alt="Kudzu app icon"
              width="112"
              height="112"
              loading="lazy"
            />
            <span className="eyebrow">MAKE ROOM FOR YOUR NEXT GOOD THOUGHT.</span>
            <h2>
              A little less typing.
              <br />
              <em>A little more living.</em>
            </h2>
            <DownloadAction />
            <p className="fine-print">
              Free. Open source. Yours.<span>·</span>macOS 14+
              {release && (
                <>
                  <span>·</span>Version {release.version}
                </>
              )}
            </p>
            <CopyCommand />
            <a className="release-link" href={site.releases}>
              All downloads on GitHub
            </a>
          </div>
          <ol className="install-steps" id="install" aria-label="After you download">
            <li>
              <span>1</span>
              <h3>Drag it to Applications.</h3>
              <p>Open the DMG and move Kudzu over. It’s signed and notarized, so it opens normally.</p>
            </li>
            <li>
              <span>2</span>
              <h3>Allow shortcut detection.</h3>
              <p>
                A helper opens beside System Settings. Turn Kudzu on in Accessibility so it can spot
                your triggers and insert text.
              </p>
            </li>
            <li>
              <span>3</span>
              <h3>Type your first shortcut.</h3>
              <p>Practice in the preview, then use it anywhere. Espanso users can import on the way.</p>
            </li>
          </ol>
        </section>
      </main>
    </>
  );
}
