import type { NextConfig } from "next";
import { site } from "./lib/site";

const config: NextConfig = {
  devIndicators: false,
  async redirects() {
    return [{ source: "/download", destination: site.download, permanent: false }];
  },
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          { key: "X-Frame-Options", value: "DENY" },
        ],
      },
    ];
  },
};

export default config;
