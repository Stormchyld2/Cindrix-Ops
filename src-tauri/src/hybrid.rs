//! Tauri side of the survivalist side of the program (docs/51): the skills matrix, "what replaces it", the
//! readiness row and "in season now". The logic and its tests are in `core/` (skills, replace, readiness, seasons).
//! The people's grid and sign-offs are the family's own, so none of these commands is in the share allowlist.

use cindrix_core::{academy, calendar, cascade, drills, progresssheet, readiness, replace, seasons, skills, training};
use serde::Serialize;
use std::path::PathBuf;

/// A shipped file of the content folder: the drive's copy when it has one, otherwise the one that came with the program.
fn content_file(name: &str) -> Vec<PathBuf> {
    let r = crate::root();
    let mut v = vec![r.join("content").join(name)];
    let p = crate::program_dir().join("content").join(name);
    if !v.contains(&p) {
        v.push(p);
    }
    v.push(r.join("../../content").join(name));
    // The first that exists is the shipped list.
    let first = v.iter().find(|p| p.exists()).cloned();
    first.into_iter().collect()
}

fn skill_paths() -> Vec<PathBuf> {
    let mut v = content_file("skills.csv");
    v.push(crate::root().join("user").join("skills.csv"));
    v
}

fn replacement_paths() -> Vec<PathBuf> {
    let mut v = content_file("replacements.csv");
    v.push(crate::root().join("user").join("replacements.csv"));
    v
}

fn facts_for(people: &[String]) -> Result<Vec<skills::PersonFacts>, String> {
    let user = crate::root().join("user");
    let results = academy::Log::new(&user).list()?;
    let roster = training::Roster::new(&user).list()?;
    let courses = academy::load_all(&crate::academy_dirs()).courses;
    Ok(people
        .iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let r = training::report(&courses, &results, &roster, p.trim());
            let done = |c: &training::CourseState| c.complete() || !c.signed_date.is_empty();
            skills::PersonFacts {
                name: p.trim().to_string(),
                courses_done: r.courses.iter().filter(|c| done(c)).map(|c| c.id.clone()).collect(),
                courses_started: r.courses.iter().filter(|c| c.chosen && !done(c)).map(|c| c.id.clone()).collect(),
            }
        })
        .collect())
}

fn grid_for(people: &[String]) -> Result<skills::Grid, String> {
    let user = crate::root().join("user");
    let defs = skills::load(&skill_paths())?;
    let facts = facts_for(people)?;
    let sign = skills::Signoffs::new(&user).list()?;
    let ran: Vec<(String, String)> = drills::Log::new(&user).list()?.into_iter().map(|r| (r.drill, r.date)).collect();
    Ok(skills::build(&defs, &facts, &sign, &ran, calendar::today_days()))
}

#[derive(Serialize)]
pub struct SkillRowDto {
    skill: String,
    area: String,
    must_have: bool,
    proof: String,
    cells: Vec<&'static str>,
    signed: usize,
    flags: Vec<String>,
}

#[derive(Serialize)]
pub struct SkillsGridDto {
    people: Vec<String>,
    rows: Vec<SkillRowDto>,
    bar: usize,
    must_total: usize,
    must_covered: usize,
}

/// The grid of people against skills. The names come from the Family page (the interface passes them).
#[tauri::command]
pub fn skills_grid(people: Vec<String>) -> Result<SkillsGridDto, String> {
    let g = grid_for(&people)?;
    Ok(SkillsGridDto {
        people: g.people,
        bar: g.bar,
        must_total: g.must_total,
        must_covered: g.must_covered,
        rows: g.rows.into_iter().map(|r| SkillRowDto { skill: r.skill, area: r.area, must_have: r.must_have, proof: r.proof, cells: r.cells, signed: r.signed, flags: r.flags }).collect(),
    })
}

#[derive(Serialize)]
pub struct PhoneTicksDto {
    person: String,
    skills: Vec<String>,
    plan_steps: usize,
}

