# S01 — workspace evidence

Date: 2026-11-11. Host: arm64, macOS 27.2 (26B123), Rust 1.99.1.
Source: 2352d319969bc753b2de1ca12ce76b7f8c3035c6.
Rune and rune-kit: git revision 29aacb688883a9712bc27175bdafd99fc6edb421.
No branch change, push, PR, release, tag, label or upstream issue write was performed.

## Acceptance

| Item | Result | Evidence |
|---|---|---|
| Same copied tests | PASS | `cargo test --workspace`: 377 core + 73 engine, zero failures; one launch-mode test and eight dependency-gate tests also pass. Original crates' `cargo +1.99.1 test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -p kudzu-core -p kudzu-engine -- --list` with CARGO_TARGET_DIR outside the repository lists the same 377 + 73 tests. Test names compared as well. |
| Frozen paths | PASS | `git diff --exit-code --` all rule-1 paths is empty. The step adds only the new workspace and desktop-v2.yml; no existing migration document is modified. Copied engine is byte-identical; copied core differs only in the skill include path and its path documentation. |
| Dependency gate | PASS | `cargo xtask check-deps` passes against the locked, resolved host graph. `cargo test -p xtask`: eight tests; real temporary Cargo manifests reject an app-to-engine edge, a ../desktop path dependency, external #[path]/include macros and build.rs; additional cases check transitive tokio, raw/computed include paths, and an unused external workspace dependency. |
| --tree in CI | FAIL / not run | GitHub Actions execution is prohibited in this task. Locally the identical workflow commands pass except fmt (below). --tree and --snap both work under Metal locally. Only a real xcode-27 run can confirm its GPU/window-server availability, sandbox-exec policy, toolchain provisioning, cache behavior and permissions. |
| Four headless invocations | PASS locally | `cargo build && python3 scripts/check-headless.py`: --tree, --snap, --list-windows and --list-scenes all exit 0. RUNE_DIRS_ROOT, HOME, TMPDIR and cache roots are temporary; sandbox-exec denies filesystem writes outside that directory. JSON includes Kudzu Next; PNG has the PNG signature; windows lists main. S01 contains no native adapter constructors; the binary branches to fixtures before window composition and imports no system adapter. |
| Root install | PASS | `pnpm install --frozen-lockfile`: exit 0, pnpm 12.16.1; root manifests/lockfile unchanged. |

## Required gate and local workflow validation

- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS (459 tests total: 450 copied + 9 new).
- `cargo xtask check-deps`: PASS.
- `cargo fmt --check`: FAIL in copied code, beginning with core/src/activity.rs:77.
  The original files do not match Rust 1.99.1's default formatting. Running fmt
  changes dozens of copied files. Those formatting changes were discarded because
  S01 explicitly allows only path edits in copied Rust. The check is retained
  unchanged in CI: this commit is **not gate-green**.
- `actionlint -config-file /tmp/s01-actionlint.yml .github/workflows/desktop-v2.yml`:
  PASS with `self-hosted-runner: { labels: [xcode-27] }` in that temporary config.
  Without the config, actionlint rejects only this custom runner label.
- `git diff --cached --check`: reports only four pre-existing whitespace lines
  copied verbatim from the OFL licence and the rich TextExpander fixture; these
  were preserved as source material. Hand-written files have no whitespace errors.
- `cargo build` and the workflow's Python smoke command: PASS locally.
- Clean build: PASS: `/usr/bin/time -p env CARGO_TARGET_DIR=/tmp/kudzu-s01-clean-target cargo build` finished in 163.79 s wall (975.16 s user, 42.96 s sys), with an empty target directory and warm dependency download caches; dev profile, dependency opt-level 2. Not a cold network/cache or CI timing.
- Native launch: built binary opened a temporary smoke .app outside the repository.
  Computer Use observed standard window “Kudzu Next” and text ID workspace-title.
  The close button terminated that smoke app. This is window smoke evidence, not
  S03 identity/bundling/signing validation.

## Release isolation dry run

An **offline** rehearsal used release-please 17.11.2 from npm,
invoked only with `node /tmp/s01-release-dry-run.cjs --dry-run`.
It used the real CommitSplit, conventional-commit parser and Simple strategy,
the current release configuration and a synthetic local S01 commit. The GitHub
object throws on every access except repository identity, so no network write
(or read) is possible. Its updates were applied only to a temporary copy.

