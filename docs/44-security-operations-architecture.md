# 44. Security Operations Architecture

**Product:** CindrixOps (Windows host first; which editions get which parts is an open decision, section 15)  
**Status:** Approved scope and architectural design; implementation pending  
**Revision:** 1.3 — 2026-11-08 (1.2 of 2026-11-07 plus the carried-over requirements in section 14 and the open decisions in section 15; the separate earlier draft `44-security-operations.md` was merged into this file and removed)  
**Related documents:** 01 Architecture; 02 Files and folders; 03 Safety and licenses; 09 Passphrase lock; 11 Navigation; 18 Maps; 21 Academy; 26 Plans; 27 Emergency cards and plans; 30 Map drawing; 34 Sharing server; 35 Top bar and Dashboard; 37 Connecting layer; 43 Hub/Overwatch model.

> **Owner-approved product decision:** Security Operations is an integrated CindrixOps workspace, not a standalone Predator application. The separately named **Cindrix Predictor** repository is not changed. The exact name **Operational Response** replaces “Offensive Plans” in the UI and documentation.

## 1. Purpose, scope, and non-goals

Provide an offline-first workspace to manage household, homestead, campsite and preparedness-group safety: defensive security, watchstanding, patrol and inspection schedules, readiness, human-directed protective response, incident command, SOPs, training, equipment assignments and after-action review. Design for 4–5 people initially, without a fixed maximum team size.

Critical records and checklists must remain usable with no internet, AI model, server, radio, or other Cindrix application. No autonomous weapons, targeting, engagement, surveillance of unrelated people, or automatically ordered physical intervention. The system tracks information and assists human coordination; it does not substitute for emergency services or applicable law.

## 2. Navigation: one parent tab, six subtabs

Add exactly **one** top-level navigation item: **Security Operations**. Selecting it opens **Command** (the security dashboard), not an intermediate home page. The six subtabs appear below the parent, in this fixed order:

| Subtab | Owned workflows |
|---|---|
| **Command** | Detailed security dashboard, status, freshness, current watch, active patrols, incidents, overdue check-ins, critical tasks and quick access |
| **Security** | Site profiles, zones, gates/access points, physical and mechanical safeguards, passive alarms, camera references, inspections |
| **Operations** | Watchbill, six-hour rotation templates, patrols, check-ins, handover, duty journal, activated response assignments |
| **Planning** | Defensive Security Plans, **Operational Response**, contingencies, plan approvals and revisions, map/route references |
| **Personnel** | Team roles, availability, duty fitness/rest information, Academy training, exercises, qualifications, assigned equipment |
| **Records** | SOP/doctrine library, watch and patrol logs, incident records, after-action reviews, corrective actions and authorized exports |

This is a UI grouping, **not six independent databases**. A record has one authoritative owner and ID even when referenced by Command, Operations and Records. Desktop uses the existing top-tab + secondary-tab pattern; Android retains one parent item and a scrollable/overflow secondary tab strip. Tab deep links, keyboard accessibility and offline navigation are required.

Security Operations must be available during normal preparedness, not only when an emergency incident is open.

## 3. Two dashboards, one authority

**Main CindrixOps Dashboard:** include one compact, optionally hideable/reorderable Security Operations summary. Default indicators:
1. Active site's security posture, with last verified timestamp.
2. Current watch assignment/coverage and any gap.
3. Number of open incidents and unacknowledged alerts.
4. Security tasks, inspections or check-ins due/overdue.

Each item deep-links to the relevant Security Operations subtab, without copying sensitive personnel names, routes or site geometry into the general dashboard when locked.

**Command:** show active site/profile selector, posture and reason, last verification, watch now/next and handover, patrol assignments and last check-in, perimeter inspections, outstanding incidents/Overwatch alerts, communications health when known, readiness exceptions, due tasks and activity timeline. Offer manual actions to acknowledge, assign and open incidents (subject to authorization), as well as protected emergency quick access.

Both screens consume a common **read-only projection** built from the authoritative file records and integration adapters. Projection state is disposable/rebuildable and must never become authoritative. Derived counts and timestamps must agree between dashboards. “No report” is **Unknown**, never “secure”; absent feeds are **Unavailable/Stale**, never “all clear.” Demo values must be visibly labeled demonstration data.

## 4. Security posture model (approved)

Each security site has its own independently stored posture:

