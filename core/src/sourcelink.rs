//! Turns where a passage came from into an address the interface can open: an emergency card, an
//! Academy lesson, a plan or a Shelf book page. Anything else gets no address (the Ask window then
//! shows the words the source gave and nothing to open).
//!
//! Addresses: `card:<id>`, `academy:<lesson id>`, `plan:<id>`, `guide:<section>/<page>`, `book:<book id>:<page>`.

/// The address for a file shown as a path below a library or drive folder (slashes either way).
pub fn for_file(shown: &str) -> String {
    let parts: Vec<&str> = shown.split(|c| c == '/' || c == '\\').filter(|s| !s.is_empty()).collect();
    let Some(file) = parts.last() else { return String::new() };
    let lower = file.to_lowercase();
    if !lower.ends_with(".md") || file.starts_with('_') || file.starts_with('.') || lower == "readme.md" || lower == "course.md" {
        return String::new();
    }
    let stem = &file[..file.len() - 3];
    let dirs = &parts[..parts.len() - 1];
    if dirs.last().map(|d| d.eq_ignore_ascii_case("cards")) == Some(true) {
        return format!("card:{}", stem);
    }
    if dirs.last().map(|d| d.eq_ignore_ascii_case("plans")) == Some(true) {
        return format!("plan:{}", stem);
    }
    // a guide page is "guides/<section>/<file>" (Agro and radio reference)
    if let Some(i) = dirs.iter().rposition(|d| d.eq_ignore_ascii_case("guides")) {
        let below = &dirs[i + 1..];
        if below.len() == 1 && !below[0].starts_with('_') && !below[0].starts_with('.') {
            return format!("guide:{}/{}", below[0], stem);
        }
    }
    // a lesson is "module/file" in the basics, or "course/module/file" in a course folder
    if let Some(i) = dirs.iter().rposition(|d| d.eq_ignore_ascii_case("academy")) {
        let below = &dirs[i + 1..];
        if (below.len() == 1 || below.len() == 2) && !below.iter().any(|d| d.starts_with('_') || d.starts_with('.')) {
            return format!("academy:{}/{}", below.join("/"), stem);
        }
    }
    String::new()
}

/// The address of a page in a Shelf book. `heading` is the "page N" the book's text is split by.
pub fn book_page(book_id: &str, heading: &str) -> String {
    let page: u32 = heading.trim().trim_start_matches("page").trim().parse().unwrap_or(1);
    format!("book:{}:{}", book_id, page.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_pages_get_addresses() {
        assert_eq!(for_file("content/guides/crop-guides/tomatoes.md"), "guide:crop-guides/tomatoes");
        assert_eq!(for_file("D:\\guides\\radio-reference\\gmrs.md"), "guide:radio-reference/gmrs");
        assert_eq!(for_file("content/guides/README.md"), "");
        assert_eq!(for_file("content/guides/_notes/x.md"), "");
    }

    #[test]
    fn cards_plans_and_lessons_get_addresses() {
        assert_eq!(for_file("content/cards/low-blood-sugar.md"), "card:low-blood-sugar");
        assert_eq!(for_file("D:\\content\\plans\\first-72-hours.md"), "plan:first-72-hours");
        assert_eq!(for_file("content/academy/water/purify/boiling.md"), "academy:water/purify/boiling");
        assert_eq!(for_file("content/academy/basics-water/boiling.md"), "academy:basics-water/boiling");
    }

    #[test]
    fn templates_descriptions_and_other_files_get_none() {
        assert_eq!(for_file("content/cards/_template-card.md"), "");
        assert_eq!(for_file("content/academy/water/course.md"), "");
        assert_eq!(for_file("content/academy/water/purify/_module.md"), "");
        assert_eq!(for_file("notes/water.md"), "");
        assert_eq!(for_file("content/academy/a/b/c/d.md"), "");
        assert_eq!(for_file("content/cards/pic.png"), "");
        assert_eq!(for_file(""), "");
    }

    #[test]
    fn a_book_page_address_keeps_the_page_number() {
        assert_eq!(book_page("u1", "page 12"), "book:u1:12");
        assert_eq!(book_page("u1", "odd"), "book:u1:1");
    }
}
