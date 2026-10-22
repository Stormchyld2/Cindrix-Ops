# CindrixOps — Architecture Evolution & Proposed Features

**Document status:** PROPOSAL / DISCUSSION ONLY — NOT AUTHORIZATION TO IMPLEMENT  
**Version:** 0.2  
**Prepared:** 2026-10-22  
**Audience:** Project owner, Claude, ChatGPT, future maintainers  
**Repository location:** `docs/CINDRIXOPS_ARCHITECTURE_EVOLUTION_PROPOSED_FEATURES.md`

> **Change-control rule:** This document records ideas developed during design discussions. It does not override existing architecture documents, approved requirements, implemented code, or the project's current roadmap. Do not implement, refactor, migrate data, rename modules, or alter UI based solely on this document. First reconcile each proposal with the repository and obtain explicit owner approval.

## 1. Purpose and scope

CindrixOps is envisioned as a cohesive **offline preparedness, reference, and emergency operations platform**, with **Vault** as the desktop-oriented edition and **Hub** as the Android-oriented edition. The goal is to help users prepare, respond, and recover, including in extended disruptions without internet access.

This document preserves brainstorming from the October 2026 UI/UX and architecture discussions, particularly proposals **not yet incorporated into the repository architecture documents**. It intentionally distinguishes **observed UI**, **reported project intent**, **proposals**, and **unverified implementation state**.

### 1.1 Evidence and uncertainty

- Screenshots reviewed in discussion show navigation or pages labeled Dashboard, Ask, Library (including Shelf and Emergency Cards), Intel / ZIM browsing, Academy, Maps, Music, Plan and its subareas, Inventory, and multiple Settings pages.
- Discussions describe local Qwen models, ZIM archives, portable storage, emergency planning, seed management, and Vault/Hub editions.
- A screenshot or discussion **does not verify** that a feature is implemented, tested, secure, or fully operational.
- Existing code, test results, architecture constraints, and current roadmap **must be verified in the repository** before planning changes.
- The project owner indicated the architecture documents are maintained, but these brainstorming ideas have not yet been incorporated.

## 2. Product principles proposed for approval

1. **Offline first:** Critical plans, reference access, inventory, emergency procedures, and records must work without internet.
2. **AI optional:** Every critical task has a deterministic, non-AI pathway. AI can retrieve, summarize, and explain but must not be the sole source of safety-critical decisions.
3. **Enter once, reuse appropriately:** Shared entities and stable IDs prevent duplicate household, equipment, and location data.
4. **Independent modules, useful integration:** A module should remain functional when another module or service is unavailable.
5. **Traceability:** Operational estimates show inputs, assumptions, freshness, and source. Reference answers cite exact local sources.
6. **User authority:** Starting incidents, altering plans, recording medical information, changing inventories, and other consequential actions require explicit authorization.
7. **Data portability and recoverability:** Documented, versioned formats; integrity validation; atomic writes; backups and tested restores.
8. **Privacy and least privilege:** Encrypt sensitive personal records, minimize cross-module disclosure, and distinguish ordinary reference content from private household records.
9. **Accessible emergency operation:** Keyboard support, readable contrast, predictable navigation, printable fallbacks, and simplified crisis workflows.
10. **No invented status:** Distinguish zero, unknown, not configured, stale, estimated, and verified.

**Constraint to verify:** A local-file-first, SQL-free architecture was discussed as a preference. Confirm CindrixOps' actual documented storage rules before adopting this as a binding constraint. The proposed design below is compatible with ordinary files and disposable indexes; it does not require SQL.

## 3. Proposed CindrixOps Operations Core (COC)

### 3.1 Role

A **shared coordination layer**, not another disconnected UI module and not a monolithic data file. It connects existing feature modules through stable records, explicit contracts, events, deterministic rules, contextual relationships, and common retrieval services.

