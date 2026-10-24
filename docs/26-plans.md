# 26. Plans that carry on

The walkthrough in Plan, Start ("Get ready") is unchanged. It ends with a box that leads to **What next**, where plans pick up after it.

## What a plan is
One text file in a `plans` folder: the program's own (`content/plans`), a `plans` folder on a drive, or `user/plans`. Anyone can add one. See `content/plans/_how-to-add-a-plan.md` and `_template-plan.md` (files starting with `_` are ignored). Three kinds:

- **Repeating checks** (`kind: checks`): steps with `every: N` days. Tick one and it comes back when N days have passed.
- **Learning** (`kind: learning`): steps that tick themselves when you take an Academy lesson's quiz (`check: lesson:<id>`).
- **Emergency plans** (`kind: scenario`): what to have ready and do for one kind of emergency.

A step can also tick itself from what is set up (`check: water`, `food`, `family`, `inventory`, `drill`, `maps`, `model`, `library`). A step with `every:` is always ticked by hand.

## Several at once
Open a plan and press "Follow this plan". Any number can be followed. The Dashboard shows each followed plan's progress and its next step, and a tile counts the checks that are due.

## Where progress is kept
`user/plans.csv` (kind, plan, step, date). A damaged file is reported and never overwritten. Steps are identified by their title, so reordering a plan loses nothing. A newer `version:` of the same plan replaces an older copy.

## Shipped plans (drafts until reviewed)
Keep it fresh (11 repeating checks), Learn the basics (10 steps from the Academy), Power outage, Winter storm, Flood.

## Rules
Plans never give medicine doses; a plan that does is left out and listed on the screen (the same check as Academy courses). Code: `core/src/plans.rs`; tests in that file and `core/tests/shipped_plans.rs` (every shipped plan loads, and its screens, checks and lessons exist).

## Not built yet
Reminders when a check is due (there are no notifications yet), plans for a household member, a plan pack file.

## The Binder

The Binder (Plan > Binder) is the paper copy. Besides family details, contacts, supplies, emergency cards and drill checklists it now holds: a contents list, the plans you pick (followed ones start ticked; each step has a box, how often it repeats and when it was last done), the next 60 days of the calendar, the open to-do list, projects with their tasks, any guide pages (Agro and radio reference, picked page by page or a whole section), and blank lined note pages. A plan's own screen has **Print in Binder**, which opens the Binder with that plan ticked. The choices are remembered in the browser part of the program (not in `user/`). Anything marked DRAFT prints with a warning. It is all on-screen layout and printing; no new logic lives in `core/`.
