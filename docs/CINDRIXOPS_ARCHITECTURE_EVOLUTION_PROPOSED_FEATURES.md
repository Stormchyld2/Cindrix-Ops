# CindrixOps — Architecture Evolution & Proposed Features

**Document status:** PROPOSAL / DISCUSSION ONLY — NOT AUTHORIZATION TO IMPLEMENT  
**Version:** 0.6  
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

### 3.5 Making every module work together — worked scenario

This is where the most architectural effort should go. Consider an **extended winter power outage**:

```text
              Winter storm incident activated
   (selected manually, or created from verified imported information)
                                |
   Family ──── account for people and special needs
   Inventory ─ estimate fuel, water and food reserves
   Maps ────── identify alternate destinations
   Library ─── retrieve authoritative cold-weather guidance
   Plan ────── load winter outage procedures
   Comms ───── show contacts and radio check schedules
                                |
                  Unified operational picture
   Prioritized tasks · known shortages · verified references
                 · reassessment checkpoints
```

Design rules that follow from it:

1. **One authoritative record for each piece of information.** Changing a water quantity in Inventory updates the readiness assessment and the incident's resource estimates automatically. It must not require updating three separate records. Derived figures are projections that can be rebuilt (see 3.4).
2. **Each module works on its own when another is unavailable.** The Plan still opens if Inventory has not been filled in; Inventory still works if the Maps pack is missing; the incident view shows "not entered" for what is missing rather than failing. Offline, with no assistant model, or with a library drive unplugged, the essentials remain usable.
3. **Information is entered once.** A household of six entered on the Family page is the number the water planner, the inventory days-left figures and What Fails Next all use.
4. **Distinguish stored, portable and assigned supplies.** Home stores, evacuation loads, and supplies assigned to a person or vehicle are different pools (important when leaving unexpectedly).
5. **The system recommends; the person decides.** Starting an incident, evacuating, or changing medical instructions always needs the person's confirmation.

### 3.6 Internal layers (not new navigation tabs)

Organise the code and the data around three layers. Every module can contribute to an operational decision without being tightly coupled to every other module.

| Layer | What belongs in it |
|---|---|
| **Knowledge** | Library, Intel, Academy, agricultural references, offline maps, local AI retrieval |
| **Preparedness** | Household profiles, plans, inventory, seed management, equipment, maintenance, drills |
| **Operations** | Emergency checklists, resource forecasts, evacuation decisions, offline navigation, communications, recovery |

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

### 5.3 Illustrative incident dashboard (not live data)

A first sketch of the Response view, shown for discussion only: the active incident's name (for example *Extended power outage*), response mode, household accounted for (for example 4 of 4), stored-water estimate in days, backup-power hours, the next scheduled check, and a short list of immediate priorities with tick boxes: confirm everyone is safe and accounted for; check hazards and essential medical needs; review water, food and power; record conditions and the next reassessment. Numbers are examples. Real figures come only from the household's own records and are shown with their assumptions.

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

Easy to overlook, and valuable in prolonged emergencies:

| Capability | Why it matters |
|---|---|
| Incident journal | Records decisions, observations, actions and changing conditions with timestamps |
| Situation reports | Printable household or community status summaries |
| Shift and duty roster | Assigns responsibilities, check-ins and rest periods during long incidents |
| Resource rationing planner | Models alternative consumption scenarios without automatically imposing unsafe limits |
| Offline document vault | Identification copies, insurance documents, manuals and emergency records, encrypted |
| Financial continuity | Cash reserves, expenses, insurance claims and essential obligations |
| Neighborhood mutual aid | Voluntary contacts, available skills, resource requests and assistance records |
| Missing-person accountability | Last confirmed contact, expected locations and unresolved check-ins (unconfirmed is not the same as missing) |
| Weather observation log | Local temperature, precipitation, wind and manually entered alerts, without pretending to have live forecasts |
| Offline printing | Paper emergency plans, maps, medical summaries and contact sheets |
| Supply substitution references | Links shortages to verified alternatives and the relevant safety warnings |
| Post-incident review | Turns lessons learned into updated plans, drills and inventory requirements |

### 6.11 Dependency planner

