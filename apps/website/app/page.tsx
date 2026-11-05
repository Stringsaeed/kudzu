import Image from "next/image";
import { AppleMark } from "@/components/apple-mark";
import { SketchArrow } from "@/components/sketch-arrow";
import { site } from "@/lib/site";
import { IconDefinitions } from "@/components/icons";
import { SiteHeader } from "@/components/site-header";
import { SiteFooter } from "@/components/site-footer";
import { SnippetPlayground } from "@/components/snippet-playground";
import { ClipboardPreview } from "@/components/clipboard-preview";
import { TemplateExamples } from "@/components/template-examples";
import { CopyCommand } from "@/components/copy-command";

export default function Home() {
  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <IconDefinitions />
      <SiteHeader />
      <main id="main">
        <section className="hero wrap" aria-labelledby="hero-title">
          <a className="release-pill" href="https://github.com/Stringsaeed/kudzu/releases/latest">
            <span className="status-dot"></span>A little less typing. A little more you.
          </a>
          <h1 id="hero-title">
            For things
            <br />
            worth <span className="hero-accent">repeating.</span>
            <SketchArrow variant="up" className="echo-mark" />
          </h1>
          <p className="hero-description">
            Your best words, a few keystrokes away.
            <br />
            Text shortcuts and clipboard history that feel right at home on your Mac.
          </p>
          <div className="hero-actions">
            <a className="button button-primary" href={site.download}>
              <AppleMark />
              Download for Mac
            </a>
            <a className="button button-text" href="#playground">
              Give it a spin{" "}
            </a>
          </div>
          <p className="fine-print">
            Free & open source<span>·</span>macOS 14+<span>·</span>Apple Silicon & Intel
          </p>

          <SnippetPlayground />
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

        <section className="feature-row wrap" aria-labelledby="shortcut-title">
          <div className="feature-copy">
            <span className="feature-number">01 / TEXT SHORTCUTS</span>
            <h2>
              A few keys.
              <br />A whole lot said.
            </h2>
            <p>
              Turn the things you type all the time into shortcuts you’ll actually remember. Email
              replies, addresses, code, your very particular sign-off. Write it once. Make it yours.
            </p>
            <a className="inline-link" href="#playground">
              Try your first shortcut{" "}
            </a>
            <div className="feature-detail">
              <svg className="icon">
                <use href="#i-check" />
              </svg>
              Changed your mind? Backspace undoes an expansion.
            </div>
          </div>
          <div className="shortcut-art feature-art" aria-labelledby="shortcut-title">
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
              <span>↓</span>
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
          </div>
        </section>

        <section className="feature-row reverse wrap" aria-labelledby="clipboard-title">
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
          <ClipboardPreview />
        </section>

        <TemplateExamples />

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
              <a
                className="inline-link"
                href="https://github.com/Stringsaeed/kudzu"
                target="_blank"
                rel="noopener noreferrer"
              >
                Open source. Come have a look.{" "}
              </a>
            </div>
            <div className="privacy-notes">
              <article>
                <span>01</span>
                <div>
                  <h3>Local feels right.</h3>
                  <p>
                    Your clipboard history stays on your Mac. Password managers are excluded by
                    default, and concealed clipboard content is skipped.
                  </p>
                </div>
              </article>
              <article>
                <span>02</span>
                <div>
                  <h3>Your Macs, in sync.</h3>
                  <p>
                    Keep your snippet library with you through iCloud. Your words travel between
                    your Macs through your own iCloud account.
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
        </section>

        <section className="faq-section wrap" aria-labelledby="faq-title">
          <div>
            <span className="eyebrow">THE LAST FEW THINGS</span>
            <h2 id="faq-title">Glad you asked.</h2>
            <p>Little app. Straight answers.</p>
          </div>
          <div className="faq-list">
            <details>
              <summary>
                Actually free?<span aria-hidden="true">+</span>
              </summary>
              <p>
                Yes. Kudzu is free and open source under the MIT license. No subscription, no
                account, and no paid tier to unlock the features you came for.
              </p>
            </details>
            <details>
              <summary>
                Will it work on my Mac?<span aria-hidden="true">+</span>
              </summary>
              <p>
                Kudzu needs macOS 14 Sonoma or later and runs on both Apple Silicon and Intel Macs.
                Download the universal DMG, move Kudzu to Applications, and follow the setup guide.
              </p>
            </details>
            <details>
              <summary>
                Where do my shortcuts work?<span aria-hidden="true">+</span>
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
                Why does it need Accessibility permission?<span aria-hidden="true">+</span>
              </summary>
              <p>
                Kudzu needs macOS permissions to recognize your shortcuts and insert their text into
                other apps. Onboarding walks you through the setup. Typed text is not stored; your
                snippet library and local clipboard history are separate.
              </p>
            </details>
            <details>
              <summary>
                Can I bring my Espanso snippets?<span aria-hidden="true">+</span>
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
                How do updates work?<span aria-hidden="true">+</span>
              </summary>
              <p>
                Kudzu checks for updates and lets you install and restart when you’re ready. You can
                also check manually in Settings → About. Your library gets a backup before an update
                migration.
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
            <a className="button button-primary" href={site.download}>
              <AppleMark />
              Get Kudzu for Mac
            </a>
            <p className="fine-print">
              Free. Open source. Yours.<span>·</span>macOS 14+
            </p>
            <CopyCommand />
            <a className="release-link" href="https://github.com/Stringsaeed/kudzu/releases/latest">
              Release notes & other downloads
            </a>
          </div>
        </section>
      </main>
      <SiteFooter />
    </>
  );
}
