import Link from "next/link";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { DownloadAction } from "@/components/download-action";
import { Icon } from "@/components/icons";
import { comparisons } from "@/lib/compare";
import { pageMetadata } from "@/lib/metadata";
import { worstCase, worstComparison } from "@/lib/worst-case";

type Props = { params: Promise<{ slug: string }> };

export const dynamicParams = false;

export function generateStaticParams() {
  return comparisons.map(({ slug }) => ({ slug }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { slug } = await params;
  const item = comparisons.find((comparison) => comparison.slug === slug);
  if (!item) return {};
  return pageMetadata({
    title: item.seoTitle,
    description: item.summary,
    path: `/compare/${item.slug}`,
    absolute: true,
  });
}

export default async function ComparisonPage({ params }: Props) {
  const { slug } = await params;
  const found = comparisons.find((comparison) => comparison.slug === slug);
  if (!found) notFound();
  const item = (await worstCase()) ? worstComparison(found) : found;
  return (
    <main id="main" className="page wrap">
      <header className="page-heading">
        <Link className="eyebrow" href="/compare">
          COMPARE
        </Link>
        <h1>Kudzu vs {item.name}</h1>
        <p>{item.summary}</p>
      </header>

      <div className="table-scroll">
        <table className="compare-table">
          <thead>
            <tr>
              <th scope="col">
                <span className="visually-hidden">Feature</span>
              </th>
              <th scope="col">Kudzu</th>
              <th scope="col">{item.name}</th>
            </tr>
          </thead>
          <tbody>
            {item.rows.map((row) => (
              <tr key={row.label}>
                <th scope="row">{row.label}</th>
                <td data-label="Kudzu">{row.kudzu}</td>
                <td data-label={item.name}>{row.other}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="choose-grid">
        <section>
          <h2>Choose Kudzu if</h2>
          <ul>
            {item.chooseKudzu.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </section>
        <section>
          <h2>Choose {item.name} if</h2>
          <ul>
            {item.chooseOther.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </section>
      </div>

      <section className="prose">
        <h2>{item.moving.title}</h2>
        <ol>
          {item.moving.steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
        <p>
          New to Kudzu? <Link href="/docs#first-shortcut">Make your first shortcut</Link>, or see
          what the <Link href="/docs/templates">template syntax</Link> can do.
        </p>
      </section>

      <div className="page-cta">
        <DownloadAction />
      </div>

      <nav className="other-comparisons" aria-label="Other comparisons">
        {comparisons
          .filter((other) => other.slug !== item.slug)
          .map((other) => (
            <Link key={other.slug} className="inline-link" href={`/compare/${other.slug}`}>
              Kudzu vs {other.name}
              <Icon name="arrow" />
            </Link>
          ))}
      </nav>

      <p className="page-note">
        {item.name} details were checked on {item.checked} and can change, so please confirm them with{" "}
        {item.sources.map((source, index) => (
          <span key={source.href}>
            {index > 0 && " and "}
            <a href={source.href} target="_blank" rel="noopener noreferrer">
              {source.label}
            </a>
          </span>
        ))}
        . {item.name} is a trademark of its owner. Kudzu isn’t affiliated with or endorsed by it.
      </p>
    </main>
  );
}
