# Live workspace visibility for running agents

Status: implementation plan; no runtime changes in this branch.

## Goal and acceptance contract

A folder added to a Murvix project's workspace after an agent starts must appear in that agent's next workspace listing, without restarting the terminal or agent. The agent must be able to enumerate and use every current root through an explicit workspace namespace, subject to native permission approval. Removal must stop advertising the root and invalidate future workspace-qualified operations against it.

This guarantee applies to integrated agents. An arbitrary already-running external process cannot be forced to refresh its prompt, tools, sandbox, or environment by changing Murvix's catalog. Unsupported agents must be identified explicitly, not presented as live-integrated. Full delivery includes a functioning Oh My Pi integration, not only a CLI command or skill instructions.

## Evidence and corrected diagnosis

In the reported session, `read .` listed only the physical Murvix repository. Running `murvix project here --json` later in the same terminal returned:

```json
{"workspace":["/home/teomurgi/work/murvix","/home/teomurgi/work/mac-gliff"]}
```

The original response confused physical cwd children with workspace roots. There is no evidence here that directory reads are cached at agent startup. `mac-gliff` is an additional workspace root, not a physical child of the Murvix checkout.

Existing implementation anchors:

- `crates/murvix-protocol/src/project.rs`: `ProjectDescriptor.extra_directories` and `workspace_roots` already define the authoritative root set.
- `crates/murvix-cli/src/agent.rs`: launch resolves the current project once, passes native `--add-dir` options where supported, then replaces itself with the agent process using `exec`.
- `crates/murvix-server/src/spawn.rs`: `MUXY_WORKSPACE_*` variables are terminal-start snapshots. Server/session identity survives independently of that snapshot.
- `crates/murvix-cli/tests/management.rs`: existing scenarios exercise live membership with deliberately obsolete environment roots and reject wrong-server identity.
- `crates/murvix-server/src/catalog/mutations.rs`: extra roots are validated when the project is mutated.
- `crates/murvix-cli/skills/murvix-cli/SKILL.md`: already tells agents to use `project here`, rather than cwd or environment snapshots.
- `docs/features/ai-agents.md`: explicitly documents that running agents are not reconfigured when roots change.
- `crates/murvix-server/shell/murvix.{bash,zsh,fish}`: wrappers resolve roots at launch; they cannot update a running agent.

The existing live lookup is the starting point. Do not build a second workspace registry or fix an unproven filesystem-cache issue.

## Design decision

Expose a logical workspace namespace backed by the live catalog, and integrate it into agent tool dispatch and turn context. Do not introduce FUSE, bind mounts, a symlink forest inside the repository, or a copied/indexed filesystem. Those mechanisms do not update model context or native sandbox permissions and add platform, lifecycle, and path-security costs.

Keep three concepts separate:

1. Membership: the server catalog decides which roots belong to this terminal's project now.
2. Discovery: the agent receives a current root list and understands that workspace listing is not `read .`.
3. Access: the native agent's existing permissions decide whether an operation is allowed.

Do not reinterpret bare `.` or relative paths globally. They keep their normal cwd meaning. Introduce an explicit logical namespace such as `workspace://` for all roots and `workspace://<root-key>/<relative-path>` for a path within a selected root. The exact URI registration API must be verified against OMP before implementation; the spelling here is a proposed contract, not an existing tool API.

## Implementation sequence

### 1. Establish the agent integration boundary

Inspect the installed/supported OMP extension or tool-provider APIs before changing Murvix. Identify hooks for before-turn context, tool dispatch, subagent creation, permission checks, and root changes. Confirm how resumed sessions are initialized. Pin supported capabilities rather than guessing API names.

Choose a maintained OMP extension shipped through the existing Murvix agent setup path if those hooks are sufficient. If hooks cannot update native tools and permissions safely, implement the missing provider support in OMP as a required companion change. This repository alone cannot promise native OMP root refresh without that integration.

Produce a capability matrix for OMP, Claude, Codex, Copilot, Pi, and OpenCode: live context, live tool namespace, native permission refresh, and launch-only behavior. Only mark an adapter live once its real running-process scenario passes. Preserve existing launch-only adapters with truthful UI/help labels; do not claim universal support.

### 2. Reuse and formalize live workspace resolution

Use the same server/session-bound resolution as `murvix project here --json`. Preserve stale-session and foreign-server rejection. Do not fall back to environment roots, cwd, or another local server on lookup failure.

Expose a structured snapshot suitable for the adapter: server/project identity, ordered roots, lossless path bytes, and a membership revision. First inspect existing catalog revision/event support and reuse it if suitable; otherwise add a narrowly scoped membership revision. Do not bump it for unrelated project edits.

Give roots stable opaque keys tied to server/project identity and exact path bytes, not array indices or basenames. Derive keys deterministically with an existing repository primitive, or add a catalog identifier only if necessary. Check collisions. Reordering roots must not retarget an old URI; removal must make that URI fail. The same unchanged path may recover its identity when re-added. Preserve the primary root's distinguished role.

Keep `project here` output backward-compatible unless a deliberate migration of every consumer is included. Human display names may be lossy; machine identity and routing must not be. Reuse existing byte-array path representation.

### 3. Resolve namespace operations against current membership

Implement one resolver shared by adapter operations. Each workspace-qualified request resolves against a current authoritative snapshot. `workspace://` lists roots, including names, keys, real paths when representable, and availability. It must never silently return only the primary root.

Route existing read/list/search/write/edit capabilities through the resolver rather than duplicating their semantics. Reuse native permission checks for the resolved real path. Path aliases are routing conveniences, not authorization. Reject parent traversal and handle symlinks according to the existing tool security model; resolve permissions on the actual target rather than trusting an alias prefix.

