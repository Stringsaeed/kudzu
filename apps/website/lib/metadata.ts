import type { Metadata } from "next";
import { site } from "./site";

/**
 * Metadata for an inner page. Next.js merges `openGraph` and `twitter` shallowly from the root
 * layout, so without these every shared link would carry the homepage's title and URL.
 * `absolute` skips the "— Kudzu" suffix for titles that already name the product.
 */
export function pageMetadata({
  title,
  description,
  path,
  absolute = false,
}: {
  title: string;
  description: string;
  path: string;
  absolute?: boolean;
}): Metadata {
  const fullTitle = absolute ? title : `${title} — ${site.name}`;
  return {
    title: absolute ? { absolute: title } : title,
    description,
    alternates: { canonical: path },
    openGraph: {
      type: "website",
      url: path,
      title: fullTitle,
      description,
      siteName: site.name,
    },
    twitter: { card: "summary_large_image", title: fullTitle, description },
  };
}
