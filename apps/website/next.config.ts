import type { NextConfig } from "next";

const config: NextConfig = {
  devIndicators: false,
  images: {
    // 2400 fills the 2x slot for the 1160px stage, so retina screens don't pull the 3840 source.
    deviceSizes: [640, 828, 1080, 1200, 1440, 1920, 2400],
    formats: ["image/avif", "image/webp"],
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