A removed root must fail future namespace resolution instead of using a cached path. An operation already authorized and started may finish against its captured root; document this boundary. Removing workspace membership is not OS-level revocation of an open file descriptor, absolute-path access, or permissions previously granted to a native agent.

For cross-root search, name each result's root explicitly and retain existing ignore/hidden-file controls. Apply bounded result limits with explicit truncation and continuation; do not conflate an omitted result with an absent root. Limit root metadata, not membership visibility. Do not recursively scan trees merely to refresh the root list.

Keep Git and execution tied to an explicitly selected real root. Do not silently change the existing primary-project semantics of `murvix files`, `murvix git`, or `murvix exec` as part of this feature.

### 4. Refresh OMP automatically

At session attachment and before each model turn, query live membership and inject a compact authoritative workspace descriptor. When membership changes, replace the prior descriptor and state additions/removals. Do not accumulate stale snapshots in reusable context.

Before each workspace-qualified tool operation, refresh or validate the descriptor against the server. Turn-level refresh alone is insufficient when the user adds a folder during a long tool sequence. Begin with on-demand lookup; add an existing catalog event subscription only as an optimization. Events must not be the sole correctness mechanism, and disconnect/reconnect must fetch a full snapshot.

For added roots, use the native dynamic directory/permission mechanism if available. Otherwise request native approval through the supported tool interface. An advertised but unapproved root must be clearly marked pending approval, not omitted or silently authorized. If neither mechanism exists, the missing upstream capability is a release blocker for that adapter's full live-access claim.

Subagents inherit a workspace resolver binding, not a frozen root array. They refresh on their own turns and requests. Resumed sessions rebind to the live terminal's verified identity; reject stale saved identities. Cancellation and teardown release subscriptions and adapter resources without restarting the shell.

### 5. Integrate launch, setup, and user-facing behavior

Wire the adapter into the current `murvix agent omp` and desktop launch path. Trace the shared agent setup/skill installer before choosing an extension installation location. Do not edit user configuration or install dependencies without the established consent flow. Keep plain external agent invocation behavior explicit; already-running sessions without the adapter require attachment through a supported hook or one restart to enable it.

Update `docs/features/ai-agents.md`, `docs/features/murvix-cli.md`, `docs/features/server.md`, the bundled CLI skill, and relevant help text. Explain workspace roots versus cwd children, supported adapters, permission prompts, removal semantics, and offline failures. Replace obsolete launch-only claims only for adapters that actually support live updates.

The desktop workspace UI should distinguish advertised membership from native agent access when that state is available. Do not imply that adding a folder bypasses a sandbox. No runtime version bump is needed for this plan-only branch. Implemented functionality warrants a minor release using `scripts/release.py stamp` and the repository's stable SemVer policy.

## Failure and performance requirements

- Server unavailable: return an explicit workspace-unavailable error; no stale success or fallback to a different project.
- Root deleted or unreadable on disk: retain its membership entry with an unavailable/error status; fail operations with the real reason.
- Duplicate basenames, spaces, newlines, and non-UTF-8 Unix paths: retain unambiguous routing and lossless representation. If an agent API cannot represent a path, report that limitation explicitly.
- Remote project: resolve and operate on the project's server. Never turn remote paths into local filesystem accesses. Keep current remote-launch limitations until a tested adapter supports that route.
- Membership changes during pagination: bind continuation to the snapshot revision or reject/restart it; never mix incompatible root sets silently.
- No per-agent recursive watchers, busy polling, full-tree indexing, or subprocess per file. Fetch root metadata once per turn/request boundary; prefer a reusable existing client connection if the adapter API permits it.
- No new third-party dependency by default. Any unavoidable dependency needs user approval with its GitHub URL, stars, and maintenance evidence.

## Verification and acceptance scenarios

Use isolated temporary projects and a real server. Extend existing management integration tests for membership semantics, rather than tests that merely assert copied argv or prompt wording.

1. Start OMP through the integrated launcher with only root A. Add an external `mac-gliff` root B through the desktop while OMP remains running. Ask for workspace folders; the next answer lists A and B. Read a known file in B through the namespace, grant native approval if required, and verify its actual contents.
2. Repeat addition between two tool calls in the same turn. The second call must discover B without waiting for another user prompt.
3. Remove B. The next listing excludes it, and its prior workspace URI fails. A previously running operation follows the documented captured-root boundary. Absolute-path/native-permission behavior is not falsely described as revoked.
4. Create an ordinary child directory inside A after startup. A normal native directory read sees it; workspace membership remains unchanged. This separates filesystem freshness from root membership.
5. Start a subagent before adding B; after addition it sees B on its next workspace operation. Repeat with session resume and with server reconnect.
6. Exercise root reorder, identical basenames, special path bytes, symlink escape, native denial, unavailable roots, stale identities, foreign-server identity, and removal during pagination. Assert errors or real file contents, not just non-empty output.
7. Verify a denied root stays visible as pending/denied but cannot be read through an alias. No fallback tool may bypass approval.
8. Verify unsupported agent launches remain usable and explicitly launch-only. Do not simulate an unsupported adapter and count it as live coverage.

Capture a real OMP interaction transcript and desktop add/remove observation. Unit tests alone do not establish agent discovery. Record supported OMP version and the permissions used. Use throwaway markers and remove them after the smoke run.

Run repository checks after implementation:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
```

## Delivery boundaries

Deliver the resolver contract and real OMP adapter together, including required upstream hooks if any. A new command, a refreshed skill, native launch flags, or a directory of symlinks alone does not meet acceptance. No implementation should assert that it can force an unintegrated running agent to see folders.

This planning change adds only this document. The live CLI observation proves current membership discovery; it does not prove that native agent tools already integrate that discovery.