```text
                       CINDRIXOPS VAULT / HUB
              Dashboard · Ask · Library · Intel · Academy
              Maps · Plan · Inventory · Music · Settings
                                  |
                      OPERATIONS CORE (proposed)
          Shared entity registry · Event/change notifications
          Context links · Tasks · Incidents · Rules/forecasting
          Retrieval/search · Access control · Sync interfaces
                                  |
              VERSIONED OFFLINE FILES AND RESOURCE PACKS
         Household records · Plans · Inventory · Documents
         ZIM/reference material · Maps · AI models · Backups
```

Music and other independent modules need not depend on operational data; integrate only where meaningful.

### 3.2 Proposed core services

| Service | Responsibility | Important constraint |
|---|---|---|
| Entity registry | Stable IDs for people, locations, vehicles, equipment, consumables, plans, incidents | One authoritative owner for each record type |
| Relationship/context service | Links inventory to plans, training, maps, references, and incidents | Links may be user-authored, imported, or suggested; record provenance |
| Event service | Notifies consumers about validated changes | Durable change handling; no silent loss or circular updates |
| Deterministic rules | Readiness, consumption, maintenance, dependencies, due dates | Explainable formulas, configurable assumptions, no AI arithmetic as authority |
| Operations service | Incidents, checklists, task assignments, logs, status reports | Explicit lifecycle and user confirmation |
| Unified retrieval | Federated search across authorized local sources | Source-specific adapters, freshness, citations, permission checks |
| Sync interface | Vault–Hub data exchange | Offline independence, conflicts, versioning, authentication |
| Data resilience | Atomic writes, integrity, backup/restore, migrations | Recovery testing before high-value migrations |

### 3.3 Canonical record candidates

`Person`, `Household`, `Location`, `Vehicle`, `InventoryItem`, `InventoryLot`, `Equipment`, `Consumable`, `Plan`, `Checklist`, `Task`, `Drill`, `Skill`, `Incident`, `Observation`, `DecisionLog`, `Reference`, `SourceCitation`, `Document`, `Device`, `SyncChange`.

**Design note:** These are candidate domain concepts, not a mandated schema. Inspect existing entities and IDs first; extend rather than duplicate them. Each record should have a stable ID, schema version, provenance, timestamps where appropriate, and a defined owning module. Use structured references instead of copying sensitive records into other modules.

### 3.4 Events and consistency

Candidate events: `inventory.quantity_changed`, `equipment.status_changed`, `plan.updated`, `household.membership_changed`, `incident.activated`, `task.completed`, `resource.index_updated`.

Consumers should derive projections (e.g., water days remaining) from authoritative records. Avoid event-driven cascades that write back into the same source indefinitely. Define idempotency, replay/rebuild, and partial-failure behavior. For file-first storage, consider append-only change logs only after confirming repository conventions; generated indexes and projections should be rebuildable.

## 4. Unified Knowledge Retrieval (UKR) — highest-leverage ASK proposal

### 4.1 Why

ASK should answer not only **“What does the library say?”** but, with permission, **“What applies to this household, its supplies, location, plans, and active incident?”** The same retrieval service should power Dashboard search, Plan context links, Inventory reference lookups, and Academy.

### 4.2 Retrieval pipeline

```text
Question / search request
  -> intent and authorization check
  -> source routing (operational records vs. references)
  -> hybrid retrieval (metadata + exact text + optional semantic search)
  -> relevance ranking and deduplication
  -> structured calculations via deterministic rules engine
  -> grounded response via optional local model
  -> citations + confidence/limitations + navigable actions
```

**Sources:** Library/ZIM content; Emergency Cards; user plans and drills; Inventory; household records subject to permissions; Maps/locations; Academy; agricultural references; incident logs and other operational records. Add source adapters rather than building separate search implementations per screen.

### 4.3 Specific ASK improvements

