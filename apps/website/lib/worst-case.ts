// Development only. Worst-case data for stress-testing the UI ("break-ui"). Toggle it with the
// "Demo data / Worst case" switch that `pnpm dev` shows in the corner. It's a cookie, read on the
// server, and every check is behind NODE_ENV, so production builds never read it.
import { cookies } from "next/headers";
import type { Comparison } from "./compare";
import type { clipboardItems, snippets } from "./examples";
import type { Release } from "./releases";

export const worstCaseCookie = "kudzu-worst-case";

export async function worstCase() {
  if (process.env.NODE_ENV !== "development") return false;
  return (await cookies()).get(worstCaseCookie)?.value === "1";
}

const repo = "https://github.com/Stringsaeed/kudzu";
const commit = (sha: string) => `([${sha.slice(0, 7)}](${repo}/commit/${sha}))`;

/** Shaped like real release-please notes, plus the hand-written and auto-generated kinds. */
export const worstReleases: Release[] = [
  {
    version: "10.12.103",
    url: `${repo}/releases/tag/v10.12.103`,
    publishedAt: "2027-06-30T23:59:59Z",
    dmgUrl: null,
    body: [
      `## [10.12.103](${repo}/compare/v10.11.0...v10.12.103) (2027-06-30)`,
      "",
      "### ⚠ BREAKING CHANGES",
      "",
      `* **sync:** snippets synced from Kudzu 9.x need a one-time re-upload, and Macs still on 9.4 or earlier stop receiving changes until they update ${commit("abc1234def5678abc1234def5678abc1234def56")}`,
      "",
      "### Features",
      "",
      `* **clipboard:** keep pinned items across restores, including images copied from Preview, screenshots dragged out of the floating thumbnail, and files copied in Finder ${commit("1234567890abcdef1234567890abcdef12345678")}`,
      `* **desktop:** open \`kudzu://snippet/new?trigger=;sig&group=Everyday&name=Email%20signature\` links from Shortcuts ${commit("2345678901bcdef02345678901bcdef023456789")}`,
      `* add Українська, Polski, 日本語, and العربية translations, thanks to Aleksandra Wiśniewska-Kowalczyk ${commit("3456789012cdef013456789012cdef0134567890")}`,
      "  * nested: the Arabic translation lays out right to left",
      "",
      "### Fixes",
      "",
      `* **deps:** update dependencies ${commit("4567890123def0124567890123def01245678901")}`,
      `* **deps:** update dependencies ${commit("5678901234ef01235678901234ef012356789012")}`,
      `* **engine:** stop hanging on NSPasteboardTypeFileURLPromiseAttachmentsFromMailComposeWindow, reported in ${repo}/issues/1284#issuecomment-2187654321 ${commit("6789012345f012346789012345f0123467890123")}`,
      "",
      "### Performance Improvements",
      "",
      `* **clipboard:** open history 3× faster with 5,000 items ${commit("7890123456012345789012345601234578901234")}`,
      "",
      "### Reverts",
      "",
      `* revert "feat(engine): type short snippets instead of pasting" ${commit("8901234567123456890123456712345689012345")}`,
    ].join("\n"),
  },
  {
    version: "10.12.2",
    url: `${repo}/releases/tag/v10.12.2`,
    publishedAt: null,
    dmgUrl: null,
    body: "",
  },
  {
    version: "10.12.1",
    url: `${repo}/releases/tag/v10.12.1`,
    publishedAt: "2027-06-01T08:00:00Z",
    dmgUrl: null,
    body: "Hotfix: the 10.12.0 DMG was signed with the wrong certificate. If macOS refused to open it, download it again.",
  },
  {
    version: "10.12.0",
    url: `${repo}/releases/tag/v10.12.0`,
    publishedAt: "2027-05-31T08:00:00Z",
    dmgUrl: null,
    body: [
      "## What's Changed",
      `* Fix a typo in the README by @bartholomew-fitzgerald-northwind in ${repo}/pull/42`,
      "",
      `**Full Changelog**: ${repo}/compare/v10.11.0...v10.12.0`,
    ].join("\n"),
  },
];

