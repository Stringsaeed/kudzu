// Competitor details come from each vendor's own pages. Update `checked` and the rows together,
// and only add a row when it can be confirmed from a primary source.

export type Comparison = {
  slug: string;
  name: string;
  migration: string;
  summary: string;
  rows: { label: string; kudzu: string; other: string }[];
  chooseKudzu: string[];
  chooseOther: string[];
  moving: { title: string; steps: string[] };
  sources: { label: string; href: string }[];
};

export const checked = "November 6, 2026";

export const comparisons: Comparison[] = [
  {
    slug: "espanso",
    name: "Espanso",
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
        "Install Kudzu. Onboarding detects your Espanso library and offers to import it.",
        "Dates, clipboard variables, forms, and nested snippets carry over.",
        "Scripts and rich text are imported as plain text, with warnings so you can review them.",
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
    migration: "No importer yet. Bring your favorites over by hand.",
    summary:
      "TextExpander is a long-running service that spans several platforms and offers team plans. Kudzu is a free, open-source Mac app with no account.",
    rows: [
      {
        label: "Price",
        kudzu: "Free",
        other: "Free plan, plus paid Pro and Business plans per user",
      },
      {
        label: "Platforms",
        kudzu: "macOS 14 or later",
        other: "Mac, Windows, Chrome, iPhone, iPad",
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
      "You need your snippets on Windows, in Chrome, or on an iPhone or iPad.",
      "Your team shares snippets and needs admin features.",
    ],
    moving: {
      title: "Moving from TextExpander",
      steps: [
        "Kudzu has no TextExpander importer yet, so start with the snippets you use most.",
        "Recreate them in Kudzu’s editor. Most people rely on a small set.",
        "Your own trigger habits still work: Kudzu triggers can be any text you like.",
      ],
    },
    sources: [{ label: "TextExpander pricing", href: "https://textexpander.com/pricing" }],
  },
  {
    slug: "raycast",
    name: "Raycast",
    migration: "No importer yet. Bring your favorites over by hand.",
    summary:
      "Raycast is a launcher that does many things, and snippets and clipboard history are two of them. Kudzu does just those two, in a dedicated app.",
    rows: [
      {
        label: "Price",
        kudzu: "Free",
        other: "Free plan. Pro is $10 a month or $96 a year",
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
        "Kudzu has no Raycast importer yet, so start with the snippets you use most.",
        "Recreate them in Kudzu’s editor, keeping the same triggers.",
        "Turn off Raycast’s snippet expansion so both apps don’t fire on the same trigger.",
      ],
    },
    sources: [{ label: "Raycast pricing", href: "https://www.raycast.com/pricing" }],
  },
];

export const switching = comparisons.map(({ slug, name, migration }) => ({
  slug,
  name,
  migration,
}));
