//! Checks the in-app reader's assumptions against a real kiwix-serve:
//!   cargo run --example probe_kiwix -- <port> <expected text in the sample archive>
use cindrix_core::kiwix;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a.get(1).and_then(|p| p.parse().ok()).expect("port");
    let want = a.get(2).cloned().unwrap_or_default();
    let mut ok = true;
    let mut check = |name: &str, good: bool, detail: String| {
        println!("{} {}: {}", if good { "ok  " } else { "FAIL" }, name, detail);
        ok &= good;
    };

    let cat = kiwix::get_full(port, "/catalog/v2/entries").expect("catalog");
    let books = kiwix::parse_catalog(&String::from_utf8_lossy(&cat.body));
    check("catalog lists a book", !books.is_empty(), format!("{:?}", books));
    let Some(book) = books.first() else { std::process::exit(1) };

    // The sample archive made by our website saver has no start page, so the book address may
    // be a 404; the page named in the third argument is used instead.
    let root = kiwix::follow(port, &format!("/content/{}", book.name));
    check("book address answers (informational)", true, format!("{:?}", root.as_ref().map(|(r, p)| (r.status, p.clone())).map_err(|e| e.clone())));
    let page_path = a.get(3).map(|p| format!("/content/{}/{}", book.name, p)).unwrap_or_else(|| format!("/content/{}", book.name));
    let (main, final_path) = kiwix::follow(port, &page_path).expect("page");
    check("a page is served as HTML", main.status == 200 && main.content_type.starts_with("text/html"), format!("{} {} {}", main.status, main.content_type, final_path));

    let html = String::from_utf8_lossy(&main.body).to_string();
    let injected = kiwix::inject_reader_script(&html);
    check("script goes in", injected.contains("cx:'page'") && injected.len() > html.len(), format!("{} -> {} bytes", html.len(), injected.len()));

    let sug = kiwix::get_full(port, &format!("/suggest?content={}&term=water", kiwix::url_encode(&book.name))).expect("suggest");
    check("suggest answers JSON", sug.status == 200, format!("{} {} {}", sug.status, sug.content_type, String::from_utf8_lossy(&sug.body).chars().take(200).collect::<String>()));

    let s = kiwix::get_full(port, &format!("/search?content={}&pattern=water&format=xml", kiwix::url_encode(&book.name))).expect("search");
    let hits = kiwix::parse_search(&String::from_utf8_lossy(&s.body));
    // An archive without a full-text index (like the website saver's) answers 404; that is normal.
    check("search answers or says there is no index", s.status == 200 || s.status == 404, format!("{} {} hits, first {:?}", s.status, hits.len(), hits.first()));

    let skin = kiwix::get_full(port, "/skin/kiwix.css");
    check("skin files are reachable (informational)", true, format!("{:?}", skin.map(|r| r.status)));

    if !want.is_empty() {
        let page = hits.first().map(|h| h.1.clone()).unwrap_or_else(|| final_path.clone());
        let r = kiwix::follow(port, &page).map(|(r, _)| String::from_utf8_lossy(&r.body).contains(&want)).unwrap_or(false);
        check("an article holds the expected text", r, page);
    }
    if !ok {
        std::process::exit(1);
    }
}
