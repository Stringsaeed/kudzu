import Image from "next/image";
import { Icon } from "./icons";

export function SiteFooter() {
  return (
    <footer className="footer-section">
      <div className="site-footer wrap">
        <a className="brand" href="#main" aria-label="Back to top">
          <Image src="/assets/kudzu-icon.png" width="32" height="32" alt="" />
          <span>kudzu</span>
        </a>
        <span>For things worth repeating.</span>
        <div>
          <a className="source-link" href="https://github.com/Stringsaeed/kudzu">
            <Icon name="github" />
            Source code
          </a>
          <a
            className="github-star footer-star"
            href="https://github.com/Stringsaeed/kudzu"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Icon name="star" />
            Star on GitHub
          </a>
          <a href="https://github.com/Stringsaeed/kudzu/issues">Say hello</a>
          <span>
            Made by <a href="https://thisissaeed.com">Saeed</a>
          </span>
        </div>
      </div>
      <div className="footer-wordmark wrap" aria-hidden="true" />
    </footer>
  );
}
