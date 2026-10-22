# Emergency cards, walkthroughs and the training sign-off (2026-10-22)

## Emergency cards (`content/cards`)
49 cards in three groups (the `kind:` line): `medical` (first aid and health), `disaster` (weather and big events)
and `home` (fire, gas, power, water, staying put). Every card is a draft until a qualified person fills in `reviewer:`
and `reviewed:`. Every card ends with a `Source:` line. A test (`core/tests/shipped_cards.rs`) checks each card loads,
has a source, says when to call for help, and never names a medicine amount.

How the text was made: written in our own words from public guidance. The pages below were opened and checked on
2026-10-22: Ready.gov (earthquakes, tornado, floods, heat, home fires, radiation, the hurricane information sheet, the
kit page), CDC (heart attack, stroke, carbon monoxide, making water safe), the Red Cross (adult CPR, survival kit
supplies), Poison Help and Stop the Bleed (partly). The rest (choking, burns, wildfire, winter storms, USDA food times,
tsunami, and so on) follow the same agencies' standard guidance but their pages could not be opened from the build
environment that day, so **they still need a person to check them against the live pages** before the first drive is
sold. Do that when the clinician review is done.

Your own cards: drop a `.md` file in `user\cards` (Settings, "Your own cards and plans", opens it and puts the template and
guide in). Cards are also read from a `cards` folder at the top of the drive and of other libraries. A file with the
same name in a later folder replaces the earlier one. A card that cannot be read or names a medicine amount is left out
and listed in Settings.

## Plans: new kinds
`prevent` (keep one supply from failing: what to do, and how), `planning` (how much to have), joining `checks`,
`learning` and `scenario`. The What next page groups them. 27 were added: 14 "keep it from failing" walkthroughs (one
for each part of the What fails page, which now links to them), 12 emergency plans (hurricane, wildfire, tornado,
earthquake, extreme heat, water emergency, evacuation, house fire, illness at home, chemical or radiation, lost or
stranded, weeks to months) and **How much to have on hand**.

## Inventory
* More questions when an item is added: brand or type, size of one, model or serial number, a goal ("want to keep"),
  who it is for, where it is, bought, use by, last looked at, condition, cost, where to get more, and a few prompts that
  change with the category. The CSV gained ten columns at the end; older files still load. An item below its goal shows
  as "Check", and as "Act now" when none is left.
* Five new categories (20 in all): Fuel and heat, Cleaning and laundry, Garden and long-term food, Medical equipment and
  special needs, Fishing and outdoors.
* **How much should I have?** (`core/src/stockplan.rs`): from adults, children, babies, pets and days it lists what to
  have. Red Cross numbers are used where they exist (1 gallon per person per day; 3 days to leave with, 2 weeks at home;
  7 days of medicines). Every other number is labelled "Rule of thumb". Setting them writes the goals into the Inventory.

## Training sign-off (Academy)
`core/src/training.rs`. The Family page gives the people. Academy results now carry the person (`academy.csv` gained a
`person` column; the old layout is still read). `user/training.csv` holds which person follows which course and who signed
it off, and when. A lesson counts as learned at 70% or better on its quiz; a lesson without a quiz is shown but not
counted. Each person has a card, a page (lessons learned and still to learn per course), a sign-off box and the next
courses to take: finish a course already started, then the next in the usual order. If the Family page is locked the
names are hidden until its passphrase is typed there (it is not kept).

## Seed vault (Plan > Seed vault)
* `core/src/seeds.rs`: the seed inventory (`user/seeds.csv`), written with `write_atomic`; a damaged file is an error and
  is never overwritten. Crop, variety, type, packets, seeds per packet, year packed, last germination test, where kept,
  when to plant, days to harvest, a goal and notes. Each seed shows **Good**, **Check** or **Replace** from its age
  against a rule-of-thumb life for the crop (1 to 5 years), its last test, and whether packets are below the goal.
* Guides: `content/seed-guides/*.md` (12, kinds buy, store, grow, save, each with a `Source:` line). Your own go in
  `user/seed-guides` (Settings > Your own cards and plans), with a template and how-to.
* Academy course `seed-vault` (3 modules, 9 lessons, quizzes) and the plan **Build a seed vault**.
* Easier to find: Plan tabs are named Get ready, Plans, Drills, Family, Binder, Seed vault, What fails next; the Dashboard
  has a Seed vault tile; the Seed vault page links to the course.

## Ten more medical cards (2026-10-22)
Overdose and naloxone, electric shock, knocked-out tooth, cut-off finger or limb, nosebleed, sepsis, fainting, fever in a child,
severe belly pain, object stuck in a wound. Checked against the live pages on the day: Mayo Clinic (electric shock), University of
Michigan Health (knocked-out tooth, amputation), Kane County Health Department (overdose), CDC (sepsis symptoms), Healthline
(nosebleed). **Written from standard guidance and not yet read on the live page:** fainting, fever in a child, belly pain,
object in a wound, and the 911 criteria on the sepsis card. These need the first check by a person.

## Skills cards (2026-10-22)
22 cards of kind `skills` (new group in Library > Emergency cards): lost (S.T.O.P.), reading a map, compass bearings, direction
without a compass, pace count, finding water, making water safe, shelter, fire, signalling, radio calls, knots, cold, heat, river
crossing, weather signs, toilet and hygiene, wild food, wound care, moving as a group, staying calm, the ten essentials.
They come from general field guidance (US Army field survival manuals and the Ranger Handbook, CDC, NWS, National Park Service, Leave
No Trace) and were **not checked line by line against those sources** in this session. Each says so in its banner. They do not need
a 911 line (the test checks that only for medical, disaster and home cards).