- Hybrid lexical and semantic retrieval, with fallback to lexical search if embeddings or model are unavailable.
- Context selection from selected region, household, equipment, active incident, and explicit question scope.
- Exact document/page/section citations, including archive identity and version where feasible.
- Clear separation between **source fact**, **user-entered fact**, **calculated estimate**, and **AI synthesis**.
- Permission-aware access to medical and personal records; do not automatically disclose them just because ASK can search them.
- Direct navigation: “Open water inventory,” “View outage plan,” “Read reference,” “Start drill,” with confirmation for modifications.
- Query transparency: optionally show searched collections, omitted/unavailable collections, and last-indexed status.
- Search results still usable if the local model fails or is not installed.
- Retrieval must not treat untrusted documents as instructions to execute actions.

### 4.4 Contextual relationships / local knowledge graph

Example: `generator -> stored fuel -> power-outage plan -> maintenance task -> safety reference -> drill`. Another: `pressure canner -> manufacturer manual -> approved preservation guidance -> altitude setting -> training lesson`.

Store explicit relationships with source and confidence. Automated link suggestions require review for safety-critical associations. A graph **does not require a graph database**; portable records and indexes are possible.

## 5. Emergency Operations Center (EOC)

### 5.1 Proposed operating states

- **Preparedness:** planning, inventory, training, maintenance, reference management.
- **Response:** an explicitly activated incident with accountability, hazards, prioritized checklists, tasks, communications, resource forecasts, and logs.
- **Recovery:** damage assessment, expenses, repairs, replenishment, insurance/documentation, and after-action review.

These are application workflow states, **not automatic emergency declarations**. The same data and navigation remain available across modes.

### 5.2 EOC capabilities

- Incident templates and user-created incidents (e.g., winter storm, power outage, evacuation, water disruption).
- Household accountability and check-in records; distinguish unconfirmed from missing.
- Time-stamped incident journal, observations, decisions, and source of information.
- Tasks with priority, assignment, completion evidence, and reassessment reminders.
- Resource dashboard: water, food, fuel, batteries, medications, equipment condition, with assumptions and ranges.
- Printable situation reports, checklists, contact sheets, and maps.
- Shift/duty rosters and scheduled check-ins for prolonged incidents.
- Recovery tracking and post-incident lessons feeding back into preparedness plans.
- A prominent, predictable emergency-access control; critical information accessible even when AI, maps, or external archives fail.

**Safety:** Never silently trigger evacuation, transmit emergency messages, alter medication instructions, or claim live weather/road status from offline data.

## 6. Proposed cross-module feature expansions

### 6.1 Household Readiness Engine

Compute scenario-specific preparedness gaps from household size, needs, inventory, equipment status, and selected targets. Display underlying assumptions rather than a misleading universal readiness score. Differentiate “not entered” from “confirmed none.”

### 6.2 Inventory and What Fails Next

Inventory by storage location, owner/assignment, lot, expiration, condition, and intended use. Separate home stores, vehicle kits, personal packs, and evacuation loads. Add minimum stock levels, rotation, usage history, and dependencies. Forecast resource exhaustion using user-confirmed consumption assumptions, not opaque AI predictions.

### 6.3 Evacuation and Relocation

Primary/alternate destinations; vehicle and on-foot configurations; people-to-vehicle assignments; route/map bookmarks; fuel/range assumptions; staged loads; rendezvous points; offline destination information. Mark map age and avoid implying real-time road conditions.

### 6.4 Communications Center

Radio channel plans, frequency/license metadata where relevant, contact trees, call schedules, message templates, communications logs, and device profiles. Hardware integrations optional and later-phase; do not promise transmission without compatible hardware.

### 6.5 Equipment Maintenance

Service intervals, tests, batteries, fuel, manuals, parts, inspection logs, and operational status for generators, radios, water treatment, vehicles, and other critical assets. Propagate equipment unavailability into readiness and incident forecasts.

### 6.6 Medical Preparedness

