# How to add a plan

A plan is one text file. Copy `_template-plan.md`, rename the copy (letters, numbers and dashes, for example
`hurricane.md`), change the words, and put it in a `plans` folder: this one, a `plans` folder on your
drive, or `user\plans`. The Plans screen shows it the next time it is opened. Files that start with `_` (like this guide and the template) are ignored.

Top of the file (between two lines of three dashes):

| Line | Meaning |
|---|---|
| `title:` | The plan's name (required) |
| `summary:` | One sentence for the plan's card |
| `kind:` | `checks` (repeat on a schedule), `learning` (things to study), `scenario` (one kind of emergency), `prevent` (keep one supply from failing: what to do and how) or `planning` (how much to have) |
| `author:`, `version:`, `status:` | Who wrote it; a bigger version replaces an older copy; `draft` until a qualified person has read it |

Each step starts with `## Title`. The lines straight after it can be:

| Line | Meaning |
|---|---|
| `every: 90` | The step comes back 90 days after it is ticked. Without it the step is done once. |
| `check: water` | The step ticks itself. Values: `water`, `food`, `family`, `inventory`, `drill`, `maps`, `model`, `library`, or `lesson:<id>` (an Academy lesson whose quiz you took). Not used together with `every`. |
| `go: Inventory` | A screen to jump to (Dashboard, Ask, Library, Intel, Academy, Maps, Music, Inventory, Drills, Family, Binder, What fails, Cards) |
| `button: Open Inventory` | The words on that button |

Everything after that is the explanation. Lines that start with `- ` are shown as a list.

**Plans never give medicine doses.** A plan that does is left out and listed on the Plans screen.
