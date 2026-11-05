import Link from "next/link";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { DownloadAction } from "@/components/download-action";
import { checked, comparisons } from "@/lib/compare";

type Props = { params: Promise<{ slug: string }> };

export const dynamicParams = false;

export function generateStaticParams() {
  return comparisons.map(({ slug }) => ({ slug }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { slug } = await params;
  const item = comparisons.find((comparison) => comparison.slug === slug);
  if (!item) return {};
  return {
    title: `Kudzu vs ${item.name}`,
    description: item.summary,
    alternates: { canonical: `/compare/${item.slug}` },
  };
}

export default async function ComparisonPage({ params }: Props) {
  const { slug } = await params;
  const item = comparisons.find((comparison) => comparison.slug === slug);
  if (!item) notFound();
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
                <td>{row.kudzu}</td>
                <td>{row.other}</td>
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
      </section>

      <div className="page-cta">
        <DownloadAction />
      </div>

      <p className="page-note">
        {item.name} details were checked on {checked} and can change, so please confirm them with{" "}
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
