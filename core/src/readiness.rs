//! The readiness row: three plain counts for supplies, skills and self-reliance, side by side. There is no blended
//! score, because one number would hide which of the three is weak.

use crate::replace::{self, Replacement};
use crate::skills::Grid;

#[derive(Debug, Clone, PartialEq)]
pub struct Readiness {
    /// Days until the first supply that is used daily runs out; None when no supply has a daily use.
    pub supplies_days: Option<f64>,
    /// Must-have skills covered by enough people, out of all must-have skills.
    pub skills_covered: usize,
    pub skills_total: usize,
    /// Needs with a renewable source in place, out of all needs.
    pub self_in_place: usize,
    pub self_total: usize,
}

pub fn compute(supplies_days: Option<f64>, grid: &Grid, replacements: &[Replacement], states: &[(String, String)]) -> Readiness {
    Readiness {
        supplies_days,
        skills_covered: grid.must_covered,
        skills_total: grid.must_total,
        self_in_place: replacements.iter().filter(|r| replace::state_of(states, &r.need) == "inplace").count(),
        self_total: replacements.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{build, parse_list, PersonFacts};

    fn rep(need: &str) -> Replacement {
        Replacement { need: need.into(), stored_as: "x".into(), replaced_by: "y".into(), skill: "z".into(), lead_time: "1 year".into(), lead_days: 365 }
    }

    #[test]
    fn the_three_counts_stand_apart() {
        let defs = parse_list("skill,area,proof,must_have\nWater,Water,hand,yes\nFire,Fire,hand,yes\nSoap,Soap,hand,no\n").unwrap();
        let people = vec![PersonFacts { name: "Sam".into(), ..Default::default() }];
        let sign = vec![crate::skills::Signoff { person: "Sam".into(), skill: "Water".into(), signed_by: "Sam".into(), signed_date: "d".into() }];
        let g = build(&defs, &people, &sign, &[], 20000);
        let reps = vec![rep("A"), rep("B"), rep("C")];
        let states = vec![("A".to_string(), "inplace".to_string()), ("B".to_string(), "learning".to_string())];
        let r = compute(Some(12.5), &g, &reps, &states);
        assert_eq!(r, Readiness { supplies_days: Some(12.5), skills_covered: 1, skills_total: 2, self_in_place: 1, self_total: 3 });
    }

    #[test]
    fn nothing_set_up_yet_reads_as_zero_not_as_an_error() {
        let g = build(&[], &[], &[], &[], 0);
        let r = compute(None, &g, &[], &[]);
        assert_eq!((r.skills_covered, r.skills_total, r.self_in_place, r.self_total), (0, 0, 0, 0));
        assert_eq!(r.supplies_days, None);
    }
}
