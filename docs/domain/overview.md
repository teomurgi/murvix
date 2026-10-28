# Domain overview

[Domain reference](README.md) · [Capability coverage](coverage.md) · [Candidate subdomains](subdomains.md)

## Problem and purpose

**Observed — product claim:** Murvix is a terminal multiplexer and development workspace for macOS and Linux. Its stated goal is continuity of terminal work across desktop windows, a terminal UI, command-line automation, and SSH disconnections. Projects organize that work by folder; worktrees isolate Git checkouts; clients organize views without owning the running terminal processes. See [README.md:13–26](../../README.md#L13-L26).

**Observed — implementation:** the desktop's `PaneContent::Terminal` holds an optional session reference rather than a process, while the client restoration planner matches both project and session identity. A pane with no session can request creation; a pane referring to a missing or foreign-project session is scheduled for closure. This is not a promise to restart a lost program. See [crates/murvix-app-core/src/pane.rs:6–20](../../crates/murvix-app-core/src/pane.rs#L6-L20), `restore::plan` in [crates/murvix-app-core/src/restore.rs:15–48](../../crates/murvix-app-core/src/restore.rs#L15-L48).

**Inferred:** the principal domain is **continuity and coordination of a developer's terminal workspace**, not merely terminal rendering. The rationale is the product emphasis on keeping work running and the distinction between shared sessions and private views. Product-owner validation would be needed to establish this as an agreed domain statement or a competitive differentiator.

## Actors

| Actor | Goal and authority | Evidence/status |
| --- | --- | --- |
| Workspace user | Organize projects, run programs, inspect output, navigate views, and reconnect to work. | **Observed:** user-facing project and terminal operations in [README.md:66–120](../../README.md#L66-L120); implemented client mutations in [crates/murvix-app-core/src/state.rs:289–493](../../crates/murvix-app-core/src/state.rs#L289-L493). |
| CLI caller, including an automation or AI tool | Select a project/session and operate it without a desktop window. This is a caller role, not a separate account or commercial customer model. | **Observed:** CLI scenarios and JSON/remote commands in [README.md:81–98](../../README.md#L81-L98); implementation evidence is indexed in [coverage](coverage.md). |
| Extension author and installed extension | Add workspace views and operations through a declared API and permissions. | **Observed — stated contract:** [README.md:124–126](../../README.md#L124-L126); enforcement and limits in [rules](business-rules.md#br-12-extension-authority). |
| Paired-device client | Access an enabled server using a device credential. A shipped phone application is not implied. | **Observed — documented scope:** [docs/product/server-model.md:88–118](../product/server-model.md#L88-L118); implementation evidence in [context map](context-map.md). |
| Maintainer/distributor | Package, install, sign, publish, and replace compatible executable versions. | **Observed:** repository release/install machinery, accounted for as ancillary capabilities in [coverage](coverage.md), not a second invented commercial domain. |

**Not established:** stakeholder-approved roles, teams, organizational ownership, multi-tenant accounts, or commercial policies. The inspected model is centered on the operating-system user and connected clients.

## System boundary

Murvix includes the server, protocol and client library, CLI/TUI, desktop app, UI-independent state/configuration, native UI adapters, and first-party distribution tooling. The [context map](context-map.md) records authority and exchanges; these components are not automatically separate bounded contexts.

External collaborators include the operating system and PTYs, shells/programs, Git and GitHub tooling, SSH endpoints, installed AI command-line tools, native WebKit/browser services, desktop notification/voice services, and extension/update distribution endpoints. Their source, availability, and live behavior are not proven by this analysis.

## Important distinctions

- A **project identity** is not its directory: the server descriptor has an independent ID; the application preserves server ownership and separate view state. See the [model](domain-model.md).
- A **session** is not a **pane**, and a terminal process surviving a client disconnect is not a process surviving a server restart. See [WF-02](workflows.md#wf-02-share-close-detach-and-restore-terminals).
- **Home**, a **worktree project**, and a normal project have different lifecycle constraints. See [BR-01](business-rules.md#br-01-project-identity-and-home) and [BR-03](business-rules.md#br-03-worktree-creation-and-removal).
- **Agent state detection** is inferred from process/title/screen evidence; it is not an authoritative agent task protocol. See [WF-04](workflows.md#wf-04-observe-and-acknowledge-agent-activity).
- **Remote SSH management** and **paired TLS device access** have different authority. See [BR-10](business-rules.md#br-10-connection-authority).

## Scope exclusions and limitations

All first-party capability families discovered in the workspace and tooling inventory are accounted for in [coverage](coverage.md). Vendored libraries, generated assets/caches, dependency internals, and external products are not primary domain-discovery sources. Examples, fixtures, and tests corroborate contracts but are not additional supported applications.

This reference describes the existing implementation and its documented intent. It is not a target architecture, security audit, runtime certification, or stakeholder-validated DDD model. No application, test, build, generator, or service was executed for this documentation task.
