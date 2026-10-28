# Murvix domain reference

A source-grounded explanation of **terminal-work continuity and workspace coordination**, organized with Domain-Driven Design terminology without claiming Murvix follows DDD or that domain experts validated this model.

## Read in this order

| Page | Purpose |
| --- | --- |
| [Overview](overview.md) | Problem, actors, system boundary and important distinctions. |
| [Glossary](glossary.md) | Extracted candidate language, context-specific meanings and overloaded terms. |
| [Subdomains](subdomains.md) | Capability-based decomposition and qualified strategic classifications. |
| [Context map](context-map.md) | Candidate model boundaries, data authority, translation and integration direction. |
| [Domain model](domain-model.md) | Identity/lifecycles, value-like concepts, consistency-root candidates and unsupported DDD classifications. |
| [Business rules](business-rules.md) | Stable BR-01–BR-18 conditions, enforcement, bypasses and limits. |
| [Workflows and events](workflows.md) | WF-01–WF-11 static scenario traces, failure branches, transitions and actual messages. |
| [Open questions](open-questions.md) | Contradictory artifacts, unvalidated intent, external prerequisites and static-analysis limits. |
| [Capability coverage](coverage.md) | First-party capability inventory, command/API families, persistence/jobs, tooling and exclusions. |

## Analyzed source and snapshot

- **Source:** this repository, all nine first-party workspace crates plus first-party shell integration, schemas, installation/packaging/release/verification tooling and relevant tests/documentation. Workspace inventory: [Cargo.toml:1–30](../../Cargo.toml#L1-L30); existing guide index: [docs/README.md:1–13](../README.md#L1-L13).
- **Revision:** `509355b5fc0b78af369a389e82fead57ba4b9843` (`Merge pull request #22 from teomurgi/fix/release-build-failures`). Read-only Git metadata showed a clean working tree before analysis. This reference describes that inspected working tree; the only task-authored changes are Markdown files in this folder. It is not a claim about subsequent revisions.
- **Product baseline:** workspace package version `0.3.1` in [Cargo.toml:33–40](../../Cargo.toml#L33-L40). Conflicting historical packaging/release guidance is retained as [Q-06](open-questions.md#q-06-release-guidance-and-packaging-disagree), not silently treated as current policy.
- **Destination:** `docs/domain`, a new folder. Existing indexes had product, feature, extension, layout and technical guides but no established domain-reference destination. Existing documentation was not overwritten or restructured.

## Evidence legend

- **Observed:** directly supported by the cited implementation, schema or named artifact. A prose claim proves documented intent; an unexecuted test describes an expectation. Neither proves a passing runtime result.
- **Inferred:** interpretation of inspected evidence, with rationale and validation limits. Subdomains, bounded-context candidates, strategic labels and DDD building-block classifications are marked accordingly.
- **Not established:** evidence is unavailable, missing or contradictory, or static analysis cannot justify a stronger guarantee. Absence from a search is not treated as proof of nonexistence.

Source citations retain repository-relative paths in visible labels and use filesystem-relative links from each page. `#Lx-Ly` records inspected source line spans for readers/viewers that support them; other local viewers may open only the file. A multi-span label names additional inspected spans in that same file. Paths and line numbers describe this snapshot and may drift after source changes.

## Coverage and completion

**Complete — static capability-level domain reference for the inspected first-party scope.** Every discovered first-party capability family is documented or classified as ancillary infrastructure in [coverage](coverage.md). No requested first-party external component is missing. Vendored/generated/dependency internals and live external products/services are explicitly outside primary discovery; first-party integration adapters remain included.

Key modeling result: server-owned projects/sessions and client-owned views have different authority and lifecycles. The catalog, local view documents and composer store provide narrower consistency boundaries; a globally transactional Project aggregate, organizational bounded contexts and durable domain-event stream are **Not established**. Read the pages for evidence and exceptions rather than interpreting those terms as architecture commitments.

## Verification and limits

Verification is **static only**: capability-to-document reconciliation; cross-checks of consequential rules/alternate mutation paths/failure branches; relative Markdown link, heading-fragment and source-citation bounds checks; and prose review for unsupported guarantees, credentials and private records. Source links are local, not invented remote URLs.

No application/project code was executed or imported. No test, Cargo formatter/linter/build/doc command, generator, installer, dependency installation, service or live-system request was run. Generic read-only documentation inspection is not runtime verification. Native UI/services, GitHub/marketplace/release availability, external AI tools and stakeholder intent remain unverified as detailed in [open questions](open-questions.md).

The task did not modify source/configuration/version files, commit, tag or publish. Existing repository guides remain authoritative inputs with disagreements explicitly recorded, not silently corrected outside this folder.
