# Open questions, contradictions and evidence limits

[Domain reference](README.md) · [Coverage](coverage.md) · [Rules](business-rules.md)

These are current-state findings and validation questions, not a refactoring backlog. **Observed** contradictions retain both sources; **Inferred** classifications remain candidates; **Not established** identifies the missing evidence and how it could be obtained. None is silently resolved by treating a test name or documentation promise as executed behavior.

## Q-01 Strategic priorities and bounded contexts are unvalidated

**Inferred:** terminal-work continuity is the core candidate because the product headline/introduction emphasize it. Workspace organization, agent awareness, Git, remote access and augmentation support that goal. **Not established:** expert-agreed domain language, strategic ranking, distinct team/model ownership or DDD relationship agreements.

**Impact:** the [subdomains](subdomains.md) and [context map](context-map.md) are analytical aids, not an assertion of organizational design. **Resolve with:** product owner/maintainers validating responsibilities, overloaded vocabulary and authority boundaries. Code size or package count would not resolve this. Evidence: [README.md:3–26](../../README.md#L3-L26), [Cargo.toml:1–30](../../Cargo.toml#L1-L30).

## Q-02 Catalog durability and receipt lifetime

**Observed:** catalog rename can commit/publish before a subsequent parent-sync error; archives, catalog status, Git side effects and process termination are separate effects. Natural-exit writes retry; retention can remove archive before catalog cleanup commits. Creation/project/close receipts and cancellation IDs remain in catalog state; the inspected ended-session retention path does not prune them.

**Not established:** bounded receipt growth or a universally atomic process/archive/catalog lifecycle. **Impact:** callers must not interpret every persistence error as no change, nor “project aggregate” as an all-or-nothing filesystem/process transaction. **Resolve with:** maintainers clarifying intended receipt lifetime and durability contract; broader storage/failure evidence for any stronger guarantee. This task did not inject failures or execute recovery tests. [crates/murvix-server/src/catalog/mod.rs:31–78,260–295](../../crates/murvix-server/src/catalog/mod.rs#L260-L295), [crates/murvix-server/src/catalog/recovery.rs:13–53](../../crates/murvix-server/src/catalog/recovery.rs#L13-L53), [crates/murvix-server/src/registry.rs:755–799](../../crates/murvix-server/src/registry.rs#L755-L799).

## Q-03 Generated pull-request metadata review is promised but not present in the traced flow

**Observed contradiction:** settings text says the user reviews generated title/summary/branch/target before commit/push/PR creation. Current `advance_ai_action` runs provider generation → `parse_draft` → `apply` directly after the earlier action/options confirmation.

**Impact:** documentation must not promise a second review/approval of generated metadata. Preparation can also stage content before generation; failures are not necessarily side-effect-free. **Resolve with:** maintainers deciding whether the current UI wording or the traced implemented flow represents intended behavior. This reference records the implementation, not a proposed fix. [crates/murvix-app/src/views/settings/catalog.rs:129–136](../../crates/murvix-app/src/views/settings/catalog.rs#L129-L136), [crates/murvix-app/src/model/ai.rs:493–514](../../crates/murvix-app/src/model/ai.rs#L493-L514), [crates/murvix-app/src/repository_actions.rs:216–300](../../crates/murvix-app/src/repository_actions.rs#L216-L300).

## Q-04 Extension API prose and surface contracts differ

**Observed contradictions/qualifications:**

- API table groups `projects.add` and `projects.create` as returning a project ID; `create_extension_project` returns an object containing `id`, `name`, `path`.
- The opening API example says `tabs.list` works in a script or background; the public background bridge exposes only `tabs.open` for tabs. Host dispatch permission mapping is not a complete copy of JS convenience-API availability.
- Package identifiers allow uppercase/underscore, but native page-owner URL generation permits only lowercase letters, digits, hyphen and dot. Contributor guidance already narrows names for extensions with pages.

**Impact:** a valid package load does not guarantee its page can open; callers cannot assume uniform return shapes or availability across script/page/background surfaces. **Resolve with:** extension API maintainers validating the intended compatibility contract and historical callers. No external extension packages were executed. [docs/extensions/api.md:8–10,39–48](../extensions/api.md#L39-L48), [crates/murvix-app/src/model/extensions/calls.rs:399–425](../../crates/murvix-app/src/model/extensions/calls.rs#L399-L425), [crates/murvix-app/src/extensions/bridge.js:96–108](../../crates/murvix-app/src/extensions/bridge.js#L96-L108), [crates/murvix-app-core/src/extensions/manifest.rs:481–488](../../crates/murvix-app-core/src/extensions/manifest.rs#L481-L488), [crates/murvix-ui/src/webview/assets.rs:17–41](../../crates/murvix-ui/src/webview/assets.rs#L17-L41), [docs/extensions/contributing.md:15–19](../extensions/contributing.md#L15-L19).

## Q-05 Mobile wording does not supply a phone application

**Observed:** CLI pairing text invites scanning with the Murvix app; product/mobile docs explicitly say no phone app ships. TLS/device authentication is implemented. Manifest `homeViews`, `remoteMethods` and `remote:serve`, and stored background `remote.handle` functions do not establish active mobile/home-view dispatch.

**Impact:** end-to-end phone workflows require an external compatible client; parsed compatibility declarations are not supported first-party applications. **Resolve with:** maintainers identifying any separately released compatible client/contract if one exists. No external client was requested or available as first-party source here. [crates/murvix-cli/src/mobile.rs:134–137](../../crates/murvix-cli/src/mobile.rs#L134-L137), [docs/features/mobile.md:1–17](../features/mobile.md#L1-L17), [docs/extensions/manifest.md:67–69](../extensions/manifest.md#L67-L69), [crates/murvix-app/src/extensions/bridge.js:327–331](../../crates/murvix-app/src/extensions/bridge.js#L327-L331).

## Q-06 Release guidance and packaging disagree

**Observed contradiction:** repository agent guidance describes manual version stamping/tag push, while README/current release workflow describe a reviewed release PR and exact merged-commit verification. Static Arch packaging contains beta metadata/sentinel comments even though workspace version is stable and repository guidance forbids prerelease releases; its `pkgver()` reads `VERSION`, so the literal metadata does not alone establish the effective package version.

**Impact:** no single release procedure or universal installation/update guarantee should be inferred from all prose/packaging artifacts. System package installation is distinct from a user-managed self-updating installation. **Resolve with:** maintainers reconciling authoritative release policy and packaging intent; this task neither releases nor edits source/guidance. [AGENTS.md:5–11](../../AGENTS.md#L5-L11), [README.md:145–150](../../README.md#L145-L150), [.github/workflows/release.yml:21–78](../../.github/workflows/release.yml#L21-L78), [Cargo.toml:33–40](../../Cargo.toml#L33-L40), [packaging/linux/PKGBUILD:3–4,27–41,56–64](../../packaging/linux/PKGBUILD#L27-L41).

## Q-07 Resource-only language packs: enforcement is narrower than wording

**Observed:** docs say bundles must not contain executable code; loader checks absence of `CFBundleExecutable`, bounds/canonicalizes catalog files and verifies placeholders. No inspected recursive scan establishes rejection of every unused executable file inside a bundle. The application reads catalogs rather than running a declared bundle executable.

**Impact:** distinguish non-execution of localization data from a universal package-content prohibition/security guarantee. **Resolve with:** localization maintainers clarifying intended threat/validation contract; native loader/runtime safety is outside static proof. [docs/extensions/localizations.md:45–53](../extensions/localizations.md#L45-L53), [crates/murvix-app-core/src/localization.rs:94–152](../../crates/murvix-app-core/src/localization.rs#L94-L152).

## Q-08 AI generation for remote projects is not established end to end

**Observed:** repository operations use the selected project's server client, while external AI generation receives a desktop-local `project.directory` and launches a local tool. Agent transcript discovery/resume similarly targets available local project scope; agent recognition has a broader provider set than supported transcript resume.

**Not established:** remote AI metadata generation succeeding without a matching desktop directory or another execution path. **Impact:** remote Git support must not be promoted into every remote AI/history capability. **Resolve with:** maintainer validation of intended remote use cases and separately authorized runtime exercise of the actual remote workflow. [crates/murvix-app/src/model/ai.rs:474–514](../../crates/murvix-app/src/model/ai.rs#L474-L514), [crates/murvix-app/src/ai.rs:248–290](../../crates/murvix-app/src/ai.rs#L248-L290), [crates/murvix-app/src/model/agent_sessions.rs:77–222](../../crates/murvix-app/src/model/agent_sessions.rs#L77-L222).

## Q-09 External services and native behavior were not exercised

**Not established:** live release/update assets, marketplace packages/publishing, GitHub authentication/environment approvals, successful SSH/remote installation, provider tool behavior, native WebKit/JavaScriptCore guarantees, OS notification display, clipboard/audio permissions, Linux capture/transcription tools/models or platform signing/notarization. The first-party adapters are in scope; external implementations and live configuration are not.

**Impact:** implemented integration must be distinguished from available service and successful end-to-end operation. **Resolve with:** separately authorized environment/runtime validation and external source/contract evidence as needed. No application, tests, build/generator, installer or service was run; no live systems were accessed. Boundary evidence: [context map](context-map.md), [crates/murvix-core/src/capability.rs:63–94](../../crates/murvix-core/src/capability.rs#L63-L94), [crates/murvix-ui/src/voice/linux.rs:17–174](../../crates/murvix-ui/src/voice/linux.rs#L17-L174), [crates/murvix-app/src/updater.rs:28–180](../../crates/murvix-app/src/updater.rs#L28-L180).

## Q-10 Similar operation names hide different guarantees

**Observed:** ordinary file Delete uses Trash, Git discard permanently removes untracked files, direct commits bypass reviewed-snapshot guards, and legacy branch deletion forces deletion. Session close preserves other connected references while explicit end/discard does not; only first attachment's size can resize an otherwise unattached session, whereas explicit resize is shared. Saved history omits live prompt metadata.

**Impact:** broad language such as “safe delete,” “reviewed commit,” “attaching resizes,” or “restored shell integration” is too strong without operation/path qualification. **Resolve with:** domain/UX validation of intended terminology and named-operation policy; current enforcement and bypasses are canonical in BR-05/06/09/13. [crates/murvix-server/src/git/mutate.rs:7–46](../../crates/murvix-server/src/git/mutate.rs#L7-L46), [crates/murvix-server/src/git/mod.rs:332–415](../../crates/murvix-server/src/git/mod.rs#L332-L415), [crates/murvix-server/src/files/trash.rs:1–65](../../crates/murvix-server/src/files/trash.rs#L1-L65), [crates/murvix-server/src/session/owner.rs:477–539](../../crates/murvix-server/src/session/owner.rs#L477-L539), [crates/murvix-server/src/archive.rs:195–249](../../crates/murvix-server/src/archive.rs#L195-L249).

## Q-11 Configuration continuity is narrower than full workspace recovery

**Observed:** backup sanitizes terminal references and excludes composer drafts/images, browser profile data, project files, credentials and normal extension packages. Startup restore uses compensating journaled file operations; exports do not snapshot every concurrent profile writer atomically. Native restore and legacy merge have different matching/layout precedence.

**Not established:** stakeholder expectation that “Backup & Restore” means full workspace/data recovery, or any stronger cross-file durability promise. **Impact:** user-facing explanations must say which data returns and that programs do not restart. **Resolve with:** maintainer/product validation of expected backup scope; actual exclusions and restore failure handling are recorded in [BR-15](business-rules.md#br-15-configuration-restore-and-exclusions). [crates/murvix-app-core/src/backup/projects.rs:15–95](../../crates/murvix-app-core/src/backup/projects.rs#L15-L95), [crates/murvix-app/src/backup/mod.rs:19–55](../../crates/murvix-app/src/backup/mod.rs#L19-L55), [crates/murvix-app/src/backup/transaction.rs:24–236](../../crates/murvix-app/src/backup/transaction.rs#L24-L236).

## Completion interpretation

These questions do not represent missing first-party capability analysis. Coverage is reconciled in [coverage.md](coverage.md); the unanswered items are explicit intent/classification questions, observed contract disagreements or external/runtime limitations. Static evidence cannot settle them, and this reference does not claim that it does.
