//! Checks a finished drive folder against the rules in docs 7 and 14.
//!   cargo run --release --example check_drive -- D:\Cindrix
//! Exits with 1 when something must be fixed, 0 otherwise. Reads only; changes nothing.

use cindrix_core::drivecheck::check_drive;

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("Usage: check_drive <drive folder>");
        std::process::exit(2);
    };
    let r = check_drive(std::path::Path::new(&dir));
    println!("{}: {:.1} GB of content, {} archive(s), {} PDF(s)", r.product.name(), r.content_gb, r.zims, r.pdfs);
    match r.product.budget_gb() {
        Some(max) => println!("Size limit: {:.0} GB", max),
        None => println!("Size limit: none"),
    }
    for w in &r.warnings {
        println!("CHECK BY HAND: {}", w);
    }
    for p in &r.problems {
        println!("MUST FIX: {}", p);
    }
    if r.ok() {
        println!("No problems found.");
    } else {
        std::process::exit(1);
    }
}
