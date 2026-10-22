//! The seed vault: seeds kept for planting, stored as `user/seeds.csv`. Like the Inventory, but with the
//! questions that matter for seed: what it is, how old it is, how long that kind lasts, and how much is left.
//!
//! How long seed lasts is a rule of thumb from university extension services (Purdue, University of
//! Wisconsin): onion and parsnip about 1 year; sweet corn, okra and pepper 2; bean, carrot, pea, kohlrabi
//! 3; beet, chard, pumpkin, tomato, turnip, watermelon 4; cucumber, melon, radish, cabbage family, lettuce
//! and spinach 5 (the shorter figure where the two disagree). Stored cool, dry and dark it can last longer,
//! and a germination test beats any chart.

use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "id,crop,variety,kind,packets,seeds_per_packet,year_packed,germination,tested,source,location,plant_when,days_to_harvest,target,notes";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seed {
    pub id: String,
    pub crop: String,
    pub variety: String,
    /// "Heirloom", "Open-pollinated", "Hybrid" or empty
    pub kind: String,
    pub packets: f64,
    /// About how many seeds are in one packet, or 0 when not known
    pub seeds_per_packet: f64,
    /// The year the seed was harvested or packed, or 0
    pub year_packed: i32,
    /// The percent that sprouted at the last test (0 to 100), or 0 when never tested
    pub germination: f64,
    /// "YYYY-MM-DD" of that test
    pub tested: String,
    pub source: String,
    /// Where it is kept
    pub location: String,
    /// When to plant it: "Spring after last frost", "Late summer" ...
    pub plant_when: String,
    pub days_to_harvest: f64,
    /// How many packets you want to keep
    pub target: f64,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Check,
    Replace,
}

/// About how many years the seed of a crop stays good when kept cool, dry and dark.
pub fn years_good(crop: &str) -> u32 {
    let c = crop.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| c.contains(x));
    if has(&["onion", "leek", "parsnip", "chive"]) {
        1
    } else if has(&["corn", "okra", "pepper", "chili", "chile"]) {
        2
    } else if has(&["bean", "carrot", "pea", "kohlrabi", "squash", "zucchini", "sunflower", "herb", "basil", "parsley", "dill", "cilantro", "coriander"]) {
        3
    } else if has(&["beet", "chard", "pumpkin", "tomato", "turnip", "watermelon", "kale", "mustard"]) {
        4
    } else if has(&["cucumber", "melon", "cantaloupe", "radish", "lettuce", "spinach", "cabbage", "broccoli", "cauliflower", "brussels", "collard"]) {
        5
    } else {
        3
    }
}

pub fn age(s: &Seed, year: i32) -> Option<i32> {
    if s.year_packed > 0 { Some(year - s.year_packed) } else { None }
}

/// Replace when it is older than it should be and has not tested well; check in its last year, when the last
/// test was poor, or when fewer packets are left than you want.
pub fn status(s: &Seed, year: i32) -> Status {
    let good_test = s.germination >= 70.0 && !s.tested.is_empty();
    let poor_test = !s.tested.is_empty() && s.germination > 0.0 && s.germination < 50.0;
    if s.packets <= 0.0 && s.target > 0.0 {
        return Status::Replace;
    }
    if poor_test {
        return Status::Replace;
    }
    if let Some(a) = age(s, year) {
        let life = years_good(&s.crop) as i32;
        if a > life && !good_test {
            return Status::Replace;
        }
        if (a >= life && !good_test) || (!s.tested.is_empty() && s.germination > 0.0 && s.germination < 70.0) {
            return Status::Check;
        }
    }
    if s.target > 0.0 && s.packets < s.target {
        return Status::Check;
    }
    Status::Ok
}

pub struct Vault {
    pub path: PathBuf,
}

fn num(s: &str, what: &str, line: usize) -> Result<f64, String> {
    if s.trim().is_empty() {
        return Ok(0.0);
    }
    s.trim().parse::<f64>().map_err(|_| format!("seeds.csv line {}: {} is not a number: {}", line, what, s))
}

impl Vault {
    pub fn new(user_dir: &Path) -> Vault {
        Vault { path: user_dir.join("seeds.csv") }
    }

