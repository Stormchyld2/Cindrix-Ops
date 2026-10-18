//! Medication safety check.
//!
//! Rule: the assistant never gives insulin or other medication doses, and never
//! advises changing a dose. This runs BEFORE the model sees a message, and again
//! on the model's output. Wording of the fixed reply is a draft pending clinical review.

/// Fixed reply used instead of a model answer.
pub const FIXED_REPLY: &str = "I can't give insulin or medication amounts, or tell you to change a dose. \
Use your doctor's written plan. If you feel unwell, confused, or can't keep fluids down, call 911. \
Here is the Low blood sugar card.";

/// Card shown with the fixed reply.
pub const FIXED_REPLY_CARD: &str = "low-blood-sugar";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Refuse(&'static str),
}

const MEDICINES: &[&str] = &[
    "insulin", "humalog", "novolog", "lantus", "toujeo", "basaglar", "levemir", "tresiba", "fiasp",
    "apidra", "humulin", "novolin", "admelog", "lyumjev", "glucagon", "baqsimi", "gvoke", "metformin",
    "glipizide", "glyburide", "glimepiride", "januvia", "sitagliptin", "jardiance", "farxiga", "ozempic",
    "wegovy", "mounjaro", "trulicity", "victoza", "semaglutide", "aspirin", "acetaminophen", "tylenol",
    "paracetamol", "ibuprofen", "advil", "motrin", "naproxen", "aleve", "nitroglycerin", "warfarin",
    "coumadin", "prednisone", "lisinopril", "amoxicillin", "antibiotic", "antibiotics", "epinephrine",
    "epipen", "benadryl", "diphenhydramine", "morphine", "oxycodone", "codeine", "opioid", "sedative",
    "medication", "medications", "medicine", "medicines", "meds", "pill", "pills", "tablet", "tablets",
    "pen", "pens", "vial", "vials", "syringe", "syringes", "injection", "inject", "injecting",
];

/// Words that signal a question about amount, timing or changing a dose.
const DOSING: &[&str] = &[
    "dose", "doses", "dosing", "dosage", "overdose", "mg", "mcg", "milligram", "milligrams", "ml", "cc",
    "amount", "amounts", "sliding", "ratio", "ratios", "bolus", "basal", "titrate", "correction",
    "extra", "double", "triple", "skip", "increase", "decrease", "reduce", "raise", "lower", "adjust",
    "convert", "conversion", "maximum", "minimum", "max", "often", "frequently", "iu",
];

/// Phrases (already normalized: lowercase words separated by one space).
const DOSING_PHRASES: &[&str] = &[
    "how much", "how many", "how often", "how long between", "cut back", "take more", "take less",
    "per carb", "insulin to carb", "carb ratio", "sliding scale", "at once", "once a day", "twice a day",
    "how fast", "give myself", "give him", "give her", "give them", "take a",
];

const UNIT_CONTEXT: &[&str] = &[
    "how many", "number of", "take", "give", "inject", "need", "use", "dose", "much", "tell", "more",
    "less", "extra", "of",
];

fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = true;
    for ch in s.chars() {
        let c = if ch.is_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' };
        let c = if ch == '\u{2019}' { ' ' } else { c };
        if c == ' ' {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn words(norm: &str) -> Vec<&str> {
    norm.split(' ').filter(|w| !w.is_empty()).collect()
}

fn has_word(ws: &[&str], list: &[&str]) -> bool {
    ws.iter().any(|w| list.contains(w))
}

fn has_phrase(norm: &str, phrases: &[&str]) -> bool {
    let padded = format!(" {} ", norm);
    phrases.iter().any(|p| padded.contains(&format!(" {} ", p)))
}

/// Squashed text catches spaced-out spellings like "i n s u l i n".
fn mentions_medicine(norm: &str, ws: &[&str]) -> bool {
    if has_word(ws, MEDICINES) {
        return true;
    }
    let squashed: String = norm.chars().filter(|c| *c != ' ').collect();
    ["insulin", "glucagon", "metformin", "humalog", "lantus"]
        .iter()
        .any(|m| squashed.contains(m))
}

fn is_number(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_ascii_digit())
}

/// "units" used as an amount: a number nearby, or asked about ("how many units").
fn unit_amount(norm: &str, ws: &[&str]) -> bool {
    for (i, w) in ws.iter().enumerate() {
        if *w == "units" || *w == "unit" {
            let near_number = i > 0 && (is_number(ws[i - 1]) || (i > 1 && is_number(ws[i - 2])));
            if near_number {
                return true;
            }
            if has_phrase(norm, UNIT_CONTEXT) || has_word(ws, UNIT_CONTEXT) {
                // "units of water" etc. is rare; we accept over-refusal here on purpose.
                return true;
            }
        }
    }
    false
}

/// Check a user's message before the model sees it.
pub fn check_message(message: &str) -> Verdict {
    let norm = normalize(message);
    let ws = words(&norm);
    if unit_amount(&norm, &ws) {
        return Verdict::Refuse("unit amount");
    }
    if mentions_medicine(&norm, &ws) {
        if has_word(&ws, DOSING) || has_phrase(&norm, DOSING_PHRASES) {
            return Verdict::Refuse("medicine with dosing words");
        }
    }
    Verdict::Allow
}

/// Check the model's answer. True if it appears to contain a medication dose.
pub fn output_contains_dose(answer: &str) -> bool {
    let norm = normalize(answer);
    let ws = words(&norm);
    if !mentions_medicine(&norm, &ws) && !ws.iter().any(|w| *w == "units" || *w == "unit") {
        return false;
    }
    for (i, w) in ws.iter().enumerate() {
        if i == 0 || !is_number(ws[i - 1]) {
            continue;
        }
        match *w {
            "units" | "unit" | "iu" | "mcg" => return true,
            "mg" => {
                // "70 mg/dL" is a blood sugar reading, not a dose.
                if ws.get(i + 1) != Some(&"dl") {
                    return true;
                }
            }
            "ml" | "cc" => return true,
            _ => {}
        }
    }
    false
}

/// What to show for a message: Some(fixed reply) if refused.
pub fn gate(message: &str) -> Option<&'static str> {
    match check_message(message) {
        Verdict::Refuse(_) => Some(FIXED_REPLY),
        Verdict::Allow => None,
    }
}

/// Run the output check; replace a bad answer with the fixed reply.
pub fn filter_output(answer: String) -> String {
    if output_contains_dose(&answer) {
        FIXED_REPLY.to_string()
    } else {
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_obvious_dose_question() {
        assert!(matches!(check_message("How many units of insulin should I take?"), Verdict::Refuse(_)));
    }

    #[test]
    fn allows_storage_question() {
        assert_eq!(check_message("How do I keep insulin cool without a refrigerator?"), Verdict::Allow);
    }

    #[test]
    fn catches_spaced_out_spelling() {
        assert!(matches!(check_message("how much i n s u l i n do i give"), Verdict::Refuse(_)));
    }

    #[test]
    fn glucose_reading_is_not_a_dose() {
        assert!(!output_contains_dose("Treat if your insulin level check shows below 70 mg/dL."));
    }

    #[test]
    fn output_with_units_is_blocked() {
        assert!(output_contains_dose("Take 10 units of insulin now."));
        assert_eq!(filter_output("Take 10 units of insulin now.".into()), FIXED_REPLY);
    }

    #[test]
    fn normal_first_aid_output_passes() {
        let a = "Take about 15 grams of fast-acting sugar, wait 15 minutes, and recheck.".to_string();
        assert_eq!(filter_output(a.clone()), a);
    }
}
