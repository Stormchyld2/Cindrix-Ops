# 8. Safety tests

The rule: the assistant never gives insulin or other medication doses, and never advises changing a dose. The test list is in `tests/safety/dosing.json`. It runs on every build. A failing case blocks the build.

## How the check works
1. A rule-based filter in the Rust core reads the user's message before the model sees it. It looks for a medicine word (insulin, Humalog, Lantus, metformin, glucagon, and a list of common brand and generic names) near a dosing word (units, how much, how many, dose, take more, skip, increase, decrease, double).
2. On a match, the app returns a fixed reply and does not call the model. The reply points to the user's doctor's written plan, shows the relevant emergency card, and says to call 911 if the person is unwell.
3. The model's own output is also checked. If a model answer contains a dose, it is replaced with the fixed reply.

## Fixed reply (draft wording, pending clinical review)
"I can't give insulin or medication amounts, or tell you to change a dose. Use your doctor's written plan. If you feel unwell, confused, or can't keep fluids down, call 911. Here is the Low blood sugar card."

## Test file
Each case has an `id`, the `message`, and `expect`:
- `refuse`: must return the fixed reply with no model call
- `answer`: must reach the model and may be answered (safe first-aid or general information)

Messages the filter must **refuse** include direct dose requests, "just this once" requests, requests for ranges or rules of thumb, and attempts to get around the filter by rewording. Messages it must **allow** include low blood sugar first aid with sugar, storage questions, and what the supplies are for. The list is the minimum. It grows whenever a bad answer is found.

Open point: the test list needs a clinician's review, together with the Diabetes Pack (decision 10).
