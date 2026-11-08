# 44. Security & Operations — Architecture

**Status:** Approved product scope; architecture specification, NOT IMPLEMENTED  
**Decision date:** 2026-11-07  
**Location:** CindrixOps core application (Go/Vault/Hub feature availability subject to edition gating)  
**Naming:** The approved name for protective action planning is **Operational Response**. Do not create a separate "Predator" application or an "Offensive Plans" module.

## 1. Purpose and boundaries

Security & Operations is an integrated workspace for household, homestead, campsite and preparedness-group safety. It supports routine security measures, passive perimeter inspections, watchstanding, non-confrontational patrol coordination, defensive planning, protective emergency response, training, incident documentation, and review. Initial UX targets a four- or five-person team; data structures must support larger groups.

CindrixOps owns these records. Cindrix Overwatch owns intelligence feeds and monitoring; Overwatch Cam owns camera streaming; existing Maps owns map layers and geometry; Inventory owns stock and equipment; Family/People owns person details; Academy owns learning materials and completion records; Operations Core owns general tasks/events/projects. Security links to those owners rather than copying their data. Cindrix Predictor remains a distinct product and is **not** this security module.

An alarm observation is not proof of an intruder. The UI always distinguishes confirmed observations, reports, unverified alerts, and unknown status. No autonomous threat classification, targeting, force recommendations, or automatic deployment decisions. The system prioritizes observation, avoidance, de-escalation, emergency services where available, shelter, evacuation, medical aid, and accountability. Any legally sensitive use-of-force policy is user-authored and reviewed externally; the software does not certify legality or authorize force.

## 2. Approved functional modules

| Module | Essential workflows | Owned records |
| --- | --- | --- |
| Security Dashboard | Current duty coverage, overdue check-ins, open incidents, current plans, readiness gaps; never imply live status from old reports | Derived view only |
| Security Operations | Start/end duty periods, shift handover, activity log, assigned operational tasks | Security shift, handover |
| Watchbill & Duty Rotations | Six-hour watch blocks by default, configurable stagger, overlap and relief; four/five-person templates; rest/fatigue warnings | Watch schedule, watch assignment |
| Patrol Management | Planned inspection routes, assigned people, check-in windows, checkpoint inspections, exception reports | Patrol assignment, check-in, observation |
| Perimeter Security | Defined zones, gates, access points, passive/no-power alert devices, maintenance and inspection results | Zone, measure, inspection |
| Defensive Security Plans | Site protection, barriers, shelter-in-place, safe movement, evacuation, medical points, communications fallback | Defensive plan revision |
| **Operational Response** | Protective action checklists, escalation and decision points, team assignments, accountability, emergency contingencies | Response plan revision, activation |
| SOP & Doctrine Library | Standing orders, watch instructions, radio procedures, incident handling, versioned approvals | SOP revision, acknowledgment |
| Training & Exercises | Lesson plans, drills, scenarios, checklists, evaluation, refresher dates | Exercise, training outcome |
| Personnel Readiness | Role suitability, training completion, availability, duty/rest state, assigned gear references | Role assignment, readiness assessment |
| Incident Management | Report, verify, classify, coordinate, communicate, resolve, and archive | Incident, timeline entry, decision |
| After-Action Reviews | Evidence-backed observations, lessons learned, corrective tasks, review and closure | AAR, corrective action |
| Communications | Comms plan references, callsign directory, check-in acknowledgments, outage procedures | Security-specific comms protocol refs |
| Tactical Mapping & Overlays | Security zones, routes, observation locations, rally/medical areas and planned inspection checkpoints | Security overlay and geo-links |
| Equipment & Gear Assignments | Link to authoritative Inventory items; assignments, inspection/return and readiness | Assignment, equipment inspection |

The Security workspace should be discoverable from the existing Operations navigation, rather than creating redundant top-level application chrome. Labels should be plain-language and accessible offline.

## 3. Runtime / component architecture

```text
Svelte 5: Security workspace (Dashboard, Watchbill, Patrols, Plans,
  Operational Response, Training, SOPs, Incidents, AAR, Settings)
                       |
             src/lib/api.ts only
                       |
       Tauri command adapter / local host adapter
                       |
Rust core: security module
  - validation and file IO
  - schedules / coverage / conflicts
  - lifecycle/state transitions
  - readiness and due-date rules
  - cross-module reference resolution
  - export / import and audit
                       |
   Plain versioned files under user/security/
                       |
Existing owners: ops.rs, people/Family, Academy, Inventory,
Maps, share server, optional Overwatch integration
```