| Posture | Meaning |
|---|---|
| **Normal** | Routine protective operations; an explicitly set/verified posture, not a guarantee of safety |
| **Elevated** | Heightened attention or additional checks authorized by a human operator |
| **Emergency** | An active serious incident or emergency response, declared/confirmed by an authorized operator |
| **Unknown** | Not assessed, unverified, insufficient information or unusable current data |

Rules:
- Default to **Unknown** for new sites, missing source data or unverified posture. Never automatically downgrade to Normal after silence.
- Record posture, site ID, setter/authority, reason, source, effective timestamp, verification timestamp and optional review deadline.
- Operator-driven changes are authoritative. Optional deterministic rules can **recommend** Elevated/Emergency and flag status conflicts, but must not silently activate a plan or change physical actions.
- “Posture” and “source/communications health” are separate dimensions: a site may be Elevated with an unavailable camera feed. Preserve incident lifecycle independently; changing posture does not close incidents.
- If posture verification is overdue, display **Stale/verification due** alongside the last selected posture, without rewriting history. Unknown is an assessability state, not an alarm severity.
- Every transition is auditable, permissions checked, and shown on Command and the main dashboard from the same projection.

## 5. Multiple security site profiles (approved)

Support any number of separately identified **Security Sites**, initially suitable for Home, Campsite, Evacuation Destination, and other user-created locations. Profiles must not assume shared geometry, personnel availability, alarms, staffing or plans.

A site owns/references its own:
- Name, stable ID, local time zone, existing saved-map ID and optional general site notes.
- Zones, access points, measures, inspections, checkpoint references and per-site communication procedures.
- Watchbill templates, active duty roster, patrol routes and check-in policy.
- Defensive and Operational Response plan selections/versions.
- Active posture, incidents, operations log, and readiness thresholds.

Personnel and Inventory item identities remain shared via their existing owners, but Security stores **per-site assignments**. A radio can be allocated to a particular site's patrol without cloning the Inventory record. Each incident and watch entry must identify the site, and the interface must make the active site explicit. All-sites aggregate summaries must label unknown/stale contributors; they cannot claim all sites Normal based on only one site's data. Deleting a site must require explicit confirmation and preserve/export dependent records or show blocking references.

## 6. Emergency quick access (approved)

Provide a prominent, consistent **Emergency Procedures** entry in Command and a modest global entry reachable from the main dashboard/app chrome without requiring the user to navigate through several tabs. It opens an offline-first local index of **approved** site-specific and general procedures: evacuation, shelter-in-place, fire, medical assistance, communications failure, missing-person response, and other authorized protective procedures.

Requirements:
- Never depend on AI, Overwatch, remote services, internet, or background synchronization to open the index or locally available approved Markdown checklists.
- Show active site, procedure title, revision, approval/review date and whether a local printable version is available.
- Provide a clearly marked general fallback procedure index if site-specific content is missing; never silently substitute another site's plans.
- Reading a procedure must not automatically activate it. Activation, incident creation, team messages or status changes require an explicit authorized action and confirmation appropriate to the consequence.
- Respect lock state: public safety instructions can be accessible quickly; restricted perimeter locations, patrol patterns, personal information and sensitive contact details require authorized unlock. Design a printable/offline binder fallback and a test for power/network loss.
- Emergency access must not bypass existing security controls merely because its UI shortcut is prominent.

## 7. Feature ownership and module contracts

| Feature | Authoritative owner / contract |
|---|---|
| People and household identity | Existing Family/People records; Security stores role, availability and assignment references |
| Supplies, equipment identity and stock counts | Existing Inventory; Security stores issue/return, inspection and assignment references only |
| Training courses, quiz results and established sign-offs | Academy; Security stores exercise scenarios, session attendance and readiness evaluation links |
| Generic preparedness plans, emergency cards and Binder | Plan/Card/Binder; Security links approved site plans and renders printable protective procedures |
| Map tiles, route/shape geometry and saved maps | Maps / CMAP; Security stores overlay metadata and stable map references rather than a second map engine |
| Received intelligence and camera alerts | Overwatch/Overwatch Cam; Security imports opt-in alert metadata and acknowledgements with source/freshness |
| Radio/network message transport | Communications/AirLink; Security owns operational messages, schedule and check-in records |
| Field observations | Reuse FieldLog references where available; no duplicative field-note system required |
| Incident, watch, patrol and Security Site records | Security Operations local domain service |
| Cindrix Predictor | No presumed coupling until Predictor's product contract is independently verified |