Encrypted household medical profiles, allergies, medications, clinician-provided emergency instructions, first-aid inventory, and care logs. Strict access control, deliberate export/print controls, and explicit provenance. AI should retrieve authoritative guidance, not invent dosing or diagnoses.

### 6.7 Home and Shelter Resilience

Utility shutoff locations, shelter plans, heating/cooling contingencies, sanitation, water access, fire safety, ventilation, inspections, and repair priorities. Include carbon-monoxide and other hazard safeguards.

### 6.8 Academy and Drills

Connect lessons to relevant equipment, plans, skills, practical drills, and recertification dates. Separate “lesson read” from “skill demonstrated.” Record after-action observations and plan improvements.

### 6.9 Seed Vault + National Agricultural Resource (CNAR)

Maintain a clear separation:

- **Seed Vault:** user's seed lots, viability, garden plans, sowing, harvests, storage, and crop records.
- **CNAR / agricultural reference:** curated offline US-wide agricultural knowledge, including cooperative extension materials, planting windows, soils, crops, pests, preservation, and regional guidance.

Proposed location configuration: state/county/region selected during setup; use it to **prioritize** relevant guidance while retaining access to all US content offline. Source metadata should include jurisdiction, climate/zone applicability, publication/update date, attribution, license, and safety review where applicable. Do not equate a statewide planting calendar with a guarantee for a specific microclimate.

Content strategy: inventory public-domain/open-license and permissioned sources; use existing ZIM archives when appropriate, Zimit only when legally and technically suitable, and normalized curated resource packs for structured lookup. Respect copyright, update provenance, and archive integrity. Size and acquisition plan require a separate evidence-based assessment.

### 6.10 Additional long-duration resilience capabilities

Document vault (encrypted), financial continuity/expense records, voluntary mutual-aid contacts and resource requests, local weather observation logs, supply substitution references, field printing, maintenance schedules, and post-incident review.

## 7. User interface evolution — preserve identity

Screenshots suggest an established dark charcoal, orange/blue accent, technical-grid identity. **Do not redesign the app wholesale.** Proposed refinements:

- Dashboard emphasizes actual readiness and actionable issues, not just navigation shortcuts.
- Reusable contextual side panel showing related plans, inventory, references, training, and tasks.
- Universal search distinguishes **operational record**, **reference**, **map**, **plan**, and **training** results.
- Settings grouped into scalable categories rather than an expanding multi-row tab strip.
- Responsive desktop widths, more readable secondary text, keyboard access, and adequate contrast.
- Status indicators use words/icons as well as color.
- Music player may collapse to preserve vertical space.
- Emergency mode simplifies navigation without hiding critical information.
- Offline/online indicator should reflect connectivity, not imply offline features are broken.

All UI changes require owner review and should follow existing component/design conventions.

## 8. Vault and Hub integration

- **Vault:** desktop management, large archives, extensive maps, local models, deep planning and editing.
- **Hub:** independent Android field access to selected records, checklists, maps, contacts, and incident updates.
- Shared versioned data contracts; explicit authenticated pairing; scoped sync; conflict reporting and resolution; schema migration handling.
- Hub remains useful if Vault is off, unreachable, or destroyed.
- Prefer selective transfer of necessary records and content rather than assuming mobile storage can hold the entire Vault library.
- Consider local network/device-to-device exchange as a later phase after durable local storage and conflicts are proven.

## 9. Data security, resilience, and failure modes

**Priority before sensitive-data expansion:**

1. Encrypt private household/medical/document records at rest using reviewed primitives and key management; a UI passphrase alone is not proof of encryption.
2. Atomic writes and versioned schema migrations; never mutate the only copy without recoverability.
3. Versioned backups, integrity checks, restore tests, and clear lost-key policy.
4. Verify portable-drive removal/reinsertion, low disk space, read-only media, partial writes, corrupted indexes, and damaged resource archives.
5. Self-test screen for source availability, indexes, map packs, AI models, storage health, and backup status.
6. Separate data security from content licensing and archive authenticity; validate downloaded/imported packs.
7. Protect against malicious document content and prompt injection in local reference sources.
8. Graceful degradation: critical plans and emergency cards available without AI, internet, or optional content packs.

