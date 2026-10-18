use cindrix_core::{cards, pipeline};
use serde::Serialize;
use std::path::PathBuf;

/// The Cindrix root folder. Portable: the folder the program sits in.
/// Set CINDRIX_ROOT to point elsewhere (used in development).
fn root() -> PathBuf {
    if let Ok(p) = std::env::var("CINDRIX_ROOT") {
        return PathBuf::from(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn content_dir() -> PathBuf {
    let r = root();
    let here = r.join("content");
    if here.exists() { here } else { r.join("../../content") }
}

#[derive(Serialize)]
struct CardDto {
    id: String,
    title: String,
    kind: String,
    status: String,
    reviewer: String,
    reviewed: String,
    body: String,
}

#[derive(Serialize)]
struct ReplyDto {
    kind: &'static str,
    text: String,
    sources: Vec<String>,
    card: Option<String>,
}

#[tauri::command]
fn list_cards() -> Vec<CardDto> {
    cards::load_dir(&content_dir().join("cards"))
        .into_iter()
        .map(|c| CardDto { id: c.id, title: c.title, kind: c.kind, status: c.status, reviewer: c.reviewer, reviewed: c.reviewed, body: c.body })
        .collect()
}

#[tauri::command]
fn ask(question: String) -> ReplyDto {
    let lib = content_dir().join("library-sample");
    // NoModel until llama.cpp is connected.
    let r = pipeline::answer(&question, &lib, &pipeline::NoModel);
    ReplyDto {
        kind: match r.kind {
            pipeline::Kind::Refused => "Refused",
            pipeline::Kind::NotFound => "NotFound",
            pipeline::Kind::Answered => "Answered",
            pipeline::Kind::Replaced => "Replaced",
        },
        text: r.text,
        sources: r.sources,
        card: r.card,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_cards, ask])
        .run(tauri::generate_context!())
        .expect("error while running Cindrix Ops");
}
