# Kudzu identity

Kudzu is a macOS text expander. The identity is typographic: the Echo wordmark, restrained blue, and neutral surfaces.

Use `design/wordmark/kudzu-wordmark-echo.svg` when there is room for its full detail. Use the small version at compact sizes. Both use currentColor and can follow the interface’s foreground color.

The app icon places the wordmark’s lowercase k on the existing porcelain keycap. Its composition is editable in `Kudzu.icon`; the letter remains a vector layer. Menu-bar icons use the filled k silhouette with pause, permission, and secure-input indicators.

Edit the native document in Icon Composer. Run `scripts/update-design-assets.sh` from desktop to refresh the application icons, favicon, menu-bar assets, public wordmarks, and installer background. `brand-assets.py` derives the compact letter artwork from the canonical wordmark.

The letter’s visible ink is optically centered on the top face, with vertical compression and a subtle perspective fit. Its layer uses Multiply in Icon Composer to retain the keycap shading. `brand-assets.py` prepares `02-echo-k-printed.svg`; use Image → Replace in Icon Composer when changing that source. The export script preserves native placement and blend settings.
