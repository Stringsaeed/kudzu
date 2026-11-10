// Competitor details come from each vendor's own pages. Update `checked` and the rows together,
// and only add a row when it can be confirmed from a primary source.

export type Comparison = {
  slug: string;
  name: string;
  /** Search-facing page title. Says "alternative" because that's what people searching type. */
  seoTitle: string;
  /** When these details were last confirmed against the vendor's own pages. */
  checked: string;
  migration: string;
  summary: string;
  rows: { label: string; kudzu: string; other: string }[];
  chooseKudzu: string[];
  chooseOther: string[];
  moving: { title: string; steps: string[] };
  sources: { label: string; href: string }[];
};

export const comparisons: Comparison[] = [
  {
    slug: "espanso",
    name: "Espanso",
    seoTitle: "Espanso alternative for Mac: Kudzu vs Espanso",
    checked: "November 6, 2026",
    migration: "Kudzu finds your Espanso library and imports it.",
    summary:
      "Both are free and open source. Espanso is a cross-platform tool you configure in YAML files. Kudzu is a Mac app with a visual editor and clipboard history built in.",
    rows: [
      { label: "Price", kudzu: "Free", other: "Free" },
      { label: "License", kudzu: "MIT", other: "GPL-3.0" },
      { label: "Platforms", kudzu: "macOS 14 or later", other: "Windows, macOS, Linux" },
      {
        label: "How you edit snippets",
        kudzu: "A visual editor in the app",
        other: "YAML files in your config folder",
      },
      {
        label: "Scripts and shell commands",
        kudzu: "Not supported",
        other: "Supported",
      },
    ],
    chooseKudzu: [
      "You want a native-feeling Mac app instead of config files.",
      "You want clipboard history and text shortcuts in one place.",
      "You want iCloud sync between Macs without managing files yourself.",
    ],
    chooseOther: [
      "You also work on Windows or Linux.",
      "Your snippets run shell commands or scripts.",
      "You like keeping your configuration as text in version control.",
    ],
    moving: {
      title: "Moving from Espanso",
      steps: [
        "Install Kudzu. Onboarding detects your Espanso library and offers to import it, after taking a backup.",
        "Dates, clipboard variables, forms, and nested snippets carry over.",
        "Bold, italic, underline, links, and lists in markdown and HTML matches carry over too. Scripts are imported as plain text, with warnings so you can review them.",
        "Pause Espanso before you try your triggers, so both apps don’t expand the same one. Keep your Espanso files until you’re happy.",
      ],
    },
    sources: [
      { label: "Espanso: Getting started", href: "https://espanso.org/docs/get-started/" },
      { label: "Espanso on GitHub", href: "https://github.com/espanso/espanso" },
    ],
  },
  {
    slug: "textexpander",
    name: "TextExpander",
    seoTitle: "TextExpander alternative for Mac: Kudzu vs TextExpander",
    checked: "November 8, 2026",
    migration: "Kudzu imports your TextExpander groups, with fill-ins and formatting.",
    summary:
      "TextExpander is a long-running service that spans several platforms and offers team plans. Kudzu is a free, open-source Mac app with no account.",
    rows: [
      {
        label: "Price",
        kudzu: "Free",
        other:
          "Free plan with 5 active snippets. Pro is $6 and Business $12 per user a month, billed yearly",
      },
      {
        label: "Platforms",
        kudzu: "macOS 14 or later",
        other: "Mac, Windows, Chrome, iPhone, iPad, and Android in beta",
      },
      { label: "Sign-up", kudzu: "No account", other: "Plans start with a sign-up" },
      { label: "Source code", kudzu: "Open source (MIT)", other: "Proprietary" },
    ],
    chooseKudzu: [
      "You work on a Mac and want everything free, with no account.",
      "You want to read the code of an app that watches for your shortcuts.",
      "You want your snippets kept on your Mac and in your own iCloud.",
    ],
    chooseOther: [
      "You need your snippets on Windows, in Chrome, or on a phone or tablet.",
      "Your team shares snippets and needs admin features.",
    ],
    moving: {
      title: "Moving from TextExpander",
      steps: [
        "In TextExpander, export a group as a .textexpander file or as CSV.",
        "In Kudzu, open Settings → Backups & data, choose Import, and pick the file. Kudzu takes a backup first.",
        "Abbreviations, dates, the clipboard, fill-ins, pop-up menus, Tab and Return key presses, and nested snippets carry over. So do bold, italic, underline, links, and lists.",
        "AppleScript, JavaScript, and shell snippets are skipped, with a warning for each so you can review them.",
        "Pause TextExpander before you try your abbreviations, so both apps don’t expand the same one.",
      ],
    },
    sources: [
      { label: "TextExpander pricing", href: "https://textexpander.com/pricing" },
      { label: "TextExpander downloads", href: "https://textexpander.com/download" },
    ],
  },
  {
    slug: "raycast",
    name: "Raycast",
    seoTitle: "Raycast snippets alternative: Kudzu vs Raycast",
    checked: "November 8, 2026",
    migration: "Keep Raycast as your launcher. Hand the snippets to Kudzu.",
    summary:
      "Raycast is a launcher that does many things, and snippets and clipboard history are two of them. Kudzu does just those two, in a dedicated app.",
    rows: [
      {
        label: "Price",
        kudzu: "Free",
        other: "Snippets are free. Pro is $10 a month or $96 a year",
      },
      {
        label: "Clipboard history",
        kudzu: "Free. Keep it from 1 day to forever, up to 5,000 unpinned items",
        other: "3 months on the free plan, unlimited on Pro",
      },
      {
        label: "What it is",
        kudzu: "A dedicated text expander and clipboard manager",
        other: "A launcher with snippets, clipboard history, and much more",
      },
      { label: "Source code", kudzu: "Open source (MIT)", other: "Proprietary" },
    ],
    chooseKudzu: [
      "You want a focused app rather than a launcher you’d have to adopt.",
      "You want long clipboard history without paying for it.",
      "You want formatted snippets, per-app rules, and insights in one place.",
    ],
    chooseOther: [
      "You already live in Raycast and want everything in one launcher.",
      "You want its extensions and AI features.",
    ],
    moving: {
      title: "Moving from Raycast",
      steps: [
        "In Raycast, run Export Snippets to save them as a JSON file.",
        "In Kudzu, open Settings → Backups & data, choose Import, and pick the file. Your snippets arrive in a Raycast group with the same keywords.",
        "Dates, the clipboard, the cursor position, arguments, the selection, and nested snippets carry over. Anything Kudzu can’t express is kept as text, with a warning.",
        "Turn off Raycast’s snippet expansion so both apps don’t fire on the same keyword.",
      ],
    },
    sources: [
      { label: "Raycast pricing", href: "https://www.raycast.com/pricing" },
      { label: "Raycast snippets", href: "https://manual.raycast.com/snippets" },
    ],
  },
];

export const switching = comparisons.map(({ slug, name, migration }) => ({
  slug,
  name,
  migration,
}));