Treat essential capabilities as a chain of dependencies instead of independent inventory items. For example: an **electric well pump** depends on **electricity**; a **generator** depends on **fuel**; **fuel availability** depends on both **storage** and **consumption**. The planner would identify these links and show how a **single failure affects several essential capabilities** at once (losing the generator takes out the pump, which takes out water, and so on). Dependencies are entered or confirmed by the person, shown plainly, and feed the readiness assessment and the incident resource estimates. Equipment status (6.5) and inventory (6.2) are its inputs.

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
8. **Extended winter power outage across modules:** activate the incident; Family, Inventory, Maps, Library, Plan and Comms each contribute to one operational picture; changing a stored-water quantity updates the readiness and incident estimates without a second edit; with one module's data missing or the assistant off, the rest still work and say what is missing.
9. **Single point of failure:** mark the generator unavailable; the dependency planner shows the well pump, water supply and heating that depend on it.

Success requires explicit expected outcomes, reproducible tests, and documented limitations—not merely that a screen renders.

## 12. Decisions needed from project owner

**Owner direction (2026-10-22):** Vault should be **both** a preparedness and reference application **and** a full emergency operations system: help people prepare, recognize what is happening, make informed decisions, carry out their plans and recover afterwards, all without the internet. Design for both a normal preparedness mode and a dedicated emergency operations mode. The scope of each piece still needs approval below.

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

## 15. Health, Holistic Medicine & Illustrated Materia Medica — owner-approved concept

**Approval scope (2026-10-22):** The owner approved recording the following product direction for future development. This is **concept approval and documentation only**; implementation, new dependencies, resource downloads, data migrations, and UI changes still require a separately approved milestone and repository review.

### 15.1 Product organization

Propose a Health & Medicine area containing Emergency Medicine & First Aid, Holistic & Traditional Medicine, Illustrated Materia Medica, Wilderness & Field Medicine, Home Health & Sanitation, and Medical Reference Library. Confirm the best navigation placement against the existing UI before implementation. Traditional practices and conventional emergency medicine must be clearly distinguished, with source provenance and evidence limitations.

### 15.2 Shared illustrated botanical knowledge resource

Build a **single authoritative botanical corpus** reused by Health, Library, Academy, ASK, Maps, Seed Vault, Inventory, Plan, and the proposed CNAR. Do not duplicate species data across modules. Use stable taxonomic identifiers and record synonyms, common names, taxonomy authority/version, native/introduced status where supported, region and habitat, identifying features, seasonality, cultivation, harvesting, traditional uses, clinical evidence, toxicity, contraindications, medication interactions, and citations.

Every species profile should support **locally stored, verified, zoomable photographs** of whole plant, leaves/stems, flowers, fruit/seeds, roots where appropriate, seasonal appearance, and harvested material. Provide side-by-side toxic-lookalike comparisons, diagnostic traits, and uncertainty notices. Never infer edibility or safe medicinal use from an image or AI identification alone. Store attribution, license, creator, source URL, retrieval date, verified taxon, and review status **per image**. Review redistribution rights before bundling. Botanical illustrations and geographic distribution maps may supplement photographs.

### 15.3 Traditional medicine and evidence

Reference categories: herbalism, ethnobotany, historical medical systems (including Ayurveda and Traditional Chinese Medicine), natural preparations, nutrition, hygiene, and toxicology. Label claims as supported, limited/preliminary, traditional use without adequate clinical evidence, unsupported, or known harmful, with source and review date. Do not present traditional use as proof of efficacy or suggest replacing essential medical treatment. Include red flags, interactions, pregnancy/child considerations where supported, and escalation to professional or emergency care.

### 15.4 Retrieval and cross-module integration

- **ASK:** Permission-aware retrieval of plant profiles, image galleries, lookalikes, citations, and evidence; show uncertainty and avoid definitive identification or prescribing.
- **Library:** Full source books, manuals, and articles linked to structured species profiles.
- **Academy:** Identification lessons, quizzes, safety and field-skills education.
- **Maps:** Offline range/habitat layers with data age; never imply an individual plant is currently present.
- **Seed Vault / CNAR:** Shared species IDs for propagation, planting windows, seed lots, and cultivation; region selection prioritizes but does not remove other US content.
- **Inventory / Plan:** Track user-owned herbs, seeds, preparations, storage lots, and relevant tasks without treating inventory as medical approval.

### 15.5 File-first resource packaging

Proposed versioned, offline botanical packs contain structured species profiles, verified media, source/citation manifests, taxonomy references, and integrity hashes. Search indexes and relationships should be rebuildable. Vault can install the complete corpus; Hub should support selective regional/subject packs. Do not mandate a new database or pack format before auditing existing CindrixOps conventions.

