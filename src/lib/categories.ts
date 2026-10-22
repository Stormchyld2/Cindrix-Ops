// The kinds of supplies in the Inventory. The first eight are the original list and keep their order.
// "Water" and "Food" are special: the Dashboard and "What fails next" count their days from them.
export const categories = [
  'Water',
  'Food',
  'Medical and diabetes',
  'Power and light',
  'Tools and repair',
  'Shelter and clothing',
  'Hygiene and sanitation',
  'Documents and cash',
  // added later
  'Cooking and fuel',
  'Communication and navigation',
  'Safety and protection',
  'Babies and children',
  'Pets and animals',
  'Vehicle and go-bag',
  'Comfort and morale',
  // added 2026-10-22
  'Fuel and heat',
  'Cleaning and laundry',
  'Garden and long-term food',
  'Medical equipment and special needs',
  'Fishing and outdoors',
] as const;

/** A short example for each, shown as a hint when an item is added. */
export const categoryHint: Record<string, string> = {
  'Water': 'bottles, jugs, filters, purification tablets',
  'Food': 'canned and dry food, a manual can opener, spices',
  'Medical and diabetes': 'first aid kit, prescriptions, test strips, glasses',
  'Power and light': 'flashlights, batteries, power banks, solar charger',
  'Tools and repair': 'knife, multitool, duct tape, rope, tarp',
  'Shelter and clothing': 'blankets, sleeping bags, rain gear, boots, socks',
  'Hygiene and sanitation': 'soap, toilet paper, bags, bleach, sanitizer',
  'Documents and cash': 'copies of papers in a waterproof bag, small bills',
  'Cooking and fuel': 'camp stove, propane, matches, lighters, pots',
  'Communication and navigation': 'radio, whistle, paper maps, compass, walkie-talkies',
  'Safety and protection': 'smoke and carbon monoxide alarms, fire extinguisher, dust masks, gloves, goggles',
  'Babies and children': 'formula, diapers, wipes, games for the kids',
  'Pets and animals': 'pet food, water, leash, carrier, vaccination papers',
  'Vehicle and go-bag': 'spare keys, jumper cables, car kit, the grab-and-go bag',
  'Comfort and morale': 'books, cards, a favourite snack, a toy',
  'Fuel and heat': 'gasoline and generator fuel, propane, firewood, a safe heater, extra blankets',
  'Cleaning and laundry': 'detergent, bleach, buckets, scrub brushes, a clothesline, mop',
  'Garden and long-term food': 'seeds, canning jars and lids, grow tools, fertilizer, grain and beans in bulk',
  'Medical equipment and special needs': 'oxygen, a walker or cane, hearing-aid batteries, a backup battery for a medical device (counted only)',
  'Fishing and outdoors': 'fishing line and hooks, a compass, a good knife, a rain poncho, a day pack',
};

/** Units that are usually used for each kind, offered while typing. */
export const categoryUnits: Record<string, string[]> = {
  'Water': ['gal', 'bottles', 'liters', 'jugs'],
  'Food': ['meals', 'cans', 'lb', 'packs', 'servings'],
  'Medical and diabetes': ['days', 'pcs', 'boxes', 'tablets'],
  'Power and light': ['pcs', 'sets', 'packs'],
  'Tools and repair': ['pcs', 'rolls', 'ft'],
  'Shelter and clothing': ['pcs', 'pairs', 'sets'],
  'Hygiene and sanitation': ['rolls', 'bars', 'bottles', 'pcs'],
  'Documents and cash': ['sets', 'envelopes', '$'],
  'Cooking and fuel': ['pcs', 'canisters', 'gal', 'boxes'],
  'Communication and navigation': ['pcs', 'sets'],
  'Safety and protection': ['pcs', 'pairs', 'boxes'],
  'Babies and children': ['days', 'pcs', 'packs'],
  'Pets and animals': ['days', 'lb', 'pcs'],
  'Vehicle and go-bag': ['bags', 'pcs', 'sets'],
  'Comfort and morale': ['sets', 'pcs'],
  'Fuel and heat': ['gal', 'lb', 'cords', 'pcs'],
  'Cleaning and laundry': ['bottles', 'sets', 'pcs'],
  'Garden and long-term food': ['packs', 'jars', 'lb', 'pcs'],
  'Medical equipment and special needs': ['pcs', 'sets', 'days'],
  'Fishing and outdoors': ['pcs', 'sets'],
};

/** Things worth writing down for each kind, shown beside the form so the entry is complete. */
export const categoryAsk: Record<string, string[]> = {
  'Water': ['Is it sealed and store-bought, or did you fill it yourself? Water you fill yourself should be replaced about every six months.', 'Write the size of one jug or bottle in “Size of one”.'],
  'Food': ['Put the date printed on the package in “Use by”.', 'Count meals if you can: one meal feeds one person once. It makes “days left” true.', 'Note anything that needs a lot of water or fuel to cook.'],
  'Medical and diabetes': ['Count only. Never write doses here.', 'Put the refill or expiry date in “Use by”. Say who it is for.', 'Keep a note of where the pharmacy and doctor are on the Family page.'],
  'Power and light': ['What size batteries does it take? Put that in “Brand or type”.', 'For a power bank, write its size in “Size of one” (for example 10,000 mAh).'],
  'Tools and repair': ['Write the model if it is a power tool, and what fuel or battery it takes.', 'Condition tells you what to fix before you need it.'],
  'Shelter and clothing': ['Size and who it fits matter more than the count. Write them in “Who it is for”.', 'For a sleeping bag, write the lowest temperature it is rated for in “Size of one”.'],
  'Hygiene and sanitation': ['Count rolls, bars or bottles. Daily use makes “days left” true.'],
  'Documents and cash': ['Do not write account numbers here. Say what the paper is and where the copy is.', 'Small bills are best. Count them in dollars.'],
  'Cooking and fuel': ['For fuel, write how long one canister really lasted when you tested it.', 'Note whether it is safe indoors. Almost nothing that burns is.'],
  'Communication and navigation': ['Write the radio’s power type: battery, hand-crank or solar.', 'Write the frequencies or channels you agreed with family in the notes.'],
  'Safety and protection': ['Smoke and carbon monoxide alarms have a replace-by date printed on them. Put it in “Use by”.', 'Fire extinguishers need a yearly check. Put the date in “Last checked”.'],
  'Babies and children': ['Say whose it is, and the size (diapers and clothes go out of date as children grow).'],
  'Pets and animals': ['Say which pet it is for. Keep vaccination papers with the Documents.'],
  'Vehicle and go-bag': ['Say which vehicle or whose bag it is. Check the go-bag twice a year.'],
  'Comfort and morale': ['Anything that helps someone stay calm. Quantity matters little.'],
  'Fuel and heat': ['Store fuel outside living space, in approved containers, away from flame.', 'Note how long it lasts at your normal use.'],
  'Cleaning and laundry': ['Never mix bleach with ammonia or other cleaners.'],
  'Garden and long-term food': ['Seeds have a best-before year. Put it in “Use by”.', 'Note what grows in your area and the season to plant.'],
  'Medical equipment and special needs': ['Count only. Note the backup power it needs and how long a battery lasts.', 'Say who it is for.'],
  'Fishing and outdoors': ['Note rules for your area (licences, seasons).'],
};

/** Who an item is for. A name from the Family page can be typed too. */
export const forWhomChoices = ['Everyone', 'Adults', 'Children', 'Baby', 'Pets'];
export const conditionChoices = ['', 'New', 'Good', 'Worn', 'Needs repair'];
