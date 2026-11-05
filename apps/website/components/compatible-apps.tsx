import Image from "next/image";

const apps = [
  { name: "Mail", icon: "mail.png", url: "https://www.apple.com/macos/" },
  { name: "Slack", icon: "slack.png", url: "https://slack.com/" },
  { name: "Notion", icon: "notion.png", url: "https://www.notion.com/" },
  { name: "Visual Studio Code", icon: "vscode.png", url: "https://code.visualstudio.com/" },
];

export function CompatibleApps() {
  return (
    <div className="works-strip">
      <span>Works where you write</span>
      <div>
        {apps.map((app) => (
          <a
            className="app-name"
            key={app.name}
            href={app.url}
            target="_blank"
            rel="noopener noreferrer"
          >
            <Image src={`/assets/apps/${app.icon}`} width={26} height={26} alt="" />
            {app.name}
          </a>
        ))}
        <span className="app-name and-more">and the rest of your Mac.</span>
      </div>
    </div>
  );
}