```text
PASS: actual release-please CommitSplit excludes S01 from apps/desktop
✔ Considering: 1 commits
❯ component:
❯ pull request title pattern: undefined
❯ componentNoSpace: undefined
✔ No user facing commits found since previous - skipping
PASS: hidden build type independently generates no release
✔ Considering: 1 commits
❯ component:
❯ pull request title pattern: undefined
❯ componentNoSpace: undefined
PASS: simple strategy updates only version.txt and CHANGELOG.md in temporary copy /tmp/s01-release-layout-NxwqFa
```

This proves the path matcher, hidden type and simple strategy's file updates.
It is not a live manifest-pr dry run over GitHub history: the S01 commit is local
and pushing is prohibited. A live dry run cannot include it without publication.
The full manifest runner may also update its release manifest; the assertion here
is specifically about the Simple strategy's package updates. No live release PR
or GitHub state claim is made.

Reproduction harness (requires release-please from npm at the recorded version;
set `base` to that installation's build/src directory):

```javascript
if (!process.argv.includes('--dry-run')) throw Error('--dry-run required');
const assert = require('node:assert/strict');
const base = '/Users/saeed/.npm/_npx/dd04923b7fe85367/node_modules/release-please/build/src/';
const {CommitSplit} = require(base+'util/commit-split.js');
const {Simple} = require(base+'strategies/simple.js');
const {parseConventionalCommits} = require(base+'commit.js');
const {Version} = require(base+'version.js');
const {TagName} = require(base+'util/tag-name.js');
const fs = require('node:fs');
const config = JSON.parse(fs.readFileSync('release-please-config.json'));
const commit = {sha:'s01-local',message:"build(desktop-v2): start the Rune app as its own workspace",notes:[],files:['apps/desktop-v2/Cargo.toml','.github/workflows/desktop-v2.yml']};
const split = new CommitSplit({packagePaths:Object.keys(config.packages)}).split([commit]);
assert.deepEqual(split, {});
console.log('PASS: actual release-please CommitSplit excludes S01 from apps/desktop');
const github = new Proxy({repository:{owner:'Stringsaeed',repo:'kudzu'}}, {get(t,k) {
 if (k in t) return t[k];
 throw Error('GitHub access forbidden in offline dry run: '+String(k));
}});
const strategy = new Simple({github,targetBranch:'main',path:'apps/desktop-v2',
packageName:'kudzu',includeComponentInTag:false,changelogSections:config['changelog-sections']});
(async()=>{
 const hidden = await strategy.buildReleasePullRequest(parseConventionalCommits([commit]),{sha:'previous',tag:new TagName(Version.parse('0.7.0'))});
 assert.equal(hidden, undefined);
 console.log('PASS: hidden build type independently generates no release');
 const pr = await strategy.buildReleasePullRequest(parseConventionalCommits([{...commit,message:'feat(desktop): rebuild Kudzu as a native Rust app on Rune'}]), {sha:'previous',tag:new TagName(Version.parse('0.7.0'))});
 assert.deepEqual(pr.updates.map(u=>u.path).sort(),['apps/desktop-v2/CHANGELOG.md','apps/desktop-v2/version.txt']);
 const tmp=fs.mkdtempSync('/tmp/s01-release-layout-');
 for (const update of pr.updates) {
  const input=update.path.endsWith('version.txt')?fs.readFileSync('apps/desktop-v2/version.txt','utf8'):'';
  const output=update.updater.updateContent(input);
  fs.writeFileSync(tmp+'/'+update.path.split('/').pop(),output);
 }
 console.log('PASS: simple strategy updates only version.txt and CHANGELOG.md in temporary copy '+tmp);
})().catch(e=>{console.error(e);process.exitCode=1;});

```

## Deviations, gaps and decisions

- Copy preservation conflicts with the fmt gate. Owner/orchestrator must authorize
  formatting the copied crates (as a follow-up), or resolve the plan's conflicting
  requirements. No formatter exclusions or test relaxations were added.
- An empty spikes/workspace-smoke crate keeps the mandatory spikes/* members glob
  valid before S10–S15 add spike binaries. It is outside default-members.
- The copied design-export scripts remain historical tooling, not build inputs.
  They still refer to their former src-tauri export layout and pnpm/Tauri tooling;
  native export tooling must replace these before using them in the new app.
- MCP mode exits with a clear “implemented in S32” message; no bridge implementation
  is claimed by S01.
- Rune gaps encountered by S01: none. Proposed upstream issues: none.
- Owner decisions: the formatting conflict above. CI execution remains with the
  orchestrator; runner-specific --tree/--snap support is unverified.
