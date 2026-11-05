/* oxlint-disable nextjs/no-img-element -- ImageResponse renders raw image elements into PNG. */
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { ImageResponse } from "next/og";

export const alt =
  "Kudzu — For things worth repeating. Free text shortcuts and clipboard history for Mac.";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default async function OpenGraphImage() {
  const [icon, wordmark] = await Promise.all([
    readFile(join(process.cwd(), "public/assets/kudzu-icon.png")),
    readFile(join(process.cwd(), "public/assets/kudzu-wordmark.svg")),
  ]);
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: "#f7f7f2",
        color: "#242624",
        padding: "60px 80px",
        justifyContent: "space-between",
        position: "relative",
        overflow: "hidden",
      }}
    >
      <img
        src={`data:image/svg+xml;base64,${wordmark.toString("base64")}`}
        width={1120}
        height={306}
        alt=""
        style={{ position: "absolute", left: 40, bottom: -75, opacity: 0.12 }}
      />
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 30 }}>
        <span style={{ fontWeight: 700 }}>kudzu</span>
        <span style={{ color: "#64685e", fontSize: 22 }}>
          A little less typing. A little more you.
        </span>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          fontSize: 104,
          lineHeight: 1,
          letterSpacing: "-6px",
        }}
      >
        <span>For things</span>
        <span style={{ color: "#3255e7" }}>worth repeating.</span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 24, fontSize: 23 }}>
        <img
          src={`data:image/png;base64,${icon.toString("base64")}`}
          width={96}
          height={96}
          alt=""
        />
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span>Text shortcuts & clipboard history</span>
          <span style={{ color: "#64685e" }}>for your Mac.</span>
        </div>
        <span style={{ marginLeft: "auto", fontSize: 20 }}>Free & open source</span>
      </div>
    </div>,
    size,
  );
}
