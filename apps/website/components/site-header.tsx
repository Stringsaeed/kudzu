import Link from "next/link";
import Image from "next/image";
import { site } from "@/lib/site";
import { ThemeToggle } from "./theme-toggle";
import { Icon } from "./icons";
import { GitHubLogo } from "./github-logo";
import { MobileNavigation } from "./mobile-navigation";
import { NavLinks } from "./nav-links";

export function SiteHeader() {
  return (
    <header className="site-header wrap">
      <Link className="brand" href="/" aria-label="Kudzu home">
        <Image src="/assets/kudzu-icon.png" width="40" height="40" alt="" loading="eager" />
        <span>kudzu</span>
      </Link>
      <nav className="desktop-navigation" aria-label="Main navigation">
        <NavLinks />
        <a className="github-star" href={site.github} target="_blank" rel="noopener noreferrer">
          <GitHubLogo /> Star on GitHub <Icon name="star" />
        </a>
      </nav>
      <div className="header-actions">
        <ThemeToggle />
        <Link className="button button-small button-primary" href="/#download">
          Get Kudzu
        </Link>
        <MobileNavigation />
      </div>
    </header>
  );
}