Illustrative organization only:

```text
botanical/
  taxonomy/
  species/<stable-id>/profile.json
  species/<stable-id>/identification.json
  species/<stable-id>/safety.json
  species/<stable-id>/media-manifest.json
  media/
  sources/
  indexes/                 # disposable/rebuildable
```

### 15.6 Staged plan and acceptance evidence

1. **Discovery and contracts:** inspect current architecture and source pack formats; define IDs, schemas, licensing, provenance, evidence levels, and security boundaries.
2. **Pilot corpus:** curate a small set of botanically verified species, including dangerous lookalikes, with multiple correctly attributed offline images per entry; test accuracy and usability before scaling.
3. **Illustrated encyclopedia:** expand the reviewed corpus and implement filtering, zoom, comparison, and accessible nonvisual descriptions.
4. **Medical reference content:** curate licensed/public-domain sources, evidence and toxicity profiles, and educational workflows.
5. **Shared retrieval and integration:** link ASK, Library, Academy, Maps, Seed Vault, CNAR, Inventory, and Plan using existing approved interfaces.
6. **Verification:** offline/no-AI search; source attribution; image/species matching; license audits; pack corruption and missing-media handling; dangerous-lookalike warnings; automated GitHub Actions tests/builds plus actual device validation.

**Planning targets, not commitments:** begin with approximately 500 thoroughly reviewed species, potentially expand to 2,000–3,000+. Preliminary complete-library storage estimate is roughly 4–18 GB, highly dependent on images, maps, and books; measure actual pilot packs before setting capacity requirements.

### 15.7 Open implementation decisions

Confirm final UI placement, source licensing strategy, editorial/medical review standards, taxonomic authority, pack distribution/update mechanism, Hub storage budget, safety-language policy, and the first approved implementation milestone.

## 16. Standalone Offline Map Viewer and `.cmap` Export Format — owner-selected format

**Decision (2026-10-22):** The owner selected **`.cmap`** as the native CindrixOps portable map export extension. This records a format decision and proposed implementation contract, **not authorization to implement an APK, server, or exporter yet**.

### 16.1 Goal and platform

A lightweight, standalone Android map viewer should import `.cmap` packages from CindrixOps Hub (Android) and eventually Vault (Windows), render maps offline without running the Hub server, and optionally display routes, waypoints, and user location where device GNSS is available. Reuse the existing map stack where practical after auditing actual Maps source, exports, Android packaging, and network support. The repository's package manifest currently declares MapLibre GL, PMTiles, and Protomaps basemaps; this does not prove export or receiver features already exist.

### 16.2 `.cmap` v1 proposed container

Use a versioned ZIP-compatible archive with a required root `manifest.json`, explicit content types, relative asset paths, and integrity hashes. Do not treat an arbitrary renamed ZIP or PMTiles file as valid. A conceptual layout:

```text
region.cmap
  manifest.json
  map/
    region.pmtiles
    style.json
    sprites/...
    fonts/...
  overlays/
    routes.geojson
    waypoints.geojson
  metadata/
    attribution.json
    licenses/
```

Manifest fields should include `format: "cindrix-cmap"`, `formatVersion: 1`, package ID, title, creation timestamp, geographic bounds, coordinate reference assumptions (WGS84 for external coordinates), minimum viewer compatibility, entries with byte lengths and SHA-256 digests, offline attribution/licensing, optional overlays, and required style/font/sprite resources. Do not assume ZIP archive byte offsets can be used directly by PMTiles readers: importer may need to extract PMTiles to a local file for random-access rendering.

**Validation:** reject path traversal, absolute paths, symlinks, duplicate/conflicting entries, unexpected file types, malformed GeoJSON, decompression bombs, truncated archives, and failed hashes; install atomically, preserving previously installed maps on failure. Define file size limits based on measured device constraints. Future versions must declare compatibility and migration rules.

### 16.3 Transport and offline operation

- **Preferred:** local HTTP transfer over Hub hotspot or a shared LAN, with discovery/pairing, explicit user approval, checksum validation, resumable downloads for large files, and no dependence on internet services.
- **Secondary:** Bluetooth transfer for smaller packages, routes, or waypoints; validate Android Bluetooth capabilities and permissions before committing to a specific protocol.
- **Fallback:** USB, SD card, and local file import.
- After successful import, the viewer must work without Hub connectivity. Device GNSS location is optional and must not be confused with an online map service.
- Test actual Android hotspot routing and client-to-host reachability on target devices before choosing implementation details. Prefer explicit pairing and authenticated transfers rather than an open unauthenticated download endpoint.

