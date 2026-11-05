# Kudzu website

The marketing site for **kudzu.site**, built with Next.js 16, React 19, TypeScript, and the App Router. It is the `kudzu-website` package in the root pnpm/Turborepo workspace.

## Development

Run from the repository root:

```sh
pnpm install
pnpm dev:website          # http://localhost:3001
pnpm check                # types and lint for both apps
pnpm build:website        # production build through Turbo
pnpm --filter kudzu-website start
```

The site uses server components for its pages and small client components for the snippet playground, clipboard search, template examples, app gallery, navigation, download button, and copy button. It has no analytics, database, or third-party browser requests. Sample content is illustrative and stays in the browser. The server reads release details from the GitHub API, caching them for an hour; no token is needed.

## Files

- `app/page.tsx`: home page composition and marketing sections.
- `app/download/route.ts`: redirects `/download` to the latest stable DMG, or to the releases page if GitHub can't be reached.
- `app/docs/`, `app/compare/`, `app/changelog/`, `app/privacy/`: inner pages. The changelog renders GitHub release notes as plain text.
- `app/globals.css`: responsive styles, tokens, and reduced-motion behavior.
- `components/`: header, footer, icons, and interactive previews.
- `lib/examples.ts`: typed sample snippets and clipboard items.
- `lib/site.ts`: URLs, navigation, metadata, and Homebrew command.
- `lib/releases.ts`, `lib/release-notes.ts`: cached GitHub release lookups and release note parsing. New releases appear without a redeploy.
- `lib/compare.ts`: comparison page content. Competitor details come from each vendor's own pages, so update the `checked` date whenever you confirm them again.
- `app/opengraph-image.tsx`, `robots.ts`, `sitemap.ts`: Next.js metadata routes.
- `public/assets/`: copies of Kudzu’s brand assets and locally hosted fonts, with their licenses. Brand sources remain in `apps/desktop/design/`.
- `public/assets/app/`: app screens for the gallery, captured from the desktop UI preview (`pnpm dev` in `apps/desktop`) with its sample data. Recapture them when the UI changes noticeably.

## Vercel

The Vercel project `kudzu-site` is connected to `Stringsaeed/kudzu` on GitHub. Its **Root Directory** is `apps/website`, with access to files outside that directory enabled so Vercel can read the root workspace and lockfile. The framework is **Next.js**.

- Install command: `cd ../.. && pnpm install --frozen-lockfile`
- Build command: `cd ../.. && pnpm exec turbo run lint typecheck --filter=kudzu-website && pnpm exec turbo run build --filter=kudzu-website`
- Output directory: Next.js default (`.next`)
- Domain: `kudzu.site`

The desktop package keeps its own Next.js static-export configuration. Website builds use the standard Next.js output, including the `/download` redirect and generated social image. No environment variables are required.

The commands above are tracked in `vercel.json`. Git pushes to feature branches create preview deployments; merging into `main` deploys to `kudzu.site`. The Vercel build runs website lint and type checks before building. No Vercel API token is stored in GitHub Actions.

Vercel's **Skip unaffected projects** setting remains enabled. `scripts/ignore-preview.mjs` adds a stricter preview filter, because Vercel otherwise treats files such as release workflows as global changes.

Preview builds run when these inputs change:

- `apps/website/`
- `packages/oxlint-config/` or `packages/typescript-config/`
- Root `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, or `turbo.json`
- Root `.npmrc`, `.node-version`, or `.nvmrc`

Desktop-only, release-workflow-only, and general documentation changes skip the preview build. Production builds keep their existing behavior. Vercel may still display a skipped/canceled deployment check; no preview is built in that case.

The filter compares with the last successful preview SHA. For a first preview it fetches enough history to compare the branch with `main`, including earlier commits in the same PR. If history cannot be established, it builds rather than risk dropping a website change. Run `pnpm --filter kudzu-website test:deployment` to verify the behavior against temporary Git repositories and shallow clones. Update the watched paths when introducing a new shared website package.

The project-level fallback is tracked in `scripts/preview-fallback.sh`. It allows production builds immediately, before any Git fetch, then fetches a pinned, tested filter for preview builds on older branches without the script. Branches with this `vercel.json` use their repository script. The regression tests check both the production bypass and Vercel’s 256-character command limit.

## Themes and preview assets

The header offers Light, Dark, and System modes. `lib/theme.ts` initializes the saved choice before paint; `components/theme-toggle.tsx` persists it in local storage and syncs changes across tabs. CSS `light-dark()` pairs cover the page and interactive previews. System mode follows the operating system automatically.

The hero uses the supplied 3220 × 2100 MacBook Neo Citrus PNG without modifying it. Its screen aperture is positioned at x=406, y=297, width=2408, height=1506. The interactive example scales inside the aperture. At 1100 px and narrower the bezel is hidden and the example renders at full width so text and controls stay readable. The example plays once when it scrolls into view, unless the visitor has already interacted or prefers reduced motion. The window chrome is a custom macOS-style frame, not an imported Figma component.

Local app icon sources:

- GitHub: `GitHub_Invertocat_Black.svg` from `https://brand.github.com/GitHub_Logos.zip`, stored unmodified and rendered as a theme-aware mask.
- Mail: the installed macOS Mail app’s `ApplicationIcon.icns`.
- Slack: `electron.icns` from the official Slack 4.53.280 macOS download, `https://slack.com/ssb/download-osx-silicon`.
- Craft: the installed macOS app’s `AppIcon.icns` (version 3.7.1).
- Visual Studio Code: the installed macOS app’s `Code.icns`.

The native app icons sit in a translucent Dock inside the laptop screen, alongside Kudzu. At 1100 px and narrower the Dock follows the full-width app preview. The Dock is decorative, so its icons don't link anywhere. They identify compatible apps and are not endorsement badges.
