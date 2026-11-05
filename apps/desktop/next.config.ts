import type { NextConfig } from "next";

const isProd = process.env.NODE_ENV === "production";

const nextConfig: NextConfig = {
  // Tauri serves the static export from disk; there is no Node server at runtime.
  output: "export",
  images: { unoptimized: true },
  reactStrictMode: true,
  // In dev, Tauri loads from the dev server; keep asset URLs absolute for both.
  assetPrefix: isProd ? undefined : undefined,
  devIndicators: false,
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
};

export default nextConfig;
