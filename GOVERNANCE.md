# Governance

## Maintainers and decisions

Murvix is maintained by [@teomurgi](https://github.com/teomurgi), currently its sole maintainer and repository administrator. Contributors can propose changes through issues and pull requests; maintainers decide project direction, review contributions, merge changes, authorize CI, and publish releases. Explain significant design decisions in the relevant issue or PR so contributors can discuss the tradeoffs before implementation. Follow the [AI-assisted contribution policy](CONTRIBUTING.md#ai-assisted-contributions).

Maintainers resolve disagreements after considering contributor feedback and the project's security, compatibility, and maintenance costs. Conduct concerns follow the [Code of Conduct](CODE_OF_CONDUCT.md); vulnerabilities follow the private reporting process in [SECURITY.md](SECURITY.md).

Existing maintainers appoint or remove maintainers through a reviewed governance PR and a repository access review. Update this document, `.github/CODEOWNERS`, every workflow's actor allowlist, and GitHub permissions together. Remove access promptly when a maintainer leaves. Only maintainers may hold write or higher access: GitHub's native workflow dispatch, cancellation, rerun, and fork-run approval permissions are not controlled by YAML alone. In this personal repository, merge and release-tag authority is restricted to the administrator role. Do not grant administrator access merely to let someone triage issues.

## Contributions and merges

- Anyone can fork the project and submit a PR under [CONTRIBUTING.md](CONTRIBUTING.md).
- All changes to `main`, including a maintainer's own changes, go through a PR. Direct pushes, force pushes, and branch deletion are blocked, including for administrators.
- Only repository administrators may merge. The separate integrity ruleset has **no bypass**, so administrators must still satisfy its PR, check, and resolved-conversation requirements.
- Both `macos` and `omarchy` must pass, sourced from the GitHub Actions app. The PR branch must include the latest `main`; update it and repeat CI when necessary.
- `.github/CODEOWNERS` requests maintainer review for every path. It does not itself restrict merging. No independent approval is mandatory while there is only one maintainer, because GitHub disallows self-approval. When another maintainer joins, revisit the required approval count and code-owner review requirement.

## Authorizing CI

Maintainer pushes to `main` run CI automatically. Opening, reopening, or updating a same-repository PR targeting `main` also runs CI automatically when both the event actor and rerun actor are maintainers. Other branch pushes, labels, and comments do not trigger CI. Fork PRs and automation PRs without a maintainer-triggered run require manual authorization from `main`. Workflows reject actors and rerun actors outside the maintainer allowlist, including on forks.

Review the exact revision, including build scripts, dependencies, tests, vendored code, and workflow changes, before pushing a same-repository PR update or manually authorizing a PR run. Both paths execute contributor-controlled code on disposable GitHub-hosted runners. Never use self-hosted runners or provide release secrets to this path. To authorize a PR manually:

1. Review the PR's current full head SHA. Obtain it with `gh pr view <number> --repo teomurgi/murvix --json headRefOid --jq .headRefOid` and inspect that revision.
2. Open **Actions → ci → Run workflow**, select **main**, and supply the PR number as `pull_request` and that reviewed full SHA as `head_sha`. Alternatively, run:

   ```bash
   gh workflow run ci.yml --repo teomurgi/murvix --ref main \
     -f pull_request=<number> -f head_sha=<reviewed-full-sha>
   ```

3. The trusted workflow verifies that the PR is open and targets `main`, rejects a changed head, and checks out the captured SHA without persisted credentials. Only separate trusted jobs can write the `macos` and `omarchy` commit statuses; those jobs never check out contributor code. Build jobs have read-only repository tokens and no release secrets.
4. Inspect both results before merging. Failure, cancellation, or missing results must not be treated as success. A new PR commit needs fresh checks: same-repository maintainer updates trigger them automatically; fork or automation updates without a maintainer-triggered run need a new review and dispatch. Statuses on an older SHA do not authorize it.

Leaving both inputs empty checks `main` without authorizing a PR. Dispatches from other branches are rejected. To retry a failed check without changing the revision, maintainers can rerun the workflow; both the original actor and rerun actor must be authorized. Shared Rust build caches are deliberately not consumed by CI or release jobs: an approved PR runs in the default-branch workflow context and could otherwise poison caches subsequently restored by trusted releases.

The live **all external contributors** approval policy remains enabled for fork workflows. Review workflow changes before approving any fork run. The existing required check names are retained; do not disable branch protection to merge a workflow update.

## Releases

Releases use **manual Release dispatch → automated version PR → maintainer-authorized CI and merge → tag, build, and publish**. Do not manually bump versions or push a tag to start a release; tag pushes do not trigger publishing. All release changes pass through the same `main` integrity requirements as other PRs.

### Credentials

Configure the repository Actions secret `RELEASE_TOKEN` before the first release. It must be a fine-grained personal access token belonging to an allowlisted maintainer with repository administrator authority, restricted to this repository, with **Contents: read and write** and **Pull requests: read and write**. Automation uses it only to push the release branch, create the PR, and create the release tag. Never expose it to PR CI. Keep it valid and rotate it when its owner or repository authority changes.

The maintainer-owned token is required by the existing policy: Actions' standard `GITHUB_TOKEN` cannot create PRs here, and only administrators can create, move, or delete `v*` tags. It also means the generated PR is authored by the token's maintainer account, not `github-actions[bot]`, and its creation can trigger normal maintainer-authorized CI. A PR created or updated with `GITHUB_TOKEN` would not trigger PR CI automatically. Do not enable a main-branch bypass, loosen tag restrictions, or change Actions' PR-approval policy to make releases work.

### Release runbook

Local release tooling requires Python 3.11 or newer and Git; it has no third-party Python dependencies.

1. Merge the intended changes into `main`, using [Conventional Commits](CONTRIBUTING.md#commit-messages-and-releases) to express their release impact. The planner ignores prereleases and unrelated branch tags, chooses the highest reachable stable version, and takes the largest requested SemVer bump. Without stable tags it uses `VERSION` at the source commit and its full history. A docs/chore-only history needs no release.
2. Open **Actions → Release → Run workflow**, select **main**, and dispatch as an allowlisted maintainer. Automation captures the source SHA and runs `python3 scripts/release.py plan --source <source-sha>`. With no release changes it stops without opening a PR. Otherwise it stamps `VERSION`, the workspace version in `Cargo.toml`, and workspace package versions in `Cargo.lock`, then opens `release/vX.Y.Z`. Inherited crate manifests and third-party dependency resolutions remain unchanged.
3. Review the automated PR and its exact head SHA. Both `macos` and `omarchy` must pass on that head. The configured maintainer-owned token normally triggers those checks automatically. If they are missing, use [Authorizing CI](#authorizing-ci): dispatch **ci** from **main** with `pull_request` set to the PR number and `head_sha` set to the reviewed full head SHA. Both inputs are required for authorizing a PR; leaving them empty checks only `main`.
4. The release PR must still be based on the latest `main`. If `main` has advanced, close the stale release PR, delete its release branch, and dispatch **Release** again from current `main`; review and authorize CI for the replacement head. Do not merge `main` into the version branch or add source changes to it. Resolve review conversations and merge through the protected PR path only.
5. The maintainer's merge triggers verification: `python3 scripts/release.py verify --source <pre-version-source> --release <merged-sha>` recomputes the planned version and requires the release tree to equal that source plus exactly the three stamped files. Any other source change or incorrect version stops publishing. The workflow creates `vX.Y.Z` at the exact merged SHA, builds that same revision, and publishes its artifacts and update metadata.
   Publication stays in draft until every expected asset has been uploaded and its size verified; only then is the release made public and marked latest.
6. If a build or publication fails after the merge, rerun the original merged-PR workflow. The tag is created before the builds; starting a new manual **Release** run would plan a different release rather than retry the failed one. Do not move or delete an existing release tag to retry publishing.

The release and reusable build workflows separately verify both the original actor and rerun actor against the maintainer allowlist. A closed, unmerged PR is not a release authorization. Never publish unreviewed contributor code or expose signing credentials to PR CI.

## GitHub administration

Policy files alone do not enforce repository settings. The following rulesets are installed on GitHub and their request bodies are tracked under `.github/rulesets/`:

| Ruleset | Policy file | Authority |
| --- | --- | --- |
| main integrity | `main-integrity.json` | PR required, resolved review threads, current-base `macos` and `omarchy` checks from Actions app 15368, no force push/deletion; no bypass |
| main maintainer merges | `main-maintainers.json` | Restrict updates; administrator role (ID 5) can bypass **through a PR only** |
| release tags maintained by admins | `release-maintainers.json` | Restrict creation, update, and deletion of `v*` tags; administrator-only bypass |

The two `main` rulesets must remain separate: combining them would let the merge-authority bypass skip the integrity requirements.

Audit installed rules with:

```bash
gh api repos/teomurgi/murvix/rulesets
gh api repos/teomurgi/murvix/rules/branches/main
gh api repos/teomurgi/murvix/collaborators --jq '.[] | {login, role_name}'
```

To update a ruleset, get its ID from the first command and apply its reviewed policy file:

```bash
gh api --method PUT repos/teomurgi/murvix/rulesets/<id> \
  --input .github/rulesets/<policy-file>.json
```

For a new repository, use `POST repos/<owner>/<repo>/rulesets` instead, after updating owner references and workflow allowlists. Do not repeatedly POST on the existing repository; that creates duplicate rulesets.

Keep these additional settings enabled:

- **Actions → General:** approval required for **all external contributors**; default workflow token permissions **read repository contents**; Actions **cannot create or approve pull requests**. Never change approval to first-time contributors only.
- **Collaborators:** no non-maintainer write/admin access. Review access when membership changes. Administrators can change the rules themselves; governance assumes they are trusted.
- **Security:** private vulnerability reporting, secret scanning, and secret-scanning push protection enabled.

Reapply the settings with:

```bash
gh api --method PUT repos/teomurgi/murvix/actions/permissions/fork-pr-contributor-approval \
  -f approval_policy=all_external_contributors
gh api --method PUT repos/teomurgi/murvix/actions/permissions/workflow \
  -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
gh api --method PUT repos/teomurgi/murvix/private-vulnerability-reporting
```

Fork owners control Actions in their own forks. These restrictions govern the upstream repository, not other people's independent repositories.
