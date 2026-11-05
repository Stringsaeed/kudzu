import { spawnSync } from "node:child_process";

// Vercel's ignored-build contract: 0 skips the build, 1 proceeds.
function decide() {
  if (process.env.VERCEL_ENV !== "preview") return [1, "Non-preview deployment: build."];

  const git = (args, cwd) =>
    spawnSync("git", args, {
      cwd,
      encoding: "utf8",
      timeout: 20_000,
      stdio: ["ignore", "pipe", "pipe"],
    });
  const location = git(["rev-parse", "--show-toplevel"]);
  if (location.status !== 0) return [1, "Git history unavailable: build."];
  const root = location.stdout.trim();
  const previous = process.env.VERCEL_GIT_PREVIOUS_SHA;
  let base;

  if (
    previous &&
    /^[a-f0-9]{40,64}$/i.test(previous) &&
    git(["cat-file", "-e", `${previous}^{commit}`], root).status === 0
  ) {
    base = previous;
  } else {
    // First preview: compare the whole branch, not just its latest commit.
    const remote =
      git(["remote", "get-url", "origin"], root).status === 0
        ? "origin"
        : "https://github.com/Stringsaeed/kudzu.git";
    if (git(["rev-parse", "--is-shallow-repository"], root).stdout.trim() === "true") {
      const head = git(["rev-parse", "HEAD"], root).stdout.trim();
      git(["fetch", "--quiet", "--no-tags", "--deepen=100", remote, head], root);
    }
    git(
      ["fetch", "--quiet", "--no-tags", "--depth=100", remote, "main:refs/remotes/origin/main"],
      root,
    );
    const ancestor = git(["merge-base", "HEAD", "refs/remotes/origin/main"], root);
    if (ancestor.status !== 0) return [1, "No reliable comparison commit: build."];
    base = ancestor.stdout.trim();
  }

  const result = git(
    [
      "diff",
      "--quiet",
      "--no-renames",
      base,
      "HEAD",
      "--",
      "apps/website/",
      "packages/oxlint-config/",
      "packages/typescript-config/",
      "package.json",
      "pnpm-lock.yaml",
      "pnpm-workspace.yaml",
      "turbo.json",
      ".npmrc",
      ".node-version",
      ".nvmrc",
    ],
    root,
  );
  if (result.status === 0) return [0, "No website inputs changed: skip preview."];
  if (result.status === 1) return [1, "Website source or build inputs changed: build preview."];
  return [1, "Could not compare website inputs: build."];
}

const [status, message] = decide();
console.log(message);
process.exitCode = status;
