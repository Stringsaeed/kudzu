import Image from "next/image";

const apps = [
  { name: "Kudzu", icon: "/assets/kudzu-icon.png", url: "#download" },
  { name: "Mail", icon: "/assets/apps/mail.png", url: "https://www.apple.com/macos/" },
  { name: "Slack", icon: "/assets/apps/slack-macos.png", url: "https://slack.com/" },
  { name: "Notion", icon: "/assets/apps/notion-macos.png", url: "https://www.notion.com/" },
  {
    name: "Visual Studio Code",
    icon: "/assets/apps/vscode-macos.png",
    url: "https://code.visualstudio.com/",
  },
];

export function MacOSDock() {
  return (
    <nav className="macos-dock" aria-label="macOS Dock: apps that work with Kudzu">
      {apps.map((app) => (
        <a
          key={app.name}
          className="dock-app"
          href={app.url}
          aria-label={app.name}
          data-running={app.name === "Kudzu" || undefined}
          target={app.url.startsWith("https:") ? "_blank" : undefined}
          rel={app.url.startsWith("https:") ? "noopener noreferrer" : undefined}
        >
          <span className="dock-tooltip" aria-hidden="true">
            {app.name}
          </span>
          <Image src={app.icon} width={52} height={52} alt="" />
        </a>
      ))}
    </nav>
  );
}
