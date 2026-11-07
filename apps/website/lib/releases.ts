// Reads published releases from GitHub on the server. Results are cached for an hour, so the site
// follows new releases without a redeploy. Every failure resolves to null or an empty list, and
// callers fall back to the GitHub releases page.

import { worstCase, worstReleases } from "./worst-case";

const api = "https://api.github.com/repos/Stringsaeed/kudzu/releases";
const downloadPrefix = "https://github.com/Stringsaeed/kudzu/releases/download/";

type GitHubRelease = {
  tag_name: string;
  html_url: string;
  published_at: string | null;
  body: string | null;
  draft: boolean;
  prerelease: boolean;
  assets: { name: string; browser_download_url: string }[];
};

export type Release = {
  version: string;
  url: string;
  publishedAt: string | null;
  body: string;
  dmgUrl: string | null;
};

async function request<T>(path: string): Promise<T | null> {
  try {
    const response = await fetch(`${api}${path}`, {
      headers: {
        Accept: "application/vnd.github+json",
        "User-Agent": "kudzu-website",
        "X-GitHub-Api-Version": "2022-11-28",
      },
      next: { revalidate: 3600 },
      signal: AbortSignal.timeout(5000),
    });
    return response.ok ? ((await response.json()) as T) : null;
  } catch {
    return null;
  }
}

function toRelease(release: GitHubRelease): Release {
  const dmg = release.assets.find(
    (asset) =>
      asset.name.endsWith("_universal.dmg") && asset.browser_download_url.startsWith(downloadPrefix),
  );
  return {
    version: release.tag_name.replace(/^v/, ""),
    url: release.html_url,
    publishedAt: release.published_at,
    body: release.body ?? "",
    dmgUrl: dmg?.browser_download_url ?? null,
  };
}

/** The changelog's fragment id for a version, so a release can be linked directly. */
export function releaseAnchor(version: string) {
  return `v${version.replace(/[^\w.-]/g, "-")}`;
}

/** GitHub's "latest" is the newest stable release, so betas and the update feed are skipped. */
export async function getLatestRelease(): Promise<Release | null> {
  if (await worstCase()) return worstReleases[0];
  const release = await request<GitHubRelease>("/latest");
  return release ? toRelease(release) : null;
}

export async function getReleases(): Promise<Release[]> {
  if (await worstCase()) return worstReleases;
  const releases = await request<GitHubRelease[]>("?per_page=30");
  return (releases ?? [])
    .filter((release) => !release.draft && !release.prerelease && /^v\d/.test(release.tag_name))
    .map(toRelease);
}
