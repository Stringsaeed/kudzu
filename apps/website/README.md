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

The site uses server components for the page and small client components for the snippet playground, clipboard search, template examples, and copy button. It has no analytics, database, or third-party browser requests. Sample content is illustrative and stays in the browser.

## Files

- `app/page.tsx`: page composition and marketing sections.
- `app/globals.css`: responsive styles, tokens, and reduced-motion behavior.
- `components/`: header, footer, icons, and interactive previews.
- `lib/examples.ts`: typed sample snippets and clipboard items.
- `lib/site.ts`: URLs, download target, metadata, and Homebrew command. Update the verified DMG target here when a new stable version ships.
- `app/opengraph-image.tsx`, `robots.ts`, `sitemap.ts`: Next.js metadata routes.
- `public/assets/`: copies of Kudzu’s brand assets and locally hosted fonts, with their licenses. Brand sources remain in `apps/desktop/design/`.

## Vercel

The Vercel project `kudzu-site` is connected to `Stringsaeed/kudzu` on GitHub. Its **Root Directory** is `apps/website`, with access to files outside that directory enabled so Vercel can read the root workspace and lockfile. The framework is **Next.js**.

- Install command: `cd ../.. && pnpm install --frozen-lockfile`
- Build command: `cd ../.. && pnpm exec turbo run lint typecheck --filter=kudzu-website && pnpm exec turbo run build --filter=kudzu-website`
- Output directory: Next.js default (`.next`)
- Domain: `kudzu.site`

The desktop package keeps its own Next.js static-export configuration. Website builds use the standard Next.js output, including the `/download` redirect and generated social image. No environment variables are required.

The commands above are tracked in `vercel.json`. Git pushes to feature branches create preview deployments; merging into `main` deploys to `kudzu.site`. The Vercel build runs website lint and type checks before building. No Vercel API token is stored in GitHub Actions.

Vercel's **Skip unaffected projects** setting is enabled. It follows the pnpm dependency graph: website changes and changes to its shared config packages deploy; changes confined to `apps/desktop` are skipped. Relevant lockfile changes and global files outside the workspace packages can also trigger a build. Keep internal workspace dependencies explicit in `package.json` so this detection stays accurate.

## Themes and preview assets

The header offers Light, Dark, and System modes. `lib/theme.ts` initializes the saved choice before paint; `components/theme-toggle.tsx` persists it in local storage and syncs changes across tabs. CSS `light-dark()` pairs cover the page and interactive previews. System mode follows the operating system automatically.

The hero uses the supplied 3220 × 2100 MacBook Neo Citrus PNG without modifying it. Its screen aperture is positioned at x=406, y=297, width=2408, height=1506. The interactive example scales inside the aperture. On narrow screens the bezel is hidden and the example renders at full width so controls remain usable. The window chrome is a custom macOS-style frame, not an imported Figma component.

Local app icon sources:

- Mail: the installed macOS Mail app’s `ApplicationIcon.icns`.
- Slack: `https://a.slack-edge.com/9cc0056/img/icons/favicon-32.png`.
- Notion: `https://www.notion.so/images/logo-ios.png`.
- Visual Studio Code: `https://code.visualstudio.com/assets/branding/code-stable.png`.

The icons link to their respective products. They identify compatible apps and are not endorsement badges.