Cross-app integration is optional, explicit, versioned, provenance-aware and authorized. If a source is offline, the Security record remains available; unresolved links render clearly, not as fabricated data. No cross-module write to another owner's authoritative record without that owner's validated API.

## 8. Domain models and lifecycles

All newly introduced records require a stable, immutable ID, schema version, site reference (where applicable), creation/update metadata and source/provenance. Reuse existing repository ID conventions when verified; do not migrate legacy IDs solely to resemble examples.

- **SecuritySite:** identity, display name, local time zone, map link, posture record, access-policy link.
- **SecurityMeasure:** site, zone, category, condition, inspection interval, last observation, next due, equipment reference.
- **Watchbill/WatchAssignment:** site, duty intervals, person references, role, relief, acknowledgement, exceptions.
- **Patrol/PatrolLog:** site, authorized route reference, checkpoints, team, scheduled inspections, check-in evidence and exceptions.
- **SecurityPlan:** site/general scope, type Defensive or Operational Response, revision, approval status, checklist, trigger, roles, communications, safety limits, termination criteria.
- **SOP:** versioned controlled text, owner, effective date, reviewer and superseded revision link.
- **TrainingExercise:** objectives, prerequisites, participants, evaluation, results, exercise-only marker.
- **Incident:** site, priority, operator-set status, source refs, timeline, decisions, tasks and closure evidence.
- **AAR:** incident/exercise ref, findings, actions, owner, due date and verification state.
- **OperationalAssignment:** site, team/person reference, equipment and task references, due time and completion confirmation.

Incident lifecycle: **draft → active → stabilized → closed**; an explicit, audited **reopen** operation is supported. No incident silently closes when a source alert stops. Watch assignment lifecycle: **proposed → scheduled → acknowledged → on-duty → relieved/completed**, plus missed/exception states. Exercise outcomes must never be counted as live alerts or operational actions.

## 9. Watchstanding, patrols, response and training

- Support flexible duty lengths, with a **six-hour watch template** for 4, 5 or more personnel; detect overlapping shifts, uncovered intervals, date/zone/DST boundary issues, unassigned relief, absence and unsafe rest assumptions. Roster suggestions are drafts until approved. No fixed patrol timing or identical-route requirement; route/schedule changes remain human approved and documented.
- Perimeter records include zones, physical barriers, mechanical/no-power alarms, access-control measures and inspection schedules. Check-in overdue triggers a **verification task**, not a presumption of intrusion or injury.
- **Operational Response** is the approved exact name for protective action plans. Include evacuation, shelter, medical/fire coordination, lawful de-escalation, communications failure and family accountability procedures. Require author, version, approval, last review, trigger and completion criteria.
- Security training complements Academy: drills, procedural review, radio discipline, first-aid readiness, exercises, performance evaluation, qualification freshness and AARs. Qualifications must distinguish attended/passed/signed-off/expired/unverified. No unsupported auto-certification.
- Security logs and AARs support immutable event IDs, attribution, correction entries and export with explicit redaction options.

## 10. File-first storage and consistency

Follow the existing CindrixOps storage policy: **no SQL/SQLite authority** for user content. User records are portable **JSON, Markdown and CSV** (PNG/JPEG only for visual attachments/exports). Do not add a hidden database or require a cloud account. Suggested layout under the existing user root:

```text
user/
  security/
    sites/<site-id>/site.json
    sites/<site-id>/posture.json
    sites/<site-id>/measures/*.json
    sites/<site-id>/watchbills/*.json
    sites/<site-id>/patrols/*.json
    sites/<site-id>/plans/*.md
    incidents/<incident-id>/incident.json
    incidents/<incident-id>/timeline.json
    incidents/<incident-id>/notes.md
    sops/*.md
    exercises/*.json
    aars/<aar-id>/review.md
    assignments/*.json
    exports/
```

Canonical shared data (Family, Inventory, Academy, Maps) **stays in its existing owners' files**. A Security file references those records by stable ID. Structured fields live in JSON with numeric `version`; long text lives in Markdown with stable metadata linkage. CSV may be used for compatible logs/exports. Generated dashboards and search indexes are disposable caches, never record authority.

