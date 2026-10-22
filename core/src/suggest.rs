//! The Dashboard's suggestions: plain-words advice worked out from what the person has really filled
//! in (supplies, family page, drills, maps, assistant, library). No guessing about their situation:
//! every suggestion says what was seen and what to do next. Nothing here gives medical advice.

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Items in the inventory.
    pub items: usize,
    pub water_days: Option<f64>,
    pub food_days: Option<f64>,
    /// Items past their use-by date.
    pub past_date: usize,
    /// Items that reach their date within the next 3 months (not counting those already past).
    pub expiring_soon: usize,
    /// Items with no "used per day" number, which "What fails next" cannot count.
    pub no_daily_use: usize,
    pub family_done: bool,
    pub drills_done: usize,
    pub drills_total: usize,
    pub maps: usize,
    pub models: usize,
    pub assistant_ready: bool,
    pub archives: usize,
    /// The person's own limits (Settings, Plan): under `act_days` is urgent, under `check_days` needs a look.
    pub act_days: f64,
    pub check_days: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Urgent,
    Soon,
    Next,
    Good,
}

impl Level {
    pub fn word(self) -> &'static str {
        match self {
            Level::Urgent => "urgent",
            Level::Soon => "soon",
            Level::Next => "next",
            Level::Good => "good",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub level: Level,
    pub title: String,
    pub text: String,
    /// The screen that helps ("Inventory", "Family", "Drills", "Maps", "Binder", "Settings"), when there is one.
    pub tab: Option<String>,
}

fn days(d: f64) -> String {
    if d < 10.0 { format!("{:.1} days", d) } else { format!("{} days", d.round() as i64) }
}
fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {}", one) } else { format!("{} {}", n, many) }
}
fn s(level: Level, title: &str, text: String, tab: &str) -> Suggestion {
    Suggestion { level, title: title.to_string(), text, tab: if tab.is_empty() { None } else { Some(tab.to_string()) } }
}

