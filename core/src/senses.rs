//! Word sense guardrails for the assistant's search. Many everyday words mean different things
//! ("dress a cut", "dress for the cold", "dress a board"; "charge a phone", "charge in court").
//! Each rule here reads the rest of the question to decide which meaning is meant, then
//! (1) rewrites the search words to the right meaning, and
//! (2) marks passages that are plainly about the wrong meaning so they rank low.
//! A rule only fires when the question itself gives the context, so a question with no clue
//! is searched exactly as written.

use crate::retrieval::tokens;

struct Rule {
    name: &'static str,
    /// The ambiguous word(s) in the question.
    word: &'static [&'static str],
    /// Words elsewhere in the question that fix the meaning.
    context: &'static [&'static str],
    /// Search words to add when the rule fires (the right meaning's usual words).
    add: &'static [&'static str],
    /// Drop the ambiguous word from the search (it only matters for the wrong meaning).
    drop_word: bool,
    /// A passage with any of these, and none of `support`, is about the wrong meaning.
    wrong: &'static [&'static str],
    support: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        name: "wound care",
        word: &["dress", "dressed", "dressing", "wrap", "cover", "clean"],
        context: &["cut", "gash", "scrape", "laceration", "wound", "graze", "slice", "bleeding", "bleed", "stitch"],
        add: &["wound", "dressing", "bandage", "clean"],
        drop_word: true,
        wrong: &["saw", "jointer", "lumber", "plywood", "glue", "woodworking", "dulcimer", "timber", "sewing", "skirt", "gown", "hem", "recipe", "salad"],
        support: &["wound", "bandage", "bleeding", "infection", "gauze", "antiseptic", "bleed"],
    },
    Rule {
        name: "clothing for weather",
        word: &["dress", "dressed", "dressing"],
        context: &["cold", "winter", "layer", "rain", "snow", "hypothermia", "warm", "freezing", "weather", "hike"],
        add: &["clothing", "layers", "insulation", "warm"],
        drop_word: true,
        wrong: &["saw", "jointer", "lumber", "plywood", "glue", "dulcimer", "wedding", "gown", "bandage", "wound", "salad"],
        support: &["clothing", "layer", "insulation", "hypothermia", "wool", "coat", "jacket"],
    },
    Rule {
        name: "burn injury",
        word: &["burn", "burned", "burnt"],
        context: &["skin", "blister", "scald", "hand", "arm", "finger", "child", "baby", "boiling", "stove", "sunburn", "hot"],
        add: &["burn", "scald", "cool", "blister"],
        drop_word: false,
        wrong: &["calorie", "dvd", "disc", "firewood", "kindling", "charcoal", "cd"],
        support: &["blister", "skin", "scald", "cool", "tissue", "injury"],
    },
    Rule {
        name: "electric shock",
        word: &["shock", "shocked"],
        context: &["electric", "electrical", "wire", "outlet", "lightning", "power", "plug", "socket", "generator"],
        add: &["electric", "current", "unplug"],
        drop_word: false,
        wrong: &["absorber", "market", "economy", "price", "aftershock"],
        support: &["electric", "current", "voltage", "wire", "breaker"],
    },
    Rule {
        name: "charging a battery",
        word: &["charge", "charged", "charging"],
        context: &["battery", "phone", "solar", "radio", "usb", "device", "flashlight", "laptop", "power", "bank"],
        add: &["battery", "charging", "power"],
        drop_word: false,
        wrong: &["criminal", "court", "tuition", "lawsuit", "indictment", "fee", "arrest"],
        support: &["battery", "volt", "solar", "usb", "power", "watt"],
    },
    Rule {
        name: "food stores",
        word: &["stock", "stocks", "stocking", "stockpile"],
        context: &["food", "water", "pantry", "supply", "supplies", "prepper", "emergency", "month", "year", "person", "family"],
        add: &["store", "storage", "supply", "pantry"],
        drop_word: false,
        wrong: &["market", "share", "investor", "dividend", "broth", "bond", "nasdaq"],
        support: &["storage", "supply", "pantry", "rotate", "store", "emergency"],
    },
    Rule {
        name: "cold illness",
        word: &["cold", "colds"],
        context: &["cough", "runny", "congestion", "sneeze", "sneezing", "throat", "flu", "symptom", "nose", "tea", "remedy", "catch"],
        add: &["cough", "congestion", "throat", "rest"],
        drop_word: false,
        wrong: &["freezer", "fridge", "frostbite", "hypothermia", "storm", "blizzard"],
        support: &["cough", "congestion", "throat", "flu", "sneeze", "runny"],
    },
    Rule {
        name: "cold exposure",
        word: &["cold"],
        context: &["exposure", "shiver", "shivering", "freezing", "frostbite", "outside", "tent", "hypothermia", "winter", "snow", "night", "blanket"],
        add: &["hypothermia", "warm", "insulation", "shelter"],
        drop_word: false,
        wrong: &["cough", "congestion", "runny", "sneeze", "throat"],
        support: &["hypothermia", "frostbite", "insulation", "shelter", "warm", "shiver"],
    },
    Rule {
        name: "muscle strain",
        word: &["strain", "strained", "pulled"],
        context: &["muscle", "ankle", "back", "knee", "wrist", "sprain", "lifting", "swollen", "swelling"],
        add: &["sprain", "muscle", "swelling", "rest"],
        drop_word: false,
        wrong: &["filter", "sieve", "cheesecloth", "broth", "strainer"],
        support: &["sprain", "muscle", "swelling", "ligament", "rest", "splint"],
    },
    Rule {
        name: "straining a liquid",
        word: &["strain", "strained"],
        context: &["tea", "broth", "liquid", "cloth", "cheesecloth", "herb", "tincture", "juice", "water"],
        add: &["filter", "cloth", "sieve"],
        drop_word: false,
        wrong: &["ligament", "sprain", "muscle", "ankle"],
        support: &["filter", "cloth", "sieve", "cheesecloth", "liquid", "steep"],
    },
    Rule {
        name: "blood pressure",
        word: &["pressure"],
        context: &["blood", "heart", "dizzy", "hypertension", "headache", "high", "low", "salt"],
        add: &["blood", "hypertension", "heart"],
        drop_word: false,
        wrong: &["canner", "cooker", "psi", "gauge", "regulator", "tire"],
        support: &["blood", "hypertension", "heart", "dizzy", "systolic"],
    },
    Rule {
        name: "pressure canning",
        word: &["pressure"],
        context: &["canner", "canning", "jar", "jars", "cooker", "psi", "preserve", "beans"],
        add: &["canner", "canning", "psi"],
        drop_word: false,
        wrong: &["blood", "hypertension", "systolic", "dizzy"],
        support: &["canner", "canning", "psi", "jar", "processing"],
    },
    Rule {
        name: "stroke",
        word: &["stroke"],
        context: &["face", "arm", "speech", "numb", "sudden", "droop", "drooping", "slurred", "confusion", "brain"],
        add: &["stroke", "face", "speech", "brain"],
        drop_word: false,
        wrong: &["swim", "freestyle", "backstroke", "golf", "brush", "paddle", "piston", "engine"],
        support: &["face", "speech", "brain", "droop", "numb"],
    },
    Rule {
        name: "garden plants",
        word: &["plant", "plants", "planting"],
        context: &["garden", "grow", "seed", "soil", "harvest", "bed", "spring", "vegetable", "frost", "water"],
        add: &["grow", "garden", "seed"],
        drop_word: false,
        wrong: &["factory", "reactor", "manufacturing", "turbine", "refinery", "nuclear"],
        support: &["grow", "garden", "seed", "soil", "harvest"],
    },
    Rule {
        name: "practice drill",
        word: &["drill", "drills"],
        context: &["family", "practice", "evacuation", "fire", "earthquake", "rehearse", "school", "kids", "children", "exercise"],
        add: &["practice", "rehearse", "exercise"],
        drop_word: false,
        wrong: &["bit", "chuck", "torque", "cordless", "masonry", "bore"],
        support: &["practice", "rehearse", "evacuation", "family", "exercise"],
    },
    Rule {
        name: "bug-out",
        word: &["bug"],
        context: &["out", "bag", "evacuate", "leave", "plan", "route", "vehicle", "go"],
        add: &["evacuate", "bag", "plan", "route"],
        drop_word: false,
        wrong: &["mosquito", "beetle", "spray", "repellent", "software", "debug"],
        support: &["evacuate", "bag", "route", "plan", "leave"],
    },
];

