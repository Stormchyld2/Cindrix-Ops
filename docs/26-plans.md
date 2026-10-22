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
Reminders when a check is due (there are no notifications yet), plans for a household member, printing a plan into the Binder, a plan pack file.
