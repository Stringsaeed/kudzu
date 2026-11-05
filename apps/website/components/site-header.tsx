import Image from "next/image";
import { ThemeToggle } from "./theme-toggle";
import { Icon } from "./icons";

export function SiteHeader() {
  return (
    <header className="site-header wrap">
      <a className="brand" href="#main" aria-label="Kudzu home">
        <Image src="/assets/kudzu-icon.png" width="40" height="40" alt="" />
        <span>kudzu</span>
      </a>
      <nav aria-label="Main navigation">
        <a href="#features">The little things</a>
        <a href="#playground">Try it out</a>
        <a
          className="github-star"
          href="https://github.com/Stringsaeed/kudzu"
          target="_blank"
          rel="noopener noreferrer"
        >
          <Icon name="github" /> Star on GitHub <Icon name="star" />
        </a>
      </nav>
      <div className="header-actions">
        <ThemeToggle />
        <a className="button button-small button-dark" href="#download">
          Get Kudzu
        </a>
      </div>
    </header>
  );
}
