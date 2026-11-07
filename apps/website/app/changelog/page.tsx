import type { Metadata } from "next";
import { DownloadAction } from "@/components/download-action";
import { pageMetadata } from "@/lib/metadata";
import { Icon } from "@/components/icons";
import { getReleases, releaseAnchor } from "@/lib/releases";
import { parseReleaseNotes } from "@/lib/release-notes";
import { site } from "@/lib/site";

export const revalidate = 3600;

export const metadata: Metadata = pageMetadata({
  title: "Changelog",
  description: "What’s new in each Kudzu release.",
  path: "/changelog",
});

const dateFormat = new Intl.DateTimeFormat("en-US", {
  dateStyle: "long",
  timeZone: "UTC",
});

export default async function ChangelogPage() {
  const releases = await getReleases();
  return (
    <main id="main" className="page wrap">
      <header className="page-heading">
        <span className="eyebrow">CHANGELOG</span>
        <h1>What’s new.</h1>
        <p>Kudzu downloads new versions quietly and waits for you to restart.</p>
        <DownloadAction />
      </header>
      <div className="changelog">
        {releases.map((release, index) => (
          <article key={release.version} id={releaseAnchor(release.version)}>
            <div className="changelog-meta">
              <h2>
                <a href={`#${releaseAnchor(release.version)}`}>{release.version}</a>
                {index === 0 && (
                  <>
                    {" "}
                    <span className="latest-badge">Latest</span>
                  </>
                )}
              </h2>
              {release.publishedAt && (
                <time dateTime={release.publishedAt}>
                  {dateFormat.format(new Date(release.publishedAt))}
                </time>
              )}
              <a
                className="inline-link"
                href={release.url}
                target="_blank"
                rel="noopener noreferrer"
              >
                On GitHub
                <Icon name="arrow" />
              </a>
            </div>
            <div className="prose">
              {parseReleaseNotes(release.body).map((section) => (
                <section key={section.title}>
                  <h3 className="note-label" data-kind={section.title.toLowerCase()}>
                    {section.title}
                  </h3>
                  <ul>
                    {section.items.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                </section>
              ))}
            </div>
          </article>
        ))}
      </div>
      <p className="page-note">
        <a href={site.allReleases}>Every release, including betas, is on GitHub</a>.
      </p>
    </main>
  );
}
