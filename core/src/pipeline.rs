//! The whole answer path in one place:
//! safety check -> library search -> model -> output check -> reply with sources.

use crate::{prompt, retrieval, safety};
use std::path::Path;

/// A model that can complete a prompt. llama.cpp is plugged in behind this trait.
pub trait Model {
    fn complete(&self, prompt: &str) -> Result<String, String>;
}

/// Used until llama.cpp is connected: shows the best library passage as it is.
pub struct NoModel;
impl Model for NoModel {
    fn complete(&self, prompt: &str) -> Result<String, String> {
        // The first numbered passage in the prompt is the best match.
        let start = prompt.find("[1] ").ok_or("no passage")?;
        let after = &prompt[start..];
        let body = after.split_once('\n').map(|x| x.1).unwrap_or("");
        let text = body.split("\n\n").next().unwrap_or("").trim();
        Ok(format!("(No model connected yet. This is the best matching passage from the library.)\n\n{}", text))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// The medication rule fired; the fixed reply was returned and no model ran.
    Refused,
    /// Nothing relevant in the library; no model ran.
    NotFound,
    Answered,
    /// The model failed or its answer was replaced by the safety output check.
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

pub fn answer(question: &str, library: &Path, model: &dyn Model) -> Reply {
    if let Some(fixed) = safety::gate(question) {
        return Reply {
            kind: Kind::Refused,
            text: fixed.to_string(),
            sources: vec![],
            card: Some(safety::FIXED_REPLY_CARD.to_string()),
        };
    }
    let passages = retrieval::search(library, question, 3);
    let prompt = match prompt::build(question, &passages) {
        Some(p) => p,
        None => {
            return Reply { kind: Kind::NotFound, text: prompt::NOT_FOUND.to_string(), sources: vec![], card: None }
        }
    };
    let sources = prompt::source_labels(&passages);
    match model.complete(&prompt) {
        Ok(raw) => {
            let checked = safety::filter_output(raw.clone());
            if checked != raw {
                Reply { kind: Kind::Replaced, text: checked, sources: vec![], card: Some(safety::FIXED_REPLY_CARD.into()) }
            } else {
                Reply { kind: Kind::Answered, text: raw, sources, card: None }
            }
        }
        Err(_) => Reply {
            kind: Kind::Replaced,
            text: "The model isn't available right now. You can still read the Library and the emergency cards.".into(),
            sources: vec![],
            card: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lib() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-sample")
    }

    struct Bad;
    impl Model for Bad {
        fn complete(&self, _: &str) -> Result<String, String> {
            Ok("Take 12 units of insulin now.".into())
        }
    }
    struct Boom;
    impl Model for Boom {
        fn complete(&self, _: &str) -> Result<String, String> {
            Err("crashed".into())
        }
    }
    struct MustNotRun;
    impl Model for MustNotRun {
        fn complete(&self, _: &str) -> Result<String, String> {
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
        let r = answer("How do I tune a violin?", &lib(), &MustNotRun);
        assert_eq!(r.kind, Kind::NotFound);
    }

    #[test]
    fn model_answer_with_a_dose_is_replaced() {
        let r = answer("How long do I boil water?", &lib(), &Bad);
        assert_eq!(r.kind, Kind::Replaced);
        assert_eq!(r.text, safety::FIXED_REPLY);
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