fn folded(list: &[&str]) -> Vec<String> {
    list.iter().flat_map(|w| tokens(w)).collect()
}

fn has_any(words: &[String], list: &[&str]) -> bool {
    let l = folded(list);
    words.iter().any(|w| l.contains(w))
}

/// Rules that fire for these question words.
fn fired<'a>(q: &[String]) -> Vec<&'a Rule> {
    RULES.iter().filter(|r| has_any(q, r.word) && has_any(q, r.context)).collect()
}

/// Rewrite the search words of a question to the meaning its context points to.
pub fn expand(mut q: Vec<String>) -> Vec<String> {
    for r in fired(&q.clone()) {
        if r.drop_word {
            let drop = folded(r.word);
            q.retain(|w| !drop.contains(w));
        }
        for a in r.add {
            for t in tokens(a) {
                q.push(t);
            }
        }
    }
    let mut seen = Vec::new();
    q.retain(|w| {
        let new = !seen.contains(w);
        seen.push(w.clone());
        new
    });
    q
}

/// Multiplier (1.0 = no change) for a passage: well under 1 when the question's meaning is
/// settled by a rule and the passage is plainly about another meaning, and when it matches
/// too few of the question's words to be an answer.
pub fn weight(query: &[String], passage: &[String]) -> f64 {
    let mut m = 1.0;
    for r in RULES {
        // the rule's meaning is recognised by the words `expand` leaves in the query
        let add = folded(r.add);
        let settled = add.iter().filter(|a| query.contains(a)).count() >= 2 && has_any(query, r.context);
        if !settled {
            continue;
        }
        let wrong = folded(r.wrong);
        let support = folded(r.support);
        if passage.iter().any(|w| wrong.contains(w)) && !passage.iter().any(|w| support.contains(w)) {
            m *= 0.2;
        }
    }
    // Coverage: with four or more different question words, a passage that holds only one
    // of them (a stray word) is a weak match for the whole question.
    let mut distinct: Vec<&String> = Vec::new();
    for w in query {
        if !distinct.contains(&w) {
            distinct.push(w);
        }
    }
    if distinct.len() >= 4 {
        let hit = distinct.iter().filter(|w| passage.contains(**w)).count();
        if hit <= 1 {
            m *= 0.5;
        }
    }
    m
}

