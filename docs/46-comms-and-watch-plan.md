# 46. Communications and watch plan (COPS-COM-001)

The owner's plan for radios, the six-hour watch, check-ins, lost contact, power and weather is kept word for word in
[COPS-COM-001-communications-and-watch-plan.md](COPS-COM-001-communications-and-watch-plan.md). It is the operating standard
for the communications side of Security (its section 16 says so). This page says where each part lands, and what is built.

The plan marks itself "draft for field validation". Nothing here checks the Kaito channel mapping, the radio coverage, the
charging times or the NOAA stations; those have to be tested at each site before the plan is relied on. The software is an
organizer. The plan's own rule applies: the radios must work without it.

## Where each part goes

| Plan section | Security subtab | State |
|---|---|---|
| 5 Six-hour watch, rotation, relief, five-person option | Operations (watchbill) | Watchbill, drafts that cover nothing until scheduled, coverage gaps, overlap, double booking and short rest are built. Night-duty rotation fairness and the relief position are not |
| 5.4 Shift turnover checklist | Operations | Handover "needs a look" is built. The checklist form and the "I have the watch" step are not; the card **Handing over the watch** carries the steps |
| 6 Check-ins, departure/arrival, expected return, next contact | Operations ("Out of camp"), Command (needs a look) | **Built** (`core/src/security_outing.rs`): who left, the task, expected return, next contact, method; check-in, checkpoint, change time (reason required), back safe, called off. Late contact or return is only a "needs a look" line (10 and 15 minutes of grace), never an incident. Help and delay reports come first. Going out alone is flagged (pairs rule) |
| 6 / 12 Patrols and checkpoints | Operations | **Built** as a patrol: the same record with named checkpoints and the time the last patrol came back. Patrol routes planned in advance and patrol schedules are not built |
| 8 Red, Amber, Green, emergency message order | Cards | Card **Emergency radio call** built |
| 9 Missed check-in, lost radio contact, base radio failure | Cards | Cards **Missed check-in or radio contact lost** and **Base radio failed or lost contact with a patrol** built |
| 3 Channel plan, channel change | Planning, Cards | Card **Moving to the back-up radio channel** built. The per-site channel card is not |
| 10 Power and battery | Planning, Inventory | Not built. Radios and batteries should be Inventory items; the test of back-up charging is a step in the plan |
| 11 NOAA stations | Planning | Not built. West Virginia stations are in the plan text; Tennessee is to be set once the place is known |
| 12 Coverage survey and 0 to 5 scale | Planning (site map) | Not built. Weak places should be marked on the site map and change the check-in method |
| 13 Communications log | Records | Not built. The plan's eight categories map onto the incident log's entry kinds plus a few new ones |
| 15 Operator training | Personnel | Not built. Twelve skills per person, recorded with a date |
| 14 Daily readiness, 12 survey, 15 practice | Plans | Built as the plan **Communications and watch routine** (repeating steps) |

## Rules from the plan that the software must keep

- A missed check-in is looked at in context. It is never turned into an incident by itself.
- Green means "this task is done", never "everything is secure".
- An unattended radio is not a watch. Responsibility passes only when the next person accepts it.
- No part of the radio procedure depends on the internet. Everything essential is a plain file that can be printed.
- Privacy codes are not encryption, and the primary net keeps them off.

## Built in this round

Four response cards (`response-emergency-radio-report`, `response-radio-blackout`, `response-radio-channel-change`,
`response-watch-handover`) and the plan `content/plans/comms-and-watch.md`. All are drafts for the family to edit and check.
The plan has no button into Security yet, because plans can only jump to the older screens.

## Built in the second round (2026-11-08)

The outing and patrol log (`user/security/outing-log.txt`, append-only like the incident log): lines are never changed, a log
that cannot be read is never added to, only a person ends an outing, and nothing the log does touches the incident log (a
test proves it). Command's "Needs a look" and the Dashboard count come from the same lines as the Out of camp list. Not
built: phone-sent check-ins (doc 45), route planning, and a comms log.