Atomic temporary writes and replacement, version-aware readers, corruption detection, backups before destructive changes, consistent export/import and tested recovery after interruption are required. Do not silently overwrite or delete corrupted records. Use offset-aware timestamps/UTC instants plus each site's IANA time zone for watch schedules. A multi-file operation must have clear failure/retry rules and prevent split-brain status.

## 11. Permissions, privacy, safety

Recommended local roles: **administrator/planner**, **watch supervisor**, **member**, **read-only observer**. Apply permissions at service level for viewing restricted plans, editing duty rosters, confirming posture, opening/closing incidents, approving training, and exporting information. UI-only hiding is insufficient.

Site layouts, precise locations, household schedules and access procedures are sensitive. Respect Family lock, store minimum necessary information, redact main-dashboard summaries when locked, and clearly warn when user files/exports are not encrypted. No automatic publishing or cloud uploads. For future Vault–Hub local sync: explicit pairing, device identity, authenticated transport, bounded access, source timestamps, conflict surfacing, and auditable reconciliations are prerequisites; do not label synchronization secure until implemented and tested.

Protective decisions remain with people. The product must not automatically actuate security devices or order physical confrontation.

## 12. Acceptance and verification matrix

1. Navigation adds exactly one Security Operations parent; six subtabs appear in correct order; **Command** loads by default; prior routes remain intact.
2. Main dashboard and Command use the same projector and display identical posture, watch and incident facts, including source freshness and locks.
3. New site starts **Unknown**. Normal never results merely from absent alerts; Elevated/Emergency transitions record actor, time and reason; stale verification is surfaced.
4. Multiple sites maintain isolated watchbills, perimeters, incident IDs and plans; shared Inventory/Family IDs do not fork.
5. Global emergency shortcut works fully offline and with AI/server/Overwatch disabled; version/approval and site scope display correctly; restricted data remains protected.
6. Watchbill validates 4/5-person six-hour template coverage, overlaps, rest warnings, DST and overnight handover.
7. Missed check-in creates a traceable verification task, without falsely declaring an incident or safe status.
8. Record IDs survive rename and movement; missing external refs remain visible as unresolved.
9. Atomic writes, interrupted saves, backups, corrupted files and migrations are tested without silent data loss.
10. Live incidents and exercise data remain segregated; no silent incident closure or qualification issuance.
11. Access checks guard backend reads/writes and export; locked dashboard cards do not leak protected names/locations.
12. Android secondary tabs and emergency shortcut remain keyboard/screen-reader accessible and usable at narrow widths.
13. Optional integrations fail gracefully without blocking local plans, logs or watchbill.

## 13. Phased implementation and change control

**Phase 1 — Foundation:** audit existing Family, Maps, Academy, Plan, Inventory, navigation, storage and dashboard components; create Security domain contracts and site records; one new parent/six subtabs; shared dashboard projection; posture; Command and emergency quick access.

**Phase 2 — Routine operations:** watchbill and handovers; patrol and check-in logs; site perimeters, inspections, assignment references.

**Phase 3 — Plans and incidents:** Defensive Plans, **Operational Response**, controlled SOP revisions, incident lifecycle, printable response checklists, AAR.

**Phase 4 — Readiness:** exercises, training/AAR integration with Academy, role qualifications, equipment assignments and readiness summaries.

**Phase 5 — Optional connectivity:** Overwatch alert import and locally authenticated Vault–Hub field status synchronization after privacy/security and conflict-recovery review.

**Implementation authority:** This document captures approved architecture and requirements, **not a claim of code implementation**. Developers must reconcile it with current runtime code and existing documents before migration or UI modification. The existing numbered architecture and file-format constraints remain authoritative except where this document explicitly adds the approved Security Operations workspace.

## 14. Requirements carried over from the earlier draft (merged 2026-11-08)

An earlier draft (`44-security-operations.md`) put Security under Operations with about 14 tabs. The approved navigation in sections 2 and 3 replaced that layout. These requirements from it still apply:

- **Activating Operational Response.** Activation is always an explicit human action. It pins a snapshot of the exact plan revision and links it to the current incident. Editing the master plan later never rewrites a live incident's history.
- **Incident timeline.** Entries are append-only, each with its own ID, the time the event happened and the time it was recorded. Each entry says where it came from: direct observation, secondhand report, imported alert, or later correction. A correction is a new entry that supersedes the old one; the original stays.
- **Handover.** A watch handover needs the receiving person's acknowledgment, and a missed handover stays open as a gap. A handover done on paper or by radio can be entered later, keeping the real event time and the entry time apart.
- **Zero-power fallback.** A printable watchbill, patrol sheet, handover log and response checklists must work without the app.
- **Guests.** No security command goes on the share server's guest allowlist (`GUEST_COMMANDS`). A guest PIN is not an operator identity. A test must show every security read and write is refused for guests.
- **Untrusted input.** Treat imported text, image metadata, attachments and outside alerts as untrusted: check record types, sizes, IDs, timestamps, symlinks and path traversal. Attachments are size-limited, checked by file signature, never executed, and kept inside their record's own folder. No media stored as data URLs inside text records.
- **No outside traffic.** A test must show routine Security workflows make no network calls.
- **Encryption.** Plain files are not encrypted at rest. Do not describe security records as encrypted until the passphrase lock (doc 9) has been checked against them (open decision, section 15).
- **Suggested code layout** (not existing code; it follows the repository's flat layout and `CLAUDE.md` rules):
  - `core/src/security.rs`, `security_schedule.rs`, `security_incident.rs`, `security_store.rs`: validation, schedules and coverage, incident lifecycle, versioned file storage. Standard library only.
  - `src-tauri/src/security.rs`: thin, host-only command adapter. Screens reach it only through `src/lib/api.ts`.
  - `src/views/Security*.svelte`: the six subtabs and the dashboard summary.
  - `content/security/`: shipped templates (example plans, SOP and drill templates), each with a `Source:` line like the cards.
  - `core/tests/security_*.rs`: the acceptance tests in section 12.
  - Editions switch these screens on or off through `Features` in `core/src/product.rs`.

## 15. Open implementation decisions (do not assume)

- Which editions (Go, Vault, Hub) get which Security features, and whether Hub may write records.
- Whether sensitive records (site layouts, personal details, incidents) need a separate encrypted area, and whether the existing passphrase lock can cover them.
- ~~File format~~ **Decided 2026-11-08 (owner):** plain text in the same style as `tasks.txt` and `events.txt` (tab-separated lines, a `# format: <kind> <n>` first line, `\t` and `\n` escaped), so the records match the rest of the program. JSON or CSV may be used for a particular record type when plain text does not fit. The one requirement is that the data outlives the program: readable in any text editor, no hidden database. This replaces the JSON file layout in section 10 for what is built; section 10's rules (atomic writes, backups, never overwrite damaged files, version-aware readers) still apply.
- The real ID formats of Family, Inventory, Academy and Maps records, and the adapter names for each.
- Roles (section 11) need an identity for each person, and today there is only the host, an anonymous guest and trusted family devices. How roles are enforced, and how future multi-device check-ins are authenticated, is undecided.
- The exact Overwatch interchange format and what the importer may do.
- Whether watch and patrol schedules may generate reminders. The connecting layer (doc 37) has no notifications yet.

## 16. Camp defaults (owner, 2026-11-08)

Starting values for the Campsite site profile. They are defaults to edit, not rules the program enforces.

- **Four-person group.** One person stays in camp; the other three may leave. The watchbill template should work for four people first.
- **Pairs leave camp.** People go out in groups of two, so never all of them at once. Coverage should warn when a planned outing would leave fewer than two in camp or send someone out alone.
- **Perimeter alerts before cameras.** Power is limited (a 300 W solar generator and power banks), so the first camp measures are passive, low-power perimeter alerts. Cameras are an option, not the default; home cameras stay in Overwatch.
- **Every camp alert starts a verification task**, as in section 9, never a presumption of intrusion.

## 17. Built so far (2026-11-08)

Core logic only, with tests, in `core/`: `security.rs` (sites, posture, overview, instants with offsets), `security_schedule.rs` (watchbill, drafts, coverage, overlaps, short rest, handover "needs a look") and `security_store.rs` (files under `user/security/`, backup before replacing, append-only posture log, damaged files never overwritten). `security_incident.rs` (incidents built from one append-only log, `user/security/incident-log.txt`: draft, active, stabilized, closed, explicit reopen with a reason, timeline lines with event time and entry time and a source, corrections that replace without erasing, imported alerts unable to change status or activate a plan, exercises never counted as live) and, in the store, plan activation that saves the exact plan text to `user/security/incident-plans/` and pins its fingerprint in the log. Not built yet: patrols, plan and SOP files, training and AARs, the Tauri commands, the screens and the guest-refusal test (it needs the commands to exist).
