//! The whole answer path in one place:
//! safety check -> library search -> model (streamed) -> medication check on the way out -> reply with sources.

use crate::safestream::{Out, SafeStream};
use crate::{prompt, retrieval, safety};
use std::path::Path;

/// A model that can answer. llama.cpp is plugged in behind this trait.
pub trait Model {
    /// Stream the answer. `on_token` returns false to ask the model to stop.
    fn complete(&self, system: &str, user: &str, on_token: &mut dyn FnMut(&str) -> bool) -> Result<String, String>;
}

/// Used when no model is available: shows the best library passage as it is.
pub struct NoModel;
impl Model for NoModel {
    fn complete(&self, _system: &str, user: &str, on_token: &mut dyn FnMut(&str) -> bool) -> Result<String, String> {
        let start = user.find("[1] ").ok_or("no passage")?;
        let body = user[start..].split_once('\n').map(|x| x.1).unwrap_or("");
        let text = body.split("\n\n").next().unwrap_or("").trim();
        let out = format!("(No model is loaded. This is the best matching passage from the library.)\n\n{}", text);
        on_token(&out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// The medication rule fired; the fixed reply was returned and no model ran.
    Refused,
    /// Nothing relevant in the library; no model ran.
    NotFound,
    Answered,
    /// The model failed, or its answer was replaced by the medication check.
    Replaced,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub kind: Kind,
    pub text: String,
    pub sources: Vec<String>,
    /// Card to show next to the reply, if any.
    pub card: Option<String>,
}

/// What the screen should do as the answer arrives.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Add this text to the end of what is shown.
    Text(String),
    /// Throw away what is shown and show this instead.
    Replace(String),
}

pub fn answer_stream(question: &str, library: &Path, model: &dyn Model, emit: &mut dyn FnMut(Event)) -> Reply {
    answer_stream_with(question, &|| retrieval::search(library, question, 3), model, emit)
}

/// Same, with the passages supplied by the caller (for example the small built-in library plus Kiwix).
/// The safety check runs first, so `passages` is not even called for a medication-dose question.
pub fn answer_stream_with(question: &str, passages: &dyn Fn() -> Vec<retrieval::Passage>, model: &dyn Model, emit: &mut dyn FnMut(Event)) -> Reply {
    if let Some(fixed) = safety::gate(question) {
        emit(Event::Replace(fixed.to_string()));
        return Reply { kind: Kind::Refused, text: fixed.to_string(), sources: vec![], card: Some(safety::FIXED_REPLY_CARD.to_string()) };
    }
    let passages = passages();
    let (system, user) = match prompt::build_parts(question, &passages) {
        Some(p) => p,
        None => {
            emit(Event::Text(prompt::NOT_FOUND.to_string()));
            return Reply { kind: Kind::NotFound, text: prompt::NOT_FOUND.to_string(), sources: vec![], card: None };
        }
    };
    let sources = prompt::source_labels(&passages);

    let mut guard = SafeStream::new();
    let mut shown = String::new();
    let mut replaced = false;
    let result = model.complete(&system, &user, &mut |tok: &str| {
        match guard.push(tok) {
            Out::Emit(t) => {
                shown.push_str(&t);
                emit(Event::Text(t));
                true
            }
            Out::Replace => {
                replaced = true;
                false
            }
            Out::Nothing => !guard.is_tripped(),
        }
    });

    if replaced || guard.is_tripped() {
        emit(Event::Replace(safety::FIXED_REPLY.to_string()));
        return Reply { kind: Kind::Replaced, text: safety::FIXED_REPLY.to_string(), sources: vec![], card: Some(safety::FIXED_REPLY_CARD.to_string()) };
    }
    match result {
        Ok(_) => match guard.finish() {
            Out::Replace => {
                emit(Event::Replace(safety::FIXED_REPLY.to_string()));
                Reply { kind: Kind::Replaced, text: safety::FIXED_REPLY.to_string(), sources: vec![], card: Some(safety::FIXED_REPLY_CARD.to_string()) }
            }
            Out::Emit(t) => {
                shown.push_str(&t);
                emit(Event::Text(t));
                Reply { kind: Kind::Answered, text: shown, sources, card: None }
            }
            Out::Nothing => Reply { kind: Kind::Answered, text: shown, sources, card: None },
        },
        Err(_) => {
            let msg = "The model isn't available right now. You can still read the Library and the emergency cards.";
            if shown.is_empty() {
                emit(Event::Replace(msg.to_string()));
                Reply { kind: Kind::Replaced, text: msg.to_string(), sources: vec![], card: None }
            } else {
                let tail = "\n\n(The model stopped before it finished.)";
                emit(Event::Text(tail.to_string()));
                shown.push_str(tail);
                Reply { kind: Kind::Answered, text: shown, sources, card: None }
            }
        }
    }
}

/// The same path without streaming, for tests and simple callers.
pub fn answer(question: &str, library: &Path, model: &dyn Model) -> Reply {
    answer_stream(question, library, model, &mut |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lib() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-sample")
    }

    /// Streams the given tokens one by one.
    struct Tokens(Vec<&'static str>);
    impl Model for Tokens {
        fn complete(&self, _s: &str, _u: &str, on: &mut dyn FnMut(&str) -> bool) -> Result<String, String> {
            let mut all = String::new();
            for t in &self.0 {
                all.push_str(t);
                if !on(t) {
                    break;
                }
            }
            Ok(all)
        }
    }
    struct Boom;
    impl Model for Boom {
        fn complete(&self, _: &str, _: &str, _: &mut dyn FnMut(&str) -> bool) -> Result<String, String> {
            Err("crashed".into())
        }
    }
    struct MustNotRun;
    impl Model for MustNotRun {
        fn complete(&self, _: &str, _: &str, _: &mut dyn FnMut(&str) -> bool) -> Result<String, String> {
            panic!("model must not run");
        }
    }

    #[test]
    fn dosing_question_never_reaches_the_model() {
        let r = answer("How many units of insulin should I take?", &lib(), &MustNotRun);
        assert_eq!(r.kind, Kind::Refused);
        assert_eq!(r.card.as_deref(), Some("low-blood-sugar"));
    }

    #[test]
    fn unknown_topic_does_not_reach_the_model() {
        assert_eq!(answer("How do I tune a violin?", &lib(), &MustNotRun).kind, Kind::NotFound);
    }

    #[test]
    fn model_answer_with_a_dose_is_replaced_and_never_shown() {
        let mut events = Vec::new();
        let r = answer_stream(
            "How long do I boil water?",
            &lib(),
            &Tokens(vec!["Take", " 12", " units", " of", " insulin", " now", "."]),
            &mut |e| events.push(e),
        );
        assert_eq!(r.kind, Kind::Replaced);
        assert_eq!(r.text, safety::FIXED_REPLY);
        for e in &events {
            if let Event::Text(t) = e {
                assert!(!t.contains("12"), "a dose reached the screen: {:?}", events);
            }
        }
        assert_eq!(events.last(), Some(&Event::Replace(safety::FIXED_REPLY.to_string())));
    }

    #[test]
    fn good_streamed_answer_is_delivered_in_full_with_sources() {
        let mut shown = String::new();
        let r = answer_stream(
            "How long do I boil water?",
            &lib(),
            &Tokens(vec!["Boil", " it", " at", " a", " rolling", " boil", " for", " 1", " minute", "."]),
            &mut |e| if let Event::Text(t) = e { shown.push_str(&t) },
        );
        assert_eq!(r.kind, Kind::Answered);
        assert_eq!(shown, "Boil it at a rolling boil for 1 minute.");
        assert_eq!(r.text, shown);
        assert!(r.sources.iter().any(|s| s.starts_with("water")));
    }

    #[test]
    fn model_failure_gives_plain_message() {
        let r = answer("How long do I boil water?", &lib(), &Boom);
        assert_eq!(r.kind, Kind::Replaced);
        assert!(r.text.contains("isn't available"));
    }

    #[test]
    fn answer_without_model_shows_passage_and_source() {
        let r = answer("How long do I boil water?", &lib(), &NoModel);
        assert_eq!(r.kind, Kind::Answered);
        assert!(r.text.contains("rolling boil"));
        assert!(r.sources.iter().any(|s| s.starts_with("water")));
    }
}
