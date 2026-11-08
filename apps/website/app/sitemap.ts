import type { MetadataRoute } from "next";
import { comparisons } from "@/lib/compare";
import { site } from "@/lib/site";

export default function sitemap(): MetadataRoute.Sitemap {
  const pages = ["/docs", "/docs/templates", "/docs/ai-agents", "/changelog", "/compare", "/privacy"];
  return [
    { url: site.url, changeFrequency: "weekly", priority: 1 },
    ...pages.map((page) => ({ url: `${site.url}${page}`, changeFrequency: "monthly" as const })),
    ...comparisons.map(({ slug }) => ({
      url: `${site.url}/compare/${slug}`,
      changeFrequency: "monthly" as const,
    })),
  ];
}