Follow `CLAUDE.md`: Rust `core/` is **standard-library-only** unless a separate approved change revises that policy. Keep Tauri adapters thin and all testable logic in `core/`. UI access to the backend goes through `src/lib/api.ts`. Check product gating in `core/src/product.rs` before exposing commands and routes; do not silently expose private security data to a browser guest. Never introduce SQL/SQLite or a remote service for the canonical records. Cached or derived views can be rebuilt.

Suggested implementation files (not claims of existing code):
- `core/src/security.rs`: record validation and core orchestration
- `core/src/security_schedule.rs`: shift allocation, coverage, fatigue checks
- `core/src/security_incident.rs`: incident lifecycle and immutable event identity
- `core/src/security_store.rs`: versioned file persistence, safe migrations
- `src-tauri/src/security.rs`: host-only Tauri command adapter
- `src/views/Security*.svelte`: screens using the shared UI conventions
- `core/tests/security_*.rs`: deterministic acceptance tests

## 4. Storage layout and identity

All personal records are host-only on the user's drive under `user/security/`. Proposed layout:

```text
user/security/
  manifest.txt
  sites/<site-id>/site.txt
  sites/<site-id>/zones/<zone-id>.txt
  sites/<site-id>/overlays/<overlay-id>.json
  watches/<watch-id>.txt
  patrols/<patrol-id>.txt
  checkins/<checkin-id>.txt
  measures/<measure-id>.txt
  plans/defensive/<plan-id>/revision-0001.md
  plans/response/<plan-id>/revision-0001.md
  sops/<sop-id>/revision-0001.md
  exercises/<exercise-id>.txt
  readiness/<assessment-id>.txt
  incidents/<incident-id>/incident.txt
  incidents/<incident-id>/timeline.txt
  incidents/<incident-id>/attachments/
  aars/<aar-id>.md
  gear/<assignment-id>.txt
  audit/changes.log
```

Example directory structure is normative as a planning target, not implemented. Exact filenames can be reconciled with existing `userdata.rs` conventions before code is written. Do not store media as data URLs inside text records. Attachments are size-limited, file-signature-validated, never executed, and paths are contained beneath their owning directory.

**Identity contract:** use stable opaque IDs with type prefix + UUID v4 for new security records. Titles, dates, file paths and roster order are not identity. Preserve record ID across rename/move/revision. Refer to existing Family, Inventory, Academy, Maps and Ops record IDs by their actual existing formats; do not fabricate UUIDs or create duplicate copies to force format uniformity.

Records carry `schema_version`, `id`, `created_at`, `updated_at`, `author_ref` when known, `status`, and typed `links`. The original source is authoritative. Records and logs must distinguish a scheduled activity from a verified completion. Timestamps are ISO 8601 with offset; store UTC for ordering and retain the site's time zone for local watch display. Handle overnight shifts and daylight-saving changes explicitly.