### 16.4 Viewer scope and acceptance

Initial viewer: import/manage/delete maps, offline pan/zoom, local GNSS location where available, view included routes/waypoints, offline attribution, storage usage, and understandable error handling. Later candidates: measure distance, user-created waypoints, route recording, multi-map switching, and selective export. Validate that all style dependencies are bundled and no external URLs are fetched during offline display.

Acceptance tests: package round-trip from exporter to viewer; import without network; hotspot transfer and resume; Bluetooth feasibility on supported devices; bad hash/unsupported version/missing tiles; no network calls during rendering; large-package and low-storage behavior; transfer interruption; preservation of existing installed maps; GitHub Actions build/test results and actual Android device verification.

### 16.5 Decisions still needed

Inspect existing Maps code and export pipeline; confirm the canonical `.cmap` v1 manifest schema, compression/extraction strategy, file size thresholds, Bluetooth transport, Android minimum version, and whether the viewer is a separate APK or shared-code application. Maintain a single format implementation for Hub and Vault exporters.

### 16.6 Owner-selected delivery sequence and technology stack (2026-10-22)

**Updated decision:** Build the standalone viewer **for Windows first**, then port/adapt it to Android with **Tauri 2**. Use the established CindrixOps stack: **Svelte, TypeScript, Rust, Tauri 2, JSON, Markdown, and ordinary filesystem storage**. Share the map rendering, package schema, Rust validation, and UI components wherever platform support permits; isolate OS-specific storage, Bluetooth, hotspot, and permissions behind adapters. Avoid introducing a separate UI framework or unrelated data store without explicit justification.

**Windows-first milestones (implementation requested; work not yet completed):**
1. Audit current CindrixOps Maps components, PMTiles sources, map styles/assets, and exporter behavior; preserve existing functionality.
2. Create a separate Windows Tauri 2 viewer project or clearly isolated workspace package using shared modules, with no requirement to run the main CindrixOps application.
3. Specify and implement `.cmap` v1 writer/reader and robust validation; support import/export of complete offline map packages and optional overlays.
4. Implement local map library, MapLibre rendering, navigation, offline source attribution, and user-facing import errors.
5. Add Windows filesystem import and local-network transfer first; test interruptions, resumability, authentication, and checksum verification.
6. Run Rust/frontend checks, Windows builds, and hands-on offline tests; record commit, CI run, and artifact.
7. Port the proven viewer to Tauri 2 Android; implement Android filesystem access, permissions, hotspot client/server reachability, and only then assess Bluetooth transport.

**Protection clarification:** The `.cmap` extension and proprietary schema provide file association, format validation, and packaging consistency, **not secrecy or DRM**. SHA-256 hashes verify integrity but do not prevent deliberate modification by an adversary. For confidential user maps, consider **optional authenticated encryption** with a user-held key/passphrase and a clearly documented recovery model; preserve an unencrypted option for emergency availability. Encryption and signing choices require explicit owner approval before finalizing v1 interoperability. Never describe a ZIP-compatible package as copy-protected solely because it uses `.cmap`.

## 17. Proposal status register

| Proposal | Status | Notes |
|---|---|---|
| Operations Core | Proposed | Shared integration architecture |
| Cross-module scenario, one-record rule, module independence (3.5) | Proposed | Design rules; owner direction: both preparedness/reference and operations |
| Knowledge / Preparedness / Operations layers (3.6) | Proposed | Internal division, not new tabs |
| Additional long-duration capabilities (6.10) | Proposed | Twelve items, each needs its own scope and safety review |
| Dependency planner (6.11) | Proposed | Person-confirmed dependencies, deterministic |
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
| Health & Holistic Medicine / Illustrated Materia Medica | Concept approved; implementation not authorized | Offline verified botanical photographs, safety/evidence profiles, shared knowledge corpus |
| Standalone Map Viewer / `.cmap` | Windows-first stack and delivery sequence selected; implementation requested, not yet completed | Tauri 2 + Svelte/TypeScript/Rust; Android after Windows validation; protection model pending |

---

**End of proposal.** No code changes are authorized by this document. Any adoption should be reflected in the approved architecture, tracked as a milestone, and accompanied by verification criteria.