/** A comparison as it might read after a few more rounds of edits. */
export function worstComparison(base: Comparison): Comparison {
  return {
    ...base,
    name: `${base.name} Pro for Teams`,
    summary: `${base.summary} ${base.summary}`,
    rows: [
      ...base.rows,
      {
        label: "Clipboard history kept on the free plan, including images and files",
        kudzu:
          "From a day to forever, up to 5,000 unpinned items. Pinned items never expire, and images, files, and colors are kept too.",
        other: "—",
      },
      { label: "Offline", kudzu: "Yes", other: "Yes" },
    ],
    chooseOther: [],
    sources: [
      ...base.sources,
      { label: `${base.name} help center: snippets and clipboard`, href: "https://example.com/a" },
      { label: `${base.name} security whitepaper (PDF)`, href: "https://example.com/b" },
    ],
  };
}

type Snippet = (typeof snippets)[number];

export const worstSnippets: Snippet[] = [
  {
    id: "long",
    group: "work",
    favorite: true,
    name: "Follow-up after the quarterly planning meeting with Northwind Industries",
    trigger: ";northwind-quarterly-followup",
    icon: "message",
    description:
      "Recap, owners, and next steps for bartholomew.fitzgerald@northwind-industries-holdings.example.com",
    text: "Hi Bartholomew,\n\nThanks for the time today. Recap and owners are in https://docs.example.com/northwind-industries-holdings/q3-planning/2027-06-30-meeting-notes-final-v12.\n\nAleksandra Wiśniewska-Kowalczyk will send the revised forecast by Friday.\n\nBest,\nAlex",
  },
  {
    id: "short",
    group: "everyday",
    favorite: false,
    name: "Jo",
    trigger: ";em",
    icon: "pen",
    description: "",
    text: "—",
  },
  {
    id: "arabic",
    group: "everyday",
    favorite: false,
    name: "ردّ سريع على العميل",
    trigger: ";rd",
    icon: "heart",
    description: "شكرًا على رسالتك، سأعود إليك قريبًا.",
    text: "شكرًا على رسالتك.\n\nسأعود إليك قبل نهاية الأسبوع.",
  },
  {
    id: "emoji",
    group: "work",
    favorite: true,
    name: "🎉 Launch-day announcement",
    trigger: ";launch",
    icon: "clock",
    description: "For Slack #general 🚀",
    text: "🎉 We’re live! Thank you all 🙏",
  },
  {
    id: "addr",
    group: "everyday",
    favorite: false,
    name: "Home address",
    trigger: ";addr",
    icon: "pen",
    description: "Street, city, postcode",
    text: "Flat 4B, 1284 Avenue of the Americas\nNew York, NY 10019-6022\nUnited States",
  },
  {
    id: "one",
    group: "work",
    favorite: false,
    name: "Single character",
    trigger: ";x",
    icon: "heart",
    description: "Exactly one character long",
    text: "×",
  },
];

type ClipItem = (typeof clipboardItems)[number];

export const worstClipboardItems: ClipItem[] = [
  {
    id: "text",
    kind: "Text",
    symbol: "Aa",
    content:
      "Aleksandra Wiśniewska-Kowalczyk, bartholomew.fitzgerald@northwind-industries-holdings.example.com",
    time: "3 weeks ago",
    search: "text name email",
  },
  {
    id: "link",
    kind: "Link",
    symbol: "",
    icon: "link",
    content:
      "https://github.com/Stringsaeed/kudzu/blob/main/apps/macos/Packages/KudzuKit/Sources/KudzuCore/Matching/KeyBuffer.swift",
    time: "a moment ago",
    search: "link github",
  },
  {
    id: "color",
    kind: "Color",
    symbol: "",
    content: "#FFFFFF",
    time: "1 minute ago",
    search: "color white",
  },
  {
    id: "code",
    kind: "Code",
    symbol: "{ }",
    content: "شكرًا على رسالتك، سأعود إليك قريبًا 🙏",
    time: "11 months ago",
    search: "code arabic",
  },
];