/// What phones say their people can do (accepted progress sheets). Only what they say: never a sign-off.
#[tauri::command]
pub fn phone_ticks() -> Result<Vec<PhoneTicksDto>, String> {
    Ok(progresssheet::summary(&crate::root().join("user"))?.into_iter().map(|p| PhoneTicksDto { person: p.person, skills: p.skills, plan_steps: p.plan_steps }).collect())
}

/// Sign a skill off for a person by hand, or take it back.
#[tauri::command]
pub fn skills_signoff(person: String, skill: String, by: String, on: bool) -> Result<(), String> {
    skills::Signoffs::new(&crate::root().join("user")).set(person.trim(), skill.trim(), &by, &calendar::today_string(), on)
}

#[derive(Serialize)]
pub struct ReplacementDto {
    need: String,
    replaced_by: String,
    skill: String,
    lead_time: String,
    /// "notstarted", "learning" or "inplace"
    state: &'static str,
    /// Days the stored supply lasts, when the inventory can tell
    supply_days: Option<f64>,
    warning: Option<String>,
}

fn replacements_now() -> Result<Vec<ReplacementDto>, String> {
    let user = crate::root().join("user");
    let list = replace::load(&replacement_paths())?;
    let states = replace::States::new(&user).list()?;
    let items = cindrix_core::userdata::UserDir::new(&user).load_inventory()?;
    Ok(list
        .into_iter()
        .map(|r| {
            let state = replace::state_of(&states, &r.need);
            let days = cascade::days_for_need(&items, &r.stored_as);
            ReplacementDto { warning: replace::warning(&r, state, days), supply_days: days, state, need: r.need, replaced_by: r.replaced_by, skill: r.skill, lead_time: r.lead_time }
        })
        .collect())
}

/// What takes the place of each stored supply, with the family's progress on each.
#[tauri::command]
pub fn replacements_list() -> Result<Vec<ReplacementDto>, String> {
    replacements_now()
}

#[tauri::command]
pub fn replacement_set(need: String, state: String) -> Result<(), String> {
    replace::States::new(&crate::root().join("user")).set(need.trim(), state.trim())
}

#[derive(Serialize)]
pub struct ReadinessDto {
    supplies_days: Option<f64>,
    skills_covered: usize,
    skills_total: usize,
    self_in_place: usize,
    self_total: usize,
}

/// The three counts for the Dashboard row.
#[tauri::command]
pub fn readiness_row(people: Vec<String>) -> Result<ReadinessDto, String> {
    let user = crate::root().join("user");
    let items = cindrix_core::userdata::UserDir::new(&user).load_inventory()?;
    let supplies = cascade::runs_out_first(&items).first().map(|r| r.days);
    let grid = grid_for(&people)?;
    let list = replace::load(&replacement_paths())?;
    let states = replace::States::new(&user).list()?;
    let r = readiness::compute(supplies, &grid, &list, &states);
    Ok(ReadinessDto { supplies_days: r.supplies_days, skills_covered: r.skills_covered, skills_total: r.skills_total, self_in_place: r.self_in_place, self_total: r.self_total })
}

#[derive(Serialize)]
pub struct InSeasonDto {
    item: String,
    region: String,
    from_month: u32,
    to_month: u32,
}

/// What is in season this month (wild food, game and fish), for the Calendar. Seasons and licences of the state apply.
#[tauri::command]
pub fn in_season_now() -> Result<Vec<InSeasonDto>, String> {
    let Some(p) = content_file("forage-seasons.csv").into_iter().next() else { return Ok(vec![]) };
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let list = seasons::parse_forage(&text)?;
    let (_, m) = calendar::today_ym();
    Ok(seasons::in_season(&list, m, "").into_iter().map(|f| InSeasonDto { item: f.item, region: f.region, from_month: f.from_month, to_month: f.to_month }).collect())
}