pub fn rule_names() -> Vec<&'static str> {
    RULES.iter().map(|r| r.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> Vec<String> {
        expand(tokens(s))
    }

    #[test]
    fn the_context_decides_the_meaning() {
        assert!(q("how do i dress a cut").contains(&"bandage".to_string()));
        assert!(!q("how do i dress a cut").contains(&"dress".to_string()));
        assert!(q("how do i dress for the cold").contains(&"clothing".to_string()));
        assert!(q("how do i charge a phone with solar").contains(&"battery".to_string()));
        assert!(q("how much food should i stock for a family").contains(&"storage".to_string()));
    }

    #[test]
    fn more_words_follow_their_context() {
        assert!(q("my child has a cold with a cough").contains(&"congestion".to_string()));
        assert!(q("i am cold and shivering outside at night").contains(&"hypothermia".to_string()));
        assert!(q("i strained my ankle lifting").contains(&"sprain".to_string()));
        assert!(q("how do i strain tea through cloth").contains(&"filter".to_string()));
        assert!(q("my blood pressure is high and i am dizzy").contains(&"hypertension".to_string()));
        assert!(q("what psi for a pressure canner with jars").contains(&"canning".to_string()));
        assert!(q("sudden numb face and slurred speech stroke").contains(&"brain".to_string()));
        assert!(q("when should i plant seed in the garden").contains(&"grow".to_string()));
        assert!(q("how do we practice a family fire drill").contains(&"rehearse".to_string()));
        assert!(q("when do we bug out and which route").contains(&"evacuate".to_string()));
    }

    #[test]
    fn no_clue_means_no_change() {
        assert_eq!(q("how do i dress a dulcimer"), tokens("how do i dress a dulcimer"));
        assert_eq!(q("what is a stock split"), tokens("what is a stock split"));
    }

    #[test]
    fn the_wrong_meaning_is_pushed_down() {
        let query = q("how do i dress a cut");
        let wood = tokens("dress this piece on a jointer, cut it with a bench saw and glue");
        let care = tokens("clean the wound and cover with a clean dressing bandage");
        assert!(weight(&query, &wood) < 0.3);
        assert!(weight(&query, &care) >= 1.0);
    }

    #[test]
    fn a_passage_with_one_stray_word_is_a_weak_match() {
        let query = tokens("how long does stored flour keep in a sealed barrel");
        assert!(weight(&query, &tokens("barrel maker history")) < 1.0);
        assert_eq!(weight(&query, &tokens("stored flour keeps in a sealed barrel")), 1.0);
    }

    #[test]
    fn every_rule_has_words_to_work_with() {
        for r in RULES {
            assert!(!r.word.is_empty() && !r.context.is_empty() && !r.add.is_empty() && !r.wrong.is_empty() && !r.support.is_empty(), "{}", r.name);
        }
        assert_eq!(rule_names().len(), RULES.len());
    }
}