## 10. Proposed implementation roadmap (not approved)

| Phase | Focus | Suggested completion evidence |
|---|---|---|
| 0 — Discovery | Audit repo, architecture, tests, and current capabilities | Feature matrix: implemented / partial / planned / proposed; baseline test results |
| 1 — Contracts & resilience | Shared IDs/schemas, ownership, safe writes, backup/restore, access controls | Migration and recovery tests; no data loss |
| 2 — Operational integration | Household + Inventory + Plan + What Fails Next | One edit propagates accurately; unknown data handled explicitly |
| 3 — Unified retrieval | Federated indexes, citations, permission-aware ASK, non-AI search | Grounded retrieval tests; model-off fallback |
| 4 — Emergency operations | Incident lifecycle, tasks, journal, situation reports, response UI | Offline scenario walkthroughs and persistence tests |
| 5 — Advanced modules | Evacuation, comms, medical, maintenance, shelter, Academy links | Module-specific functional and safety tests |
| 6 — Content & mobility | CNAR curation, Vault–Hub interoperability, optional integrations | Provenance audits, sync conflict tests, mobile offline tests |

Phases may be reordered after repository inspection. Content curation can proceed in parallel once metadata/pack contracts are stable.

## 11. Scenario-based acceptance tests

For each scenario, test both **normal operation** and **degraded operation** (internet off, model unavailable, optional archive missing):

1. **72-hour power outage:** load plan, check household, forecast water/fuel, retrieve safety guidance, log decisions.
2. **30-day utility outage:** rationing alternatives, maintenance, sanitation, communications schedules, and reassessment.
3. **Evacuation:** assign people and gear to vehicles, access offline maps, print checklist, switch to foot fallback.
4. **Loss of primary computer/drive:** restore verified backup or use independent Hub with essential records.
5. **Multi-month disruption:** seed/growing guidance, maintenance, replenishment, training, and long-term resource forecasts.
6. **Sensitive-data protection:** unauthorized search cannot expose household medical records; authorized user can retrieve them offline.
7. **Conflicting device edits:** Vault and Hub both change the same item; conflict is visible and resolvable without silent overwrite.

Success requires explicit expected outcomes, reproducible tests, and documented limitations—not merely that a screen renders.

## 12. Decisions needed from project owner

**All items below remain open until explicitly approved.**

- Approve or revise the name and scope of **Operations Core**.
- Confirm current storage/index rules and whether SQL/SQLite is expressly prohibited for CindrixOps.
- Choose incident workflow and minimum EOC feature set.
- Confirm which household records ASK may access by default versus only on explicit request.
- Decide whether Context Panel and universal search should be global UI features.
- Approve readiness/forecasting assumptions and how uncertainty is displayed.
- Choose encryption/key-recovery and backup policies before medical/document expansion.
- Confirm CNAR scope, licensing policy, regional selection, and resource-pack format.
- Confirm Vault–Hub synchronization transport and conflict policies.
- Agree on a staged milestone and acceptance criteria before implementation.

## 13. Claude / ChatGPT developer handoff protocol

When resuming work after another developer or AI assistant:

1. **Read** repository README, architecture index, current milestone docs, this proposal, and relevant module docs.
2. **Inspect** default/current branches, recent commits, open PRs/issues, and CI results. Check for local uncommitted changes if local access exists.
3. **Establish facts:** what code exists, what passes tests, what remains incomplete, what is only proposed. Never infer implementation from UI screenshots.
4. **Compare** this proposal against approved architecture; record conflicts and ask owner before altering a binding decision.
5. **Do not overwrite** another developer's unfinished work or rewrite existing components without a scoped reason and approval.
6. **Select one approved milestone** and write its acceptance criteria before modifying code.
7. **Implement incrementally** with regression tests, migrations/recovery tests where relevant, and an explicit offline test.
8. **Document** files changed, architectural decisions, test commands/results, limitations, and the exact commit/branch checkpoint.
9. **Update** architecture and progress docs only to reflect verified changes; keep unapproved brainstorming marked PROPOSED.
10. **Hand off** a concise status report so another developer can resume without guessing.

### Suggested checkpoint template

```text
Date/time:
Repository and branch:
HEAD commit:
Working tree / open PR state:
Approved milestone:
Completed since prior checkpoint:
Tests executed and results:
Unfinished work / known failures:
Files and schemas modified:
Migration/backup implications:
Architecture docs updated:
Next safe action:
Questions requiring owner decision:
```

## 14. GitHub Actions verification and build continuity

GitHub Actions is the project's existing automated test/build mechanism, as reported by the owner. Preserve and reuse the actual repository workflows rather than inventing replacement CI. Before making changes, inspect `.github/workflows/`, required checks, branch protections, artifact retention, and current run results.

### Developer verification contract

1. Record the base branch and exact commit SHA before starting; inspect open pull requests and unfinished changes.
2. Make scoped changes with appropriate regression tests. Run available local checks when feasible.
3. Push to an appropriate working branch and use the existing GitHub Actions workflows to validate tests and build targets. Do not assume a workflow checks a platform or test suite without inspecting it.
4. Record workflow names, run URLs/IDs, conclusions, commit SHA, and build artifact names/retention where available.
5. Treat failing, cancelled, skipped, or pending required checks as **not verified**. Investigate failures; do not declare the milestone complete.
6. Distinguish **CI verified** (required automated checks pass) from **device/UI validated** (Windows and/or Android application tested on target hardware). Neither substitutes for the other.
7. Preserve build artifacts where workflows produce them; do not claim Windows installers, portable bundles, or Android APKs were generated unless the run actually produced them.
8. Update the checkpoint and architecture records with verified changes only. Never overwrite another developer's work, bypass required checks, or merge without the owner's agreed workflow.

### Additional checkpoint fields

```text
Base commit / resulting commit SHA:
GitHub Actions workflow names and run URLs:
Required checks and conclusions:
Windows build artifact (if produced):
Android APK artifact (if produced):
CI verification status:
Manual device/UI verification status:
Known failures, skipped checks, and coverage gaps:
Next safe action:
```

**Handoff rule:** Claude, ChatGPT, and human maintainers should resume from the last *verified* commit, not merely the latest pushed commit. If the current HEAD is newer, inspect and reconcile its changes before proceeding.

## 15. Proposal status register

| Proposal | Status | Notes |
|---|---|---|
| Operations Core | Proposed | Shared integration architecture |
| Unified Knowledge Retrieval | Proposed | Shared service, ASK primary consumer |
| Context Panel / knowledge relationships | Proposed | Explicit and reviewed links |
| Preparedness / Response / Recovery modes | Proposed | Existing UI not presumed implemented |
| Emergency Operations Center | Proposed | Incident-oriented workflows |
| Household readiness and dependency engine | Proposed | Deterministic calculations |
| Expanded inventory, evacuation, communications, medical | Proposed | Scope and safety review required |
| Seed Vault ↔ CNAR integration | Proposed | Regional prioritization, all-US offline corpus |
| Vault–Hub selective sync | Proposed | Requires contract and conflict design |
| Security, backups, resilience hardening | Proposed for audit | Existing implementations must be inspected |
| UI refinements | Proposed | Preserve current brand and navigation unless approved |

---

**End of proposal.** No code changes are authorized by this document. Any adoption should be reflected in the approved architecture, tracked as a milestone, and accompanied by verification criteria.
