# Header structure prototype

Question: which navigation structure should Kudzu use above the existing hero?

Created with `npx ui-skills get variant` (jakubkrehel) and `npx ui-skills get mattpocock/prototype`.

Run `pnpm dev:website`, then open `/?__variant=dock`, `/?__variant=centered`, or `/?__variant=editorial`. The picker supports arrow keys and H to hide. It is available only in development and Vercel preview environments.

The user selected **Dock**, with a request to fix mobile. Promote a compact responsive Dock into the production header and keep this comparison branch out of main. Centered and Editorial are retained here as design evidence, not production components.
