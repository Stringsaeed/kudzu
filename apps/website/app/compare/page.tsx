import Link from "next/link";
import type { Metadata } from "next";
import { Icon } from "@/components/icons";
import { comparisons } from "@/lib/compare";

const description =
  "How Kudzu compares with Espanso, TextExpander, and Raycast, including when another app is the better choice.";

export const metadata: Metadata = {
  title: "Compare Kudzu",
  description,
  alternates: { canonical: "/compare" },
};

export default function ComparePage() {
  return (
    <main id="main" className="page wrap">
      <header className="page-heading">
        <span className="eyebrow">COMPARE</span>
        <h1>Find the right tool, even if it isn’t this one.</h1>
        <p>{description}</p>
      </header>
      <div className="switch-grid">
        {comparisons.map((item) => (
          <Link key={item.slug} href={`/compare/${item.slug}`}>
            <strong>Kudzu vs {item.name}</strong>
            <span>{item.summary}</span>
            <em>
              Read the comparison
              <Icon name="arrow" />
            </em>
          </Link>
        ))}
      </div>
    </main>
  );
}