Example **illustrative** watch record (`watch-*.txt` can use the repository's existing tab-separated convention; this JSON represents fields, not mandated on-disk syntax):

```json
{
  "schema_version": 1,
  "id": "watch-<uuid-v4>",
  "site_ref": "site-<uuid-v4>",
  "person_ref": "person-existing-id",
  "start_at": "2026-11-08T00:00:00-05:00",
  "end_at": "2026-11-08T06:00:00-05:00",
  "post_ref": "zone-<uuid-v4>",
  "status": "scheduled",
  "relieved_by_ref": null,
  "notes": ""
}
```

`scheduled` is never automatically rendered as `on duty`. Only explicit check-in/acknowledgment establishes reported duty state, with last-confirmed timestamp.

## 5. Watch and patrol scheduling

- Predefined 4- and 5-person coverage templates, with **six-hour** duty blocks as the owner's preferred default; user-editable durations and coverage counts.
- Schedules are based on coverage requirements, qualified availability, required rest, and voluntary time constraints. Show 24-hour coverage gaps, double booking, insufficient staffing, overnight conflicts and continuous-work risk **before** publishing a roster.
- A watch handover requires a receiving acknowledgment; missed handover remains an unresolved gap. If connectivity is absent, manual paper/radio handover is possible and can be entered later with the actual event time and entry time distinguished.
- Patrols represent safety inspections and accountability: routes/checkpoints, expected check-ins, observed issues, and exceptions. A patrol assignment does **not** imply that an individual is present, available, or safe without an acknowledged report.
- Varying route or checkpoint timing may be manually configured for operational flexibility, but defaults emphasize visibility, safety, safe navigation and adequate rest. Do not silently generate high-risk patrol instructions.
- If communications fail, report the last confirmed position/time, never fabricate live tracking. An overdue check-in creates a reviewable flag, not a declaration of harm or a hostile contact.
- Map route and inspection data are locally stored references to current map layers; avoid duplicating raster/vector packages.
- Print-ready watchbill, patrol sheet and handover log are needed for zero-power fallback.

## 6. Perimeter, defensive plans, and Operational Response

**Perimeter measures** are inventoried as layered safeguards: entrances, signage, physical barriers, passive mechanical alarms, optional local cameras, lighting and communications. Each measure records placement, responsible person, last inspection, maintenance due, and status (`unknown`, `working-reported`, `needs-attention`, `out-of-service`). Never claim a site is secure just because all checklist items are marked.

**Defensive Plans** cover site maps, access control, shelter locations, evacuation exits, family reunification, medical aid, fire prevention, weather contingencies and safe retreat. Plans link to Map features and existing CindrixOps Plans without duplicating base map datasets.

**Operational Response** is the approved name for the formerly discussed offensive-plan section. It is a library of **protective incident-response playbooks**: trigger and verification, safety objectives, responsible roles, communication, shelter/evacuation decisions, accountability, escalation to emergency responders when available, medical assistance, and termination criteria. Each plan is revisioned, has an owner and acknowledgment status, and must be explicitly activated by a user. No plan is executed automatically. Plans may contain externally reviewed policy attachments but the software does not offer tactical engagement optimization, targeting or instructions to harm individuals.

Activation creates a snapshot of the chosen plan revision plus links to the current incident. Editing the master plan never silently rewrites a live incident's history. Plans work with no network or assistant model.

## 7. Incidents, training, SOPs and after-action review

**Incident lifecycle:** draft -> reported -> acknowledged -> active -> stabilized -> closed -> archived. Reopen produces a recorded event and preserves historical status changes. Each transition has actor, timestamp, reason, and optional referenced evidence; absent actors are labeled `unattributed` rather than invented.

**Incident timeline:** append-only logical events with unique IDs, ordered by event time but retaining the actual recording time. Correcting a report creates a superseding entry; it does not erase the original. Mark source as direct observation, secondhand report, imported alert, or later correction. Incidents can link relevant inventory, plan, task, zone and map references.

**Training and Exercises:** use Academy course references for teaching content. The Security module owns exercise attendance, scenario safety briefing, evaluation rubric, pass/incomplete status, qualification expiration, and refresher recommendations. Training results cannot be equated with legal certification. Include first aid, communications, evacuation, fire response, observation/reporting and de-escalation drills. Keep training records editable with an audit trail.

**SOP Library:** versioned documents, scope, approver, effective date, review date, acknowledgments and superseded revisions. No document silently becomes authoritative merely because it is imported. A published SOP's revision used for an activity must remain retrievable.

**AAR:** activity/incident link, expected outcome, observed outcome, contributing conditions, action item, accountable owner, due date and verification of completion. Use the existing `ops.rs` tasks for corrective-action tracking where feasible.

## 8. Integration contracts and ownership

| Source | Contract | Ownership / protection |
| --- | --- | --- |
| Family / People | Read person references, availability as explicitly recorded | Family retains canonical identity and private profile |
| Existing Operations (`ops.rs`) | Link security tasks, calendar events, project and audit entries | Ops owns general tasks/events; Security owns operational facts |
| Academy / training | Reference courses and existing sign-off; add security exercise-specific outcomes | No duplicate course catalog |
| Inventory | Link gear by inventory ID, reconcile availability at inspection time | Inventory owns quantity/location |
| Maps | Reference existing map resources and store overlay metadata separately | Maps owns geographic base data |
| Share server | No security write or private record read for ordinary guests | Explicitly test against `GUEST_COMMANDS` allowlist |
| Cindrix Overwatch | Optional **manual import** of alert summaries via documented versioned interchange; retain source and timestamps | Overwatch owns alerts and feeds, no automatic threat confirmation |
| Communications / AirLink | Optional exchange of acknowledged messages if later supported | No assumption that radio or mesh transport already works |
| Predictor | No required integration | Separate application; no scope conflation |

Do **not** implement speculative integrations as if the connected products already expose compatible APIs. Define a transport-independent `SecurityService` interface first; an optional local authenticated import/export bridge can follow after protocol review.

## 9. Privacy, security, reliability and permissions

1. Default security records to **host-only**. Existing guest PIN is not a security-operator identity and grants no access. Do not add security commands to the guest allowlist without a separate reviewed authentication design.
2. The existing passphrase-lock mechanism must be evaluated before security details can be called encrypted; plain files provide no encryption at rest. Sensitive mapping/location data needs clear user warnings and optionally encrypted vault storage after security review.
3. Treat all text, imports, image metadata, attachments and external alerts as untrusted. Validate record types, max sizes, path traversal, symlinks, IDs and malformed timestamps. Disable executable attachments.
4. Use `fsio::backup_then_write` / `write_atomic`, and retain backups on corruption or migration failures. Never overwrite damaged records silently. A failed write must not display success.
5. Avoid internet calls, telemetry, cloud accounts, and external tracking. Optional Overwatch imports must not become a hidden outbound network dependency. Sharing stays off according to existing host/server rules.
6. Multi-user editing and remote operator access are **not** available merely because a share server exists. Future editable clients require identity, authorization, audit, and conflict resolution; until then, use host-only writes and export/print.
7. Reconciliation distinguishes `unknown`, `not-configured`, `not-reported`, `overdue`, `confirmed`, and `stale`. Show freshness and who made each report.
8. Security alerts must remain reviewable, dismissible, and fail safely. Do not trigger external action without explicit operator confirmation.

## 10. UI / navigation proposal

Operations > **Security** opens a workspace with local tabs:

```text
Overview | Watchbill | Patrols | Perimeter | Defensive Plans
Operational Response | SOPs | Training | Personnel | Incidents
AAR | Communications | Maps | Assigned Gear
```

Keep simple initial navigation: Overview, Duty (Watchbill + Patrols), Plans (Defensive + Operational Response + SOPs), Readiness (People + Training + Gear), Incidents (including AAR), and Map. The internal modules above remain separately addressable despite grouped tabs. Support keyboard-only use, good contrast, large print, and printable offline forms.

Overview shows `Next watch`, `Last confirmed check-in`, `Coverage gaps`, `Open incidents`, `Outstanding inspections` and `Readiness issues`. Never show fictitious live data in production. Dashboard card values must be computed from actual records with source timestamps.

## 11. Acceptance tests

Minimum automated test cases:
- Save/read/reopen each record type without network or SQLite; old records survive restart.
- IDs remain stable across file rename/move and plan revision; duplicate IDs are rejected.
- Atomic-write failure keeps the previous copy; corrupt record is flagged, not erased.
- Overnight/DST six-hour duty rotation and 4-/5-person rosters calculate coverage accurately.
- Coverage gaps, overlapping duty, unavailable person and inadequate rest are surfaced without silently changing assignments.
- A planned watch is not counted as confirmed attendance; stale and missing check-ins are labeled.
- Missing connected module/drive does not prevent reading security plans and watchbills.
- False/duplicate Overwatch alerts are not treated as validated incidents automatically.
- Incident timeline correction preserves prior entries; activation pins exact plan revision.
- Guest requests for security data and all security write actions are denied; host remains functional offline.
- Unauthorized path traversal/imports and malformed IDs/dates are rejected safely.
- Exported paper watchbill, SOP and response checklist function without app access.
- AAR corrective tasks retain links; archive/reopen never loses history.
- No outbound network traffic occurs during routine Security workflows.

CI: `cd core && cargo test --all-targets`, `npm run check`, `npm run build`; include security-specific tests in existing workflows. Verify on Windows host first, then Hub on Android after its platform groundwork is proven. Do not claim live multi-device sync or tested Android support before tests exist.

## 12. Implementation stages

**Stage 1 — Usable local foundation:** data models and safe storage, site/zone configuration, watchbill, six-hour rotation templates, personnel references, simple handover log, Security Overview, tests and printable forms.

**Stage 2 — Routine security:** patrol/inspection logs, passive perimeter measures, assigned gear, readiness and recurring inspection tasks; integrate authoritative Inventory/Maps/Academy links.

**Stage 3 — Planning and training:** Defensive Plans, **Operational Response**, SOP revision management, training/exercise records, acknowledgment and document export.

**Stage 4 — Incident management:** incident state machine, event timeline, AAR, incident-linked tasks, evidence handling, optional **manually imported** Overwatch alert summaries.

**Stage 5 — Local team expansion:** revisit secure authenticated local clients, verified device-to-device synchronization, conflict handling and feature gating only after explicit owner approval and security review.

## 13. Open implementation decisions (do not assume)

- Which product editions (Go, Vault, Hub) expose each Security feature and whether Hub is writable.
- Whether sensitive security documents require a separate encrypted storage area.
- Whether to use tab-separated files consistently or scoped JSON for complex records while preserving the file-first contract.
- The actual existing person/inventory/map ID formats and adapter names.
- The permitted authentication model for future multi-device security check-ins.
- Exact Overwatch interchange format and importer permissions.
- Whether patrol or watch scheduling may have automatically generated reminders; notifications are currently not implemented in the connecting layer.

**Change-control:** This document approves functional scope and the architectural direction; it does not mean the modules have been built. Reconcile with current code and tests before implementation. Maintain this document alongside the implementation, record deviations, and avoid changing unrelated existing CindrixOps features.
