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
};
