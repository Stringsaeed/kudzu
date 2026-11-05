import assert from "node:assert/strict";
import { spawnSync, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, existsSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./ignore-preview.mjs", import.meta.url));
const fallback = fileURLToPath(new URL("./preview-fallback.sh", import.meta.url));

test("project-level production guard allows builds without calling Git", (t) => {
  const root = mkdtempSync(join(tmpdir(), "kudzu-production-guard-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const marker = join(root, "git-called");
  writeFileSync(join(root, "git"), '#!/bin/sh\nprintf called > "$KUDZU_GIT_PROBE"\nexit 128\n', {
    mode: 0o755,
  });
  const result = spawnSync("/bin/sh", [fallback], {
    cwd: root,
    env: {
      ...process.env,
      VERCEL_ENV: "production",
      PATH: `${root}:${process.env.PATH}`,
      KUDZU_GIT_PROBE: marker,
    },
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.equal(existsSync(marker), false);
  assert.ok(readFileSync(fallback, "utf8").trim().length <= 256);
});

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "kudzu-preview-filter-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const repo = join(root, "repo");
  mkdirSync(repo);
  const git = (...args) =>
    execFileSync("git", args, {
      cwd: repo,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  git("init", "-b", "main");
  git("config", "user.name", "Preview Filter Test");
  git("config", "user.email", "preview-test@example.invalid");
  git("config", "commit.gpgsign", "false");
  function commit(path, content = "changed\n") {
    mkdirSync(dirname(join(repo, path)), { recursive: true });
    writeFileSync(join(repo, path), content);
    git("add", "--all");
    git("commit", "-m", "fixture change");
    return git("rev-parse", "HEAD");
  }
  const base = commit("README.md", "base\n");
  git("switch", "-c", "feature");
  function run({ cwd = repo, previous = base, environment = "preview" } = {}) {
    const env = { ...process.env, VERCEL_ENV: environment, VERCEL_GIT_PREVIOUS_SHA: previous };
    return spawnSync(process.execPath, [script], { cwd, env, encoding: "utf8" });
  }
  return { root, repo, git, commit, base, run };
}

test("skips release workflows, desktop files, and general documentation", (t) => {
  const f = fixture(t);
  f.commit(".github/workflows/release.yml");
  f.commit("apps/desktop/src-tauri/src/main.rs");
  f.commit("README.md");
  assert.equal(f.run().status, 0);
});

test("builds for website changes even when the final commit only touches CI", (t) => {
  const f = fixture(t);
  f.commit("apps/website/app/page.tsx");
  f.commit(".github/workflows/ci.yml");
  assert.equal(f.run().status, 1);
});

for (const path of [
  "packages/typescript-config/next.json",
  "packages/oxlint-config/next.json",
  "pnpm-lock.yaml",
  "package.json",
  "pnpm-workspace.yaml",
  "turbo.json",
]) {
  test(`builds when ${path} changes`, (t) => {
    const f = fixture(t);
    f.commit(path);
    assert.equal(f.run().status, 1);
  });
}

test("compares with the previous successful preview instead of rebuilding old website changes", (t) => {
  const f = fixture(t);
  const deployed = f.commit("apps/website/app/page.tsx");
  f.commit(".github/workflows/release.yml");
  assert.equal(f.run({ previous: deployed }).status, 0);
});

test("detects removing a website file by moving it outside the website", (t) => {
  const f = fixture(t);
  const deployed = f.commit("apps/website/public/example.txt");
  f.git("mv", "apps/website/public/example.txt", "example.txt");
  f.git("commit", "-m", "move file");
  assert.equal(f.run({ previous: deployed }).status, 1);
});

test("first previews in a shallow clone compare all branch changes with main", (t) => {
  const f = fixture(t);
  f.commit("apps/website/app/page.tsx");
  f.commit(".github/workflows/ci.yml");
  const clone = join(f.root, "clone");
  execFileSync("git", [
    "clone",
    "--quiet",
    "--depth=1",
    "--branch=feature",
    pathToFileURL(f.repo).href,
    clone,
  ]);
  const result = f.run({ cwd: clone, previous: "" });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /Website source or build inputs changed/);
});

test("first preview with only release changes is skipped", (t) => {
  const f = fixture(t);
  f.commit(".github/workflows/release.yml");
  const clone = join(f.root, "clone");
  execFileSync("git", [
    "clone",
    "--quiet",
    "--depth=1",
    "--branch=feature",
    pathToFileURL(f.repo).href,
    clone,
  ]);
  assert.equal(f.run({ cwd: clone, previous: "" }).status, 0);
});

test("production deployments still build", (t) => {
  const f = fixture(t);
  f.commit("apps/desktop/README.md");
  assert.equal(f.run({ environment: "production" }).status, 1);
});

test("missing comparison history builds rather than silently dropping a preview", (t) => {
  const f = fixture(t);
  f.git("remote", "add", "origin", join(f.root, "unavailable-remote"));
  assert.equal(f.run({ previous: "0".repeat(40) }).status, 1);
});

test("supports Vercel checkouts without a configured origin", (t) => {
  const f = fixture(t);
  f.commit(".github/workflows/release.yml");
  const clone = join(f.root, "clone");
  execFileSync("git", [
    "clone",
    "--quiet",
    "--depth=1",
    "--branch=feature",
    pathToFileURL(f.repo).href,
    clone,
  ]);
  execFileSync("git", ["remote", "remove", "origin"], { cwd: clone });
  execFileSync(
    "git",
    [
      "config",
      `url.${pathToFileURL(f.repo).href}.insteadOf`,
      "https://github.com/Stringsaeed/kudzu.git",
    ],
    { cwd: clone },
  );
  assert.equal(f.run({ cwd: clone, previous: "" }).status, 0);
});
