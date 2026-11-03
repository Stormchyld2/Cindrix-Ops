//! Runs the assistant's own library search against a real kiwix-serve and prints what happened
//! at each step, so a mismatch with the real output format is visible in the log:
//!   cargo run --example probe_search -- <port> "<question>" ["<question>" ...]
//! Exit code 1 if fewer than half of the questions produced a passage.
use cindrix_core::kiwix;

fn head(s: &[u8], n: usize) -> String {
    String::from_utf8_lossy(s).chars().take(n).collect::<String>().replace('\n', " ").replace('\r', "")
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a.get(1).and_then(|p| p.parse().ok()).expect("usage: probe_search <port> <question>...");
    let questions: Vec<&String> = a.iter().skip(2).collect();
    if questions.is_empty() {
        eprintln!("usage: probe_search <port> <question>...");
        std::process::exit(2);
    }

    let cat = kiwix::get_full(port, "/catalog/v2/entries?count=500").expect("catalog");
    let books = kiwix::parse_catalog(&String::from_utf8_lossy(&cat.body));
    println!("catalog: status {} , {} book(s): {:?}", cat.status, books.len(), books.iter().map(|b| b.name.as_str()).collect::<Vec<_>>());
    if books.is_empty() {
        println!("catalog start: {}", head(&cat.body, 400));
    }

    let asked = questions.len();
    let mut answered = 0usize;
    for q in questions {
        println!("\n=== {}", q);
        let kw = kiwix::keywords(q);
        println!("key words: {:?}", kw);
        for b in &books {
            // the raw answers, step by step
            let pattern = kw.join(" ");
            let s = kiwix::get_full(port, &format!("/search?content={}&pattern={}&pageLength=3&format=xml", kiwix::url_encode(&b.name), kiwix::url_encode(&pattern)));
            match s {
                Ok(r) => {
                    let hits = kiwix::parse_search(&String::from_utf8_lossy(&r.body));
                    println!("[{}] full-text search: status {} {} -> {} hit(s) {:?}", b.name, r.status, r.content_type, hits.len(), hits.first());
                    if hits.is_empty() {
                        println!("[{}]   raw start: {}", b.name, head(&r.body, 500));
                    } else if let Some(i) = String::from_utf8_lossy(&r.body).find("<item>") {
                        println!("[{}]   first raw item: {}", b.name, head(&r.body[i..], 400));
                    }
                }
                Err(e) => println!("[{}] full-text search: error {}", b.name, e),
            }
            if let Some(term) = kw.iter().max_by_key(|w| w.chars().count()) {
                match kiwix::get_full(port, &format!("/suggest?content={}&term={}&count=3", kiwix::url_encode(&b.name), kiwix::url_encode(term))) {
                    Ok(r) => {
                        let hits = kiwix::parse_suggest(&String::from_utf8_lossy(&r.body), &b.name);
                        println!("[{}] title search '{}': status {} -> {} hit(s) {:?}", b.name, term, r.status, hits.len(), hits.first());
                        println!("[{}]   suggest raw start: {}", b.name, head(&r.body, 300));
                    }
                    Err(e) => println!("[{}] title search: error {}", b.name, e),
                }
            }
        }
        // what the assistant really gets
        let passages = kiwix::search(port, q, 3);
        println!("assistant search: {} passage(s)", passages.len());
        for p in &passages {
            println!("  - {} / {} (score {}): {}", p.source, p.heading, p.score, head(p.text.as_bytes(), 160));
        }
        if !passages.is_empty() {
            answered += 1;
        }
    }
    println!("\nSUMMARY: {} of {} questions produced passages", answered, asked);
    if answered * 2 < asked {
        println!("FAIL: fewer than half of the questions produced a passage");
        std::process::exit(1);
    }
    println!("ok");
}
