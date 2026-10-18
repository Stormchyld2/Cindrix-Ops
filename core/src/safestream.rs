//! Streaming with the medication check applied BEFORE text reaches the screen.
//!
//! Text is held back so a dose can be caught before any part of it is shown:
//! - a sentence that contains a digit is held until the sentence ends and has passed the check;
//! - otherwise the last four words are held.
//! If the check trips, everything not yet shown is dropped and the fixed reply is used.

use crate::safety;

const HOLD_WORDS: usize = 4;

#[derive(Debug, PartialEq, Eq)]
pub enum Out {
    /// Nothing new is safe to show yet.
    Nothing,
    /// New text that is safe to append to what is already shown.
    Emit(String),
    /// A dose was detected. Show the fixed reply instead of the answer.
    Replace,
}

#[derive(Default)]
pub struct SafeStream {
    text: String,
    emitted: usize,
    tripped: bool,
}

fn is_sentence_end(text: &str, i: usize) -> bool {
    // i is the byte index of a '.', '!', '?' or '\n'.
    let b = text.as_bytes();
    match b[i] {
        b'\n' => true,
        _ => b.get(i + 1).map(|c| c.is_ascii_whitespace()).unwrap_or(true),
    }
}

/// Byte index where the current (unfinished) sentence starts.
fn sentence_start(text: &str) -> usize {
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?' | '\n') && is_sentence_end(text, i) {
            start = i + c.len_utf8();
        }
    }
    start
}

/// Byte index where the last `n` words begin.
fn tail_words_start(text: &str, n: usize) -> usize {
    let mut starts: Vec<usize> = Vec::new();
    let mut in_word = false;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            starts.push(i);
        }
    }
    if starts.len() <= n { 0 } else { starts[starts.len() - n] }
}

impl SafeStream {
    pub fn new() -> SafeStream {
        SafeStream::default()
    }

    pub fn is_tripped(&self) -> bool {
        self.tripped
    }

    pub fn push(&mut self, token: &str) -> Out {
        if self.tripped {
            return Out::Nothing;
        }
        self.text.push_str(token);
        if safety::output_contains_dose(&self.text) {
            self.tripped = true;
            return Out::Replace;
        }
        let ss = sentence_start(&self.text);
        let has_digit = self.text[ss..].chars().any(|c| c.is_ascii_digit());
        let words = tail_words_start(&self.text, HOLD_WORDS);
        let mut safe_end = if has_digit { ss.min(words) } else { words };
        // never split inside a character
        while safe_end > 0 && !self.text.is_char_boundary(safe_end) {
            safe_end -= 1;
        }
        if safe_end > self.emitted {
            let out = self.text[self.emitted..safe_end].to_string();
            self.emitted = safe_end;
            Out::Emit(out)
        } else {
            Out::Nothing
        }
    }

    /// Call when the model has finished: checks the whole answer once more and releases the rest.
    pub fn finish(&mut self) -> Out {
        if self.tripped {
            return Out::Nothing;
        }
        if safety::output_contains_dose(&self.text) {
            self.tripped = true;
            return Out::Replace;
        }
        if self.emitted < self.text.len() {
            let out = self.text[self.emitted..].to_string();
            self.emitted = self.text.len();
            Out::Emit(out)
        } else {
            Out::Nothing
        }
    }

    /// Everything the model produced, for logging in tests.
    pub fn full_text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tokens: &[&str]) -> (String, bool) {
        let mut s = SafeStream::new();
        let mut shown = String::new();
        let mut replaced = false;
        for t in tokens {
            match s.push(t) {
                Out::Emit(x) => shown.push_str(&x),
                Out::Replace => replaced = true,
                Out::Nothing => {}
            }
        }
        match s.finish() {
            Out::Emit(x) => shown.push_str(&x),
            Out::Replace => replaced = true,
            Out::Nothing => {}
        }
        (shown, replaced)
    }

    #[test]
    fn normal_answer_comes_through_in_full() {
        let toks = ["Take", " about", " 15", " grams", " of", " sugar", ".", " Wait", " 15", " minutes", " and", " recheck", "."];
        let (shown, replaced) = run(&toks);
        assert!(!replaced);
        assert_eq!(shown, "Take about 15 grams of sugar. Wait 15 minutes and recheck.");
    }

    #[test]
    fn dose_is_never_shown_even_in_part() {
        let toks = ["Take", " 10", " units", " of", " insulin", " now", "."];
        let (shown, replaced) = run(&toks);
        assert!(replaced);
        assert!(!shown.contains("10"), "shown: {:?}", shown);
        assert!(!shown.contains("units"), "shown: {:?}", shown);
    }

    #[test]
    fn dose_with_medicine_named_late_is_still_held() {
        let toks = ["Give", " 5", " mg", " of", " the", " regular", " type", " of", " insulin", " today", "."];
        let (shown, replaced) = run(&toks);
        assert!(replaced);
        assert!(!shown.contains('5'), "shown: {:?}", shown);
    }

    #[test]
    fn earlier_safe_sentences_may_already_be_shown() {
        let toks = ["Stay", " calm", " and", " sit", " down", " first", ".", " Take", " 10", " units", " of", " insulin", "."];
        let (shown, replaced) = run(&toks);
        assert!(replaced);
        assert!(shown.starts_with("Stay calm"));
        assert!(!shown.contains("10"));
    }

    #[test]
    fn glucose_readings_are_fine() {
        let toks = ["Treat", " if", " below", " 70", " mg", "/", "dL", " and", " recheck", " in", " 15", " minutes", "."];
        let (shown, replaced) = run(&toks);
        assert!(!replaced);
        assert!(shown.contains("70 mg/dL"));
    }

    #[test]
    fn multibyte_text_does_not_panic() {
        let toks = ["Agua", " potable", " é", " ñ", " 😀", " listo", "."];
        let (shown, replaced) = run(&toks);
        assert!(!replaced);
        assert!(shown.contains("😀"));
    }
}
