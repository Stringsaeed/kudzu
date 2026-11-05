import Image from "next/image";

const apps = [
  { name: "Kudzu", icon: "/assets/kudzu-icon.png" },
  { name: "Mail", icon: "/assets/apps/mail.png" },
  { name: "Slack", icon: "/assets/apps/slack-macos.png" },
  { name: "Craft", icon: "/assets/apps/craft-macos.png" },
  { name: "Visual Studio Code", icon: "/assets/apps/vscode-macos.png" },
];

export function MacOSDock() {
  return (
    <div className="macos-dock" aria-hidden="true">
      {apps.map((app) => (
        <span key={app.name} className="dock-app" data-running={app.name === "Kudzu" || undefined}>
          <span className="dock-tooltip" aria-hidden="true">
            {app.name}
          </span>
          <Image src={app.icon} width={52} height={52} alt="" />
        </span>
      ))}
    </div>
  );
}