    /// A missing file is an empty vault. A damaged one is an error and is never overwritten.
    pub fn list(&self) -> Result<Vec<Seed>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut out = Vec::new();
        for (i, l) in text.lines().enumerate() {
            let l = l.trim_end_matches('\r');
            if l.trim().is_empty() || (i == 0 && l.starts_with("id,")) {
                continue;
            }
            let f = split_record(l);
            if f.len() != 15 {
                return Err(format!("seeds.csv line {} is damaged", i + 1));
            }
            out.push(Seed {
                id: f[0].clone(), crop: f[1].clone(), variety: f[2].clone(), kind: f[3].clone(),
                packets: num(&f[4], "packets", i + 1)?, seeds_per_packet: num(&f[5], "seeds per packet", i + 1)?,
                year_packed: num(&f[6], "year", i + 1)? as i32, germination: num(&f[7], "germination", i + 1)?,
                tested: f[8].clone(), source: f[9].clone(), location: f[10].clone(), plant_when: f[11].clone(),
                days_to_harvest: num(&f[12], "days to harvest", i + 1)?, target: num(&f[13], "target", i + 1)?, notes: f[14].clone(),
            });
        }
        Ok(out)
    }

    pub fn save_all(&self, all: &[Seed]) -> Result<(), String> {
        let mut csv = String::from(HEADER);
        csv.push('\n');
        for s in all {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                quote(&s.id), quote(&s.crop), quote(&s.variety), quote(&s.kind), s.packets, s.seeds_per_packet, s.year_packed,
                s.germination, quote(&s.tested), quote(&s.source), quote(&s.location), quote(&s.plant_when), s.days_to_harvest,
                s.target, quote(&s.notes)
            ));
        }
        fsio::write_atomic(&self.path, csv.as_bytes()).map_err(|e| e.to_string())
    }

    /// Add (empty id) or replace one entry. Returns the whole list.
    pub fn save(&self, mut s: Seed) -> Result<Vec<Seed>, String> {
        if s.crop.trim().is_empty() {
            return Err("Write what the seed is, for example Tomato.".into());
        }
        // one entry is one line of the file
        for f in [&mut s.crop, &mut s.variety, &mut s.kind, &mut s.tested, &mut s.source, &mut s.location, &mut s.plant_when, &mut s.notes] {
            *f = f.replace(['\r', '\n'], " ").trim().to_string();
        }
        let mut all = self.list()?;
        if s.id.is_empty() {
            s.id = (all.iter().filter_map(|x| x.id.parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string();
            all.push(s);
        } else if let Some(slot) = all.iter_mut().find(|x| x.id == s.id) {
            *slot = s;
        } else {
            all.push(s);
        }
        self.save_all(&all)?;
        Ok(all)
    }

    pub fn delete(&self, ids: &[String]) -> Result<Vec<Seed>, String> {
        let all: Vec<Seed> = self.list()?.into_iter().filter(|s| !ids.contains(&s.id)).collect();
        self.save_all(&all)?;
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-seeds-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn tomato() -> Seed {
        Seed { crop: "Tomato".into(), variety: "Brandywine, pink".into(), kind: "Heirloom".into(), packets: 2.0, seeds_per_packet: 30.0, year_packed: 2024, ..Default::default() }
    }

    #[test]
    fn seed_life_by_crop() {
        assert_eq!(years_good("Onion"), 1);
        assert_eq!(years_good("Sweet corn"), 2);
        assert_eq!(years_good("Pole bean"), 3);
        assert_eq!(years_good("Roma tomato"), 4);
        assert_eq!(years_good("Lettuce"), 5);
        assert_eq!(years_good("Something odd"), 3);
    }

    #[test]
    fn old_seed_needs_a_test_and_a_good_test_forgives_age() {
        let mut s = tomato(); // good for about 4 years
        assert_eq!(status(&s, 2026), Status::Ok); // 2 years old
        assert_eq!(status(&s, 2028), Status::Check); // in its last year
        assert_eq!(status(&s, 2030), Status::Replace); // past it
        s.tested = "2030-03-01".into();
        s.germination = 85.0;
        assert_eq!(status(&s, 2030), Status::Ok, "a good test says the seed is fine");
        s.germination = 40.0;
        assert_eq!(status(&s, 2030), Status::Replace);
        s.germination = 60.0;
        assert_eq!(status(&s, 2026), Status::Check);
    }

    #[test]
    fn packets_below_the_goal_or_none_left() {
        let mut s = tomato();
        s.target = 3.0;
        assert_eq!(status(&s, 2026), Status::Check);
        s.packets = 0.0;
        assert_eq!(status(&s, 2026), Status::Replace);
        s.target = 0.0;
        s.packets = 0.0;
        assert_eq!(status(&s, 2026), Status::Ok);
    }

    #[test]
    fn the_vault_saves_edits_deletes_and_leaves_a_damaged_file_alone() {
        let d = tmp("vault");
        let v = Vault::new(&d);
        assert!(v.list().unwrap().is_empty());
        assert!(v.save(Seed::default()).is_err());
        let all = v.save(tomato()).unwrap();
        assert_eq!(all[0].id, "1");
        let mut b = tomato();
        b.crop = "Bean".into();
        v.save(b).unwrap();
        let mut edit = v.list().unwrap()[0].clone();
        edit.packets = 5.0;
        edit.notes = "grew well\nin 2025".into();
        v.save(edit.clone()).unwrap();
        let l = v.list().unwrap();
        assert_eq!((l.len(), l[0].packets, l[0].variety.as_str()), (2, 5.0, "Brandywine, pink"));
        assert_eq!(l[0].notes, "grew well in 2025");
        assert_eq!(v.delete(&["1".to_string()]).unwrap().len(), 1);
        fs::write(&v.path, format!("{}\n1,Tomato\n", HEADER)).unwrap();
        assert!(v.list().is_err());
        assert!(v.save(tomato()).is_err());
        assert_eq!(fs::read_to_string(&v.path).unwrap(), format!("{}\n1,Tomato\n", HEADER));
    }
}