/// What to do next, most pressing first. At most `limit`.
pub fn suggest(n: &Snapshot, limit: usize) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    let act = if n.act_days > 0.0 { n.act_days } else { 3.0 };
    let check = if n.check_days > act { n.check_days } else { act * 4.0 };

    if n.items == 0 {
        out.push(s(Level::Soon, "Count your supplies", "Nothing is in the Inventory yet. Start with water, food, batteries and any medical supplies, with how much you use each day. Everything else on this page depends on it.".into(), "Inventory"));
    } else {
        for (what, d) in [("Water", n.water_days), ("Food", n.food_days)] {
            match d {
                Some(d) if d < act => out.push(s(Level::Urgent, &format!("{} lasts only {}", what, days(d)), format!("That is under your {}-day limit. Add more {} first.", act.round() as i64, what.to_lowercase()), "Inventory")),
                Some(d) if d < check => out.push(s(Level::Soon, &format!("{} lasts {}", what, days(d)), format!("A common goal is two weeks at home (for water, about a gallon per person per day). Your own \"look at it\" limit is {} days.", check.round() as i64), "Inventory")),
                None => out.push(s(Level::Next, &format!("Add how fast you use {}", what.to_lowercase()), format!("No {} item has a \"used per day\" number, so the days left cannot be worked out.", what.to_lowercase()), "Inventory")),
                _ => {}
            }
        }
        if n.past_date > 0 {
            out.push(s(Level::Urgent, &format!("{} past {}", plural(n.past_date, "item is", "items are"), "their date"), "Use them first, or replace them, and take them off the list.".into(), "Inventory"));
        }
        if n.expiring_soon > 0 {
            out.push(s(Level::Soon, &format!("{} within 3 months", plural(n.expiring_soon, "item reaches its date", "items reach their dates")), "Use the oldest first and buy new ones so nothing goes to waste.".into(), "Inventory"));
        }
        if n.no_daily_use > 0 && n.water_days.is_some() {
            out.push(s(Level::Next, &format!("{} no daily use", plural(n.no_daily_use, "item has", "items have")), "Fill in \"used per day\" so What fails next can count them.".into(), "Inventory"));
        }
    }
    if !n.family_done {
        out.push(s(Level::Soon, "Fill in the Family page", "Names, medicines as your doctor wrote them, allergies and who to call. It stays on this device (and can be locked with a passphrase).".into(), "Family"));
    }
    if n.drills_total > 0 && n.drills_done == 0 {
        out.push(s(Level::Next, "Run your first drill", "A drill finds the gaps while everything still works. The 72-hour power outage drill is a good first one.".into(), "Drills"));
    }
    if n.maps == 0 {
        out.push(s(Level::Next, "Put a map on the drive", "No map files were found. Run scripts\\get-maps.ps1 to add the street map, then mark your home and a place to meet.".into(), "Maps"));
    }
    if n.archives == 0 {
        out.push(s(Level::Next, "Add library archives", "No .zim archives were found, so Intel is empty. Copy them into the library folder on the drive.".into(), "Library"));
    }
    if !n.assistant_ready {
        let text = if n.models == 0 { "No model file is in the models folder. Copy a Qwen3.5 .gguf file there and restart. Until then the assistant only finds pages and cards." } else { "A model file is there but it is not running. Open Settings, Assistant." };
        out.push(s(Level::Next, "Get the assistant running", text.into(), "Settings"));
    }
    out.sort_by_key(|x| x.level);
    out.truncate(limit);
    if out.is_empty() {
        out.push(s(Level::Good, "Nothing needs attention", "Your supplies, plan and drive all look fine. A good habit: look at this page once a month and run one drill.".into(), ""));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Snapshot {
        Snapshot { items: 12, water_days: Some(30.0), food_days: Some(40.0), family_done: true, drills_done: 2, drills_total: 6, maps: 1, models: 1, assistant_ready: true, archives: 3, act_days: 3.0, check_days: 14.0, ..Default::default() }
    }

    #[test]
    fn a_ready_person_is_told_nothing_needs_attention() {
        let r = suggest(&ready(), 6);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].level, Level::Good);
    }

    #[test]
    fn an_empty_start_asks_for_the_basics_in_order() {
        let r = suggest(&Snapshot { drills_total: 6, act_days: 3.0, check_days: 14.0, ..Default::default() }, 10);
        let titles: Vec<&str> = r.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles[0], "Count your supplies");
        assert!(titles.contains(&"Fill in the Family page"));
        assert!(titles.contains(&"Run your first drill"));
        assert!(titles.contains(&"Put a map on the drive"));
        // soon comes before next
        let soon = r.iter().position(|x| x.level == Level::Next).unwrap();
        assert!(r[..soon].iter().all(|x| x.level <= Level::Soon));
    }

    #[test]
    fn low_water_is_urgent_and_comes_first() {
        let mut n = ready();
        n.water_days = Some(1.5);
        n.food_days = Some(9.0);
        n.family_done = false;
        let r = suggest(&n, 6);
        assert_eq!(r[0].level, Level::Urgent);
        assert!(r[0].title.starts_with("Water lasts only 1.5 days"), "{}", r[0].title);
        assert!(r.iter().any(|x| x.title == "Food lasts 9.0 days" && x.level == Level::Soon));
        assert_eq!(r[0].tab.as_deref(), Some("Inventory"));
    }

    #[test]
    fn dates_and_missing_numbers_are_reported_with_counts() {
        let mut n = ready();
        n.past_date = 1;
        n.expiring_soon = 3;
        n.no_daily_use = 2;
        let t: Vec<String> = suggest(&n, 6).into_iter().map(|x| x.title).collect();
        assert!(t.contains(&"1 item is past their date".to_string()), "{:?}", t);
        assert!(t.contains(&"3 items reach their dates within 3 months".to_string()), "{:?}", t);
        assert!(t.contains(&"2 items have no daily use".to_string()), "{:?}", t);
    }

    #[test]
    fn the_list_is_cut_to_the_limit_keeping_the_most_pressing() {
        let n = Snapshot { drills_total: 6, act_days: 3.0, check_days: 14.0, ..Default::default() };
        let r = suggest(&n, 2);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].title, "Count your supplies");
    }

    #[test]
    fn a_missing_model_and_a_stopped_model_are_told_apart() {
        let mut n = ready();
        n.assistant_ready = false;
        n.models = 0;
        assert!(suggest(&n, 6)[0].text.contains("No model file"));
        n.models = 1;
        assert!(suggest(&n, 6)[0].text.contains("not running"));
    }
}
