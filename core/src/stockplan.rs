//! "How much should I have?": the supplies a household needs for a number of days, worked out from
//! how many people and pets there are. The answer becomes the targets in the Inventory.
//!
//! Where a published figure exists it is named in `basis`: the Red Cross asks for 1 gallon of water per
//! person per day, a 3-day supply to leave with and a 2-week supply for home, and a 7-day supply of
//! medicines; Ready.gov asks for water and food for "several days". Everything else is a plain rule of
//! thumb and is labelled that way, so nobody mistakes a guess for a rule.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Household {
    pub adults: u32,
    pub children: u32,
    pub babies: u32,
    pub small_pets: u32,
    pub large_pets: u32,
    pub days: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Need {
    pub category: &'static str,
    pub name: String,
    pub unit: &'static str,
    pub amount: f64,
    pub why: String,
    /// "Red Cross", "Ready.gov" or "Rule of thumb"
    pub basis: &'static str,
}

fn n(category: &'static str, name: &str, unit: &'static str, amount: f64, basis: &'static str, why: &str) -> Need {
    Need { category, name: name.to_string(), unit, amount, why: why.to_string(), basis }
}

fn up(x: f64) -> f64 {
    x.ceil()
}

pub fn needs(h: &Household) -> Vec<Need> {
    let days = h.days.clamp(1, 365) as f64;
    let (a, c, b) = (h.adults as f64, h.children as f64, h.babies as f64);
    let people = a + c + b;
    let eaters = a + c; // babies are fed formula or baby food, counted below
    let mut v: Vec<Need> = Vec::new();
    if people == 0.0 {
        return v;
    }
    const RC: &str = "Red Cross";
    const RD: &str = "Ready.gov";
    const RT: &str = "Rule of thumb";

    // Water
    v.push(n("Water", "Drinking and cooking water", "gal", people * days, RC, "1 gallon per person per day, for drinking, cooking and washing. More for hot weather, illness or pregnancy."));
    let pet_water = h.small_pets as f64 * 0.25 + h.large_pets as f64 * 1.0;
    if pet_water > 0.0 {
        v.push(n("Pets and animals", "Water for pets", "gal", up(pet_water * days), RT, "About a quarter gallon a day for a small pet and a gallon for a large dog."));
    }
    v.push(n("Water", "Water filter", "pcs", 1.0, RD, "Removes germs from water you find. Add a few drops of plain bleach or a purification tablet after filtering."));
    v.push(n("Water", "Unscented household bleach", "bottle", 1.0, RD, "For making water safe. Check the label says 5% to 9% sodium hypochlorite."));
    v.push(n("Water", "Water purification tablets", "pack", 1.0, RT, "A light backup for the go-bag, used as the pack says."));

    // Food
    if eaters > 0.0 {
        v.push(n("Food", "Food that keeps (meals)", "meals", eaters * 3.0 * days, RC, "Three meals a day for each adult and child, of food that needs no fridge and little cooking. Plan on about 2,000 calories a day for an adult; children need less."));
        v.push(n("Food", "Manual can opener", "pcs", 1.0, RD, "Cans are no use without one. Keep a spare."));
    }
    if h.babies > 0 {
        v.push(n("Babies and children", "Formula or baby food", "days", days, RC, "Count what your baby uses in a normal week and divide by seven to see how many days one pack lasts. Ask your child's doctor how to feed the baby if the water is not safe."));
        v.push(n("Babies and children", "Diapers", "pcs", up(b * 8.0 * days), RT, "About eight a day for a baby."));
        v.push(n("Babies and children", "Baby wipes", "pack", up(b * days / 7.0).max(1.0), RT, "About a pack a week for each baby."));
    }
    if h.children + h.babies > 0 {
        v.push(n("Comfort and morale", "Comfort items for children", "pcs", c + b, RT, "A toy, a blanket or a book each. It helps them stay calm."));
    }
    if h.small_pets + h.large_pets > 0 {
        v.push(n("Pets and animals", "Pet food", "days", days, RC, "Count what the pet eats in a week and divide by seven. Keep it sealed and swap it out before its date."));
        v.push(n("Pets and animals", "Pet carrier or leash", "pcs", (h.small_pets + h.large_pets) as f64, RC, "One for each pet, so you can leave quickly."));
    }

    // Medical
    v.push(n("Medical and diabetes", "Prescription medicines (count only)", "days", 7.0_f64.max(days.min(30.0)), RC, "At least a 7-day supply. Ask your doctor or pharmacist about an extra supply. This list only counts; it never says how much to take."));
    v.push(n("Medical and diabetes", "First aid kit", "pcs", 1.0, RD, "One for the home and one for each car."));
    v.push(n("Medical and diabetes", "Spare glasses or contact lenses", "pairs", 1.0, RT, "For each person who wears them."));

    // Power and light
    v.push(n("Power and light", "Flashlights or headlamps", "pcs", eaters.max(1.0), RT, "One for each person old enough to carry one. A headlamp keeps both hands free."));
    v.push(n("Power and light", "Spare batteries (sets)", "sets", eaters.max(1.0) * 2.0, RD, "Two spare sets for each light. Buy the sizes your lights and radio use."));
    v.push(n("Power and light", "Battery or hand-crank radio", "pcs", 1.0, RD, "Add a NOAA Weather Radio if you can."));
    v.push(n("Power and light", "Power bank for phones", "pcs", (a + c).max(1.0), RT, "One for each phone, kept charged."));

    // Shelter, hygiene, cooking
    v.push(n("Shelter and clothing", "Sleeping bag or warm blanket", "pcs", people, RD, "One for each person."));
    v.push(n("Hygiene and sanitation", "Toilet paper", "rolls", up(people * days / 5.0), RT, "About a roll for each person every five days."));
    v.push(n("Hygiene and sanitation", "Heavy garbage bags", "pcs", up(days / 2.0).max(10.0), RD, "Lined buckets make an emergency toilet. Add plastic ties."));
    v.push(n("Hygiene and sanitation", "Soap (bars)", "pcs", up(people * days / 30.0).max(2.0), RT, "Hand washing stops most sickness. Add hand sanitizer."));
    v.push(n("Cooking and fuel", "Camp stove", "pcs", 1.0, RT, "Used outdoors only, never in the house, garage or tent."));
    v.push(n("Cooking and fuel", "Stove fuel (small canisters)", "pcs", up(people * days / 6.0).max(2.0), RT, "A guess. Time how long one canister really lasts on your stove, then change this number."));
    v.push(n("Cooking and fuel", "Matches or lighters", "boxes", 2.0, RD, "Keep them dry in a sealed bag."));

    // Safety
    v.push(n("Safety and protection", "Smoke alarms", "pcs", 1.0, RD, "One on every level and outside every sleeping area. Change the batteries twice a year. Edit the number to match your home."));
    v.push(n("Safety and protection", "Carbon monoxide alarm (battery)", "pcs", 1.0, RD, "Near every sleeping area. Edit the number to match your home."));
    v.push(n("Safety and protection", "Fire extinguisher", "pcs", 1.0, RT, "One on each level, and learn how to use it."));
    v.push(n("Safety and protection", "Dust masks (N95)", "pcs", people * 2.0, RT, "For smoke, dust and cleanup."));
    v.push(n("Safety and protection", "Work gloves", "pairs", (a + c).max(1.0), RT, "For cleaning up debris."));

    // Tools, contact, papers, go-bags
    v.push(n("Tools and repair", "Multitool or knife", "pcs", 1.0, RD, ""));
    v.push(n("Tools and repair", "Wrench or pliers for the gas and water shut-off", "pcs", 1.0, RD, "Learn where the shut-offs are first."));
    v.push(n("Tools and repair", "Duct tape", "rolls", 2.0, RT, ""));
    v.push(n("Communication and navigation", "Whistle", "pcs", eaters.max(1.0), RD, "One for each person old enough to use it, to signal for help."));
    v.push(n("Communication and navigation", "Paper map of the area", "pcs", 1.0, RD, "Mark two ways out of town."));
    v.push(n("Documents and cash", "Copies of papers in a waterproof bag", "sets", 1.0, RD, "Insurance, identification and bank records, on paper and saved on a drive."));
    v.push(n("Vehicle and go-bag", "Go-bag with 3 days of supplies", "bags", people, RC, "A pack for each person with water, food and the basics, ready to grab."));
    v.push(n("Comfort and morale", "Books, cards or games", "sets", 1.0, RT, "Boredom and worry are easier with something to do."));
    v.push(n("Cleaning and laundry", "Laundry soap and cleaning supplies", "sets", up(days / 30.0).max(1.0), RT, "A set for each month: soap, a bucket, scrub brush and a clothesline."));
    if days >= 30.0 {
        v.push(n("Garden and long-term food", "Garden seed packets", "packs", 10.0, RT, "For a stay that lasts longer than a month. Choose what grows in your area."));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(a: u32, c: u32, b: u32, sp: u32, lp: u32, days: u32) -> Household {
        Household { adults: a, children: c, babies: b, small_pets: sp, large_pets: lp, days }
    }
    fn find<'a>(v: &'a [Need], name: &str) -> &'a Need {
        v.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("no need called {}", name))
    }

    #[test]
    fn water_and_food_follow_the_red_cross_numbers() {
        let v = needs(&h(2, 2, 0, 0, 0, 14));
        assert_eq!(find(&v, "Drinking and cooking water").amount, 4.0 * 14.0); // 1 gal per person per day
        assert_eq!(find(&v, "Food that keeps (meals)").amount, 4.0 * 3.0 * 14.0);
        assert_eq!(find(&v, "Drinking and cooking water").basis, "Red Cross");
        assert_eq!(find(&v, "Go-bag with 3 days of supplies").amount, 4.0);
    }

    #[test]
    fn babies_and_pets_add_their_own_things_and_others_are_left_out() {
        let none = needs(&h(1, 0, 0, 0, 0, 3));
        assert!(none.iter().all(|x| x.name != "Diapers" && x.name != "Pet food"));
        let v = needs(&h(2, 0, 1, 1, 1, 10));
        assert_eq!(find(&v, "Diapers").amount, 80.0);
        assert_eq!(find(&v, "Water for pets").amount, 13.0); // (0.25 + 1) * 10, rounded up
        assert_eq!(find(&v, "Pet carrier or leash").amount, 2.0);
        assert_eq!(find(&v, "Drinking and cooking water").amount, 30.0); // the baby drinks water too
        assert_eq!(find(&v, "Food that keeps (meals)").amount, 60.0); // the baby has formula, listed apart
    }

    #[test]
    fn nobody_means_nothing_and_every_need_has_a_known_category_and_no_doses() {
        assert!(needs(&h(0, 0, 0, 2, 0, 7)).is_empty());
        let cats = ["Water", "Food", "Medical and diabetes", "Power and light", "Tools and repair", "Shelter and clothing", "Hygiene and sanitation", "Documents and cash", "Cooking and fuel", "Communication and navigation", "Safety and protection", "Babies and children", "Pets and animals", "Vehicle and go-bag", "Comfort and morale", "Cleaning and laundry", "Garden and long-term food"];
        for x in needs(&h(3, 2, 1, 1, 1, 30)) {
            assert!(cats.contains(&x.category), "{} has category {}", x.name, x.category);
            assert!(x.amount > 0.0, "{}", x.name);
            assert!(!crate::safety::output_contains_dose(&format!("{} {}", x.name, x.why)), "{}", x.name);
        }
        // a silly number of days is held to a year
        assert_eq!(find(&needs(&h(1, 0, 0, 0, 0, 5000)), "Drinking and cooking water").amount, 365.0);
    }
}
