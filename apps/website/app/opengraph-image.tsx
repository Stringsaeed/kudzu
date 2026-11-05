import { ImageResponse } from "next/og";

export const alt =
  "Kudzu — For things worth repeating. Free text shortcuts and clipboard history for Mac.";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function OpenGraphImage() {
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
      }}
    >
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
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 24 }}>
        <span>Text shortcuts & clipboard history for Mac.</span>
        <span>Free & open source</span>
      </div>
    </div>,
    size,
  );
}
