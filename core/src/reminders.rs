//! Due-date reminders for the host (docs/48, docs/61): what is late, due today, due tomorrow, or about to start.
//! It only chooses and words them from the one agenda the program already makes (`ops::agenda`); the screen shows
//! them in the alert bar and beeps once for each new one while the program is open. Nothing leaves the computer.

use crate::ops::AgendaEntry;

#[derive(Debug, Clone, PartialEq)]
pub struct Reminder {
    /// Steady for the same thing on the same day, so "Got it" stays pressed and a new one beeps.
    pub key: String,
    pub text: String,
    pub urgent: bool,
    /// The screen to open: "Todo", "Calendar" or "Inventory".
    pub go: String,
}

/// How long before a timed event starts it is mentioned, and how long after it started it still is.
pub const EVENT_LEAD_MINUTES: i64 = 60;
pub const EVENT_GRACE_MINUTES: i64 = 15;

fn minutes_of(time: &str) -> Option<i64> {
    let (h, m) = time.split_once(':')?;
    let (h, m) = (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?);
    if (0..24).contains(&h) && (0..60).contains(&m) { Some(h * 60 + m) } else { None }
}

fn go_for(kind: &str) -> &'static str {
    match kind {
        "task" => "Todo",
        "item" => "Inventory",
        _ => "Calendar",
    }
}

/// `entries` come from `ops::agenda` for `today` to `today + 1`. `minute` is the minute of the local day.
pub fn due(entries: &[AgendaEntry], today: i64, minute: i64) -> Vec<Reminder> {
    let mut out: Vec<Reminder> = Vec::new();
    let mut late_items = 0usize;
    for e in entries {
        if e.overdue {
            if e.kind == "item" { late_items += 1; continue; }
            out.push(Reminder { key: format!("late:{}:{}", e.kind, e.id), text: format!("Late: {}", e.title), urgent: e.kind == "task", go: go_for(&e.kind).into() });
            continue;
        }
        let day_word = if e.day == today { "today" } else if e.day == today + 1 { "tomorrow" } else { continue };
        match e.kind.as_str() {
            "event" => {
                if e.day != today { continue; }
                match minutes_of(&e.time) {
                    None => out.push(Reminder { key: format!("event:{}:{}", e.id, e.date), text: format!("Today: {}", e.title), urgent: false, go: "Calendar".into() }),
                    Some(at) => {
                        let until = at - minute;
                        if (-EVENT_GRACE_MINUTES..=EVENT_LEAD_MINUTES).contains(&until) {
                            let when = if until > 0 { format!("in {} minute{}", until, if until == 1 { "" } else { "s" }) } else { "now".to_string() };
                            out.push(Reminder { key: format!("event:{}:{}", e.id, e.date), text: format!("{} at {}, {}", e.title, e.time, when), urgent: false, go: "Calendar".into() });
                        }
                    }
                }
            }
            "task" => out.push(Reminder { key: format!("task:{}:{}", e.id, e.date), text: format!("Due {}: {}", day_word, e.title), urgent: false, go: "Todo".into() }),
            "item" => out.push(Reminder { key: format!("item:{}:{}", e.id, e.date), text: format!("{} {}", e.title, day_word), urgent: false, go: "Inventory".into() }),
            "plan" => out.push(Reminder { key: format!("plan:{}:{}", e.id, e.date), text: format!("Due {}: {}", day_word, e.title), urgent: false, go: "Plan".into() }),
            _ => {}
        }
    }
    if late_items > 0 {
        // many old items would drown the bar: one line, and the Inventory screen has the list
        out.push(Reminder { key: format!("late:items:{}", late_items), text: format!("{} item{} past {} date on the Inventory list", late_items, if late_items == 1 { "" } else { "s" }, if late_items == 1 { "its" } else { "their" }), urgent: false, go: "Inventory".into() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{agenda, Dated, Event, Task};

    fn task(id: &str, title: &str, due: &str) -> Task { Task { id: id.into(), title: title.into(), due: due.into(), ..Default::default() } }
    const TODAY: i64 = 20_400; // any fixed day number

    fn day_text(d: i64) -> String {
        let (y, m, dd) = crate::calendar::civil_from_days(d);
        format!("{:04}-{:02}-{:02}", y, m, dd)
    }

    #[test]
    fn late_today_and_tomorrow_are_worded_and_late_tasks_are_urgent() {
        let tasks = vec![task("t1", "Pay the water bill", &day_text(TODAY - 3)), task("t2", "Rotate batteries", &day_text(TODAY)), task("t3", "Service the generator", &day_text(TODAY + 1)), task("t4", "Next month", &day_text(TODAY + 30))];
        let r = due(&agenda(&tasks, &[], &[], TODAY, TODAY + 1), TODAY, 600);
        let texts: Vec<&str> = r.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, vec!["Late: Pay the water bill", "Due today: Rotate batteries", "Due tomorrow: Service the generator"]);
        assert!(r[0].urgent && !r[1].urgent);
        assert_eq!(r[0].go, "Todo");
    }

    #[test]
    fn a_timed_event_is_mentioned_an_hour_before_and_a_little_after_but_not_before_or_long_after() {
        let ev = Event { id: "e1".into(), title: "Radio check".into(), date: day_text(TODAY), time: "10:00".into(), ..Default::default() };
        let at = |minute: i64| due(&agenda(&[], std::slice::from_ref(&ev), &[], TODAY, TODAY + 1), TODAY, minute);
        assert!(at(8 * 60).is_empty(), "two hours ahead: too early");
        assert_eq!(at(9 * 60 + 15)[0].text, "Radio check at 10:00, in 45 minutes");
        assert_eq!(at(10 * 60)[0].text, "Radio check at 10:00, now");
        assert_eq!(at(10 * 60 + 10)[0].text, "Radio check at 10:00, now");
        assert!(at(10 * 60 + 30).is_empty(), "long after it started");
        let all_day = Event { id: "e2".into(), title: "Market day".into(), date: day_text(TODAY), ..Default::default() };
        assert_eq!(due(&agenda(&[], &[all_day], &[], TODAY, TODAY + 1), TODAY, 300)[0].text, "Today: Market day");
        // tomorrow's events are for the Calendar, not the bar
        let tom = Event { id: "e3".into(), title: "Later".into(), date: day_text(TODAY + 1), ..Default::default() };
        assert!(due(&agenda(&[], &[tom], &[], TODAY, TODAY + 1), TODAY, 300).is_empty());
    }

    #[test]
    fn many_late_inventory_items_become_one_line_and_keys_stay_steady() {
        let extra: Vec<Dated> = (0..5).map(|i| Dated { kind: "item".into(), id: format!("i{}", i), title: format!("Thing {}", i), day: TODAY - 10 }).collect();
        let r = due(&agenda(&[], &[], &extra, TODAY, TODAY + 1), TODAY, 0);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].text, "5 items past their date on the Inventory list");
        assert_eq!(r[0].go, "Inventory");
        let t = vec![task("t1", "X", &day_text(TODAY))];
        let a = due(&agenda(&t, &[], &[], TODAY, TODAY + 1), TODAY, 100);
        let b = due(&agenda(&t, &[], &[], TODAY, TODAY + 1), TODAY, 200);
        assert_eq!(a[0].key, b[0].key, "the same thing keeps its key through the day");
        let next_day = due(&agenda(&t, &[], &[], TODAY + 1, TODAY + 2), TODAY + 1, 100);
        assert_ne!(a[0].key, next_day[0].key, "late is a new reminder the next day");
    }
}
