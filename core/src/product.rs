//! The three products and what each one switches on. Go, Vault and Hub are one program: the
//! drive's `cindrix-drive.json` says which one it is (`"edition": "go" | "vault" | "hub"`), and
//! this module turns that word into names, a content budget and a list of features. Standard
//! library only, like the rest of the core.

/// Content budget for Go, in GB. A 128 GB stick keeps room for the owner's own files.
pub const GO_BUDGET_GB: f64 = 100.0;
/// Content budget for Vault, in GB. A 4 TB drive keeps room for backups, scans and photos.
pub const VAULT_BUDGET_GB: f64 = 3400.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    Go,
    Vault,
    Hub,
}

/// What a product turns on. The interface reads these flags instead of asking "which product?",
/// so a later product (or a changed decision) is one edit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    /// Chat with the local model. Needs a model file and enough memory.
    pub assistant: bool,
    /// Library: shelf, archives and emergency cards.
    pub library: bool,
    /// Maps screen (not built yet).
    pub maps: bool,
    /// Plan screens: start, inventory, drills, family, binder, what fails next.
    pub plan: bool,
    /// The "Save a website" crawler.
    pub web_saver: bool,
    /// Offer to share the archives on the local network. Always off until the person turns it
    /// on and confirms; this flag only says whether the offer is made prominently.
    pub serve_archives: bool,
    /// Music player (the owner's own music in `music/`, not counted in the size budget).
    pub music: bool,
}

impl Product {
    /// Read the `edition` word from the drive marker. Unknown or empty means Go, the smallest
    /// product, so a stray or hand-made marker never switches on something heavier.
    pub fn from_edition(edition: &str) -> Product {
        match edition.trim().to_lowercase().as_str() {
            "vault" => Product::Vault,
            "hub" => Product::Hub,
            _ => Product::Go,
        }
    }

    /// The word written in the drive marker.
    pub fn id(self) -> &'static str {
        match self {
            Product::Go => "go",
            Product::Vault => "vault",
            Product::Hub => "hub",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Product::Go => "Cindrix Ops Go",
            Product::Vault => "Cindrix Ops Vault",
            Product::Hub => "Cindrix Ops Hub",
        }
    }

    /// Hard size limit for the content on the drive, in GB. Hub has none: it serves whatever
    /// the attached drive holds.
    pub fn budget_gb(self) -> Option<f64> {
        match self {
            Product::Go => Some(GO_BUDGET_GB),
            Product::Vault => Some(VAULT_BUDGET_GB),
            Product::Hub => None,
        }
    }

    pub fn features(self) -> Features {
        match self {
            Product::Go | Product::Vault => Features {
                assistant: true,
                library: true,
                maps: true,
                plan: true,
                web_saver: true,
                serve_archives: false,
                music: true,
            },
            // The Hub is the same program run as a server on a tablet or small computer. The
            // model is the heaviest and least proven part on Android, so it is off by default;
            // the viewer, the shelf and the archive server are the point. OPEN: confirm.
            Product::Hub => Features {
                assistant: false,
                library: true,
                maps: true,
                plan: true,
                web_saver: true,
                serve_archives: true,
                music: true,
            },
        }
    }
}

/// Does `total_gb` of content fit this product's budget? Err carries a plain-words message.
pub fn check_budget(product: Product, total_gb: f64) -> Result<(), String> {
    match product.budget_gb() {
        Some(max) if total_gb > max => Err(format!(
            "{} content is {:.1} GB, over the {:.0} GB limit by {:.1} GB.",
            product.name(),
            total_gb,
            max,
            total_gb - max
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_words_map_to_products() {
        assert_eq!(Product::from_edition("go"), Product::Go);
        assert_eq!(Product::from_edition("Vault"), Product::Vault);
        assert_eq!(Product::from_edition(" HUB "), Product::Hub);
    }

    #[test]
    fn unknown_or_empty_edition_is_go() {
        assert_eq!(Product::from_edition(""), Product::Go);
        assert_eq!(Product::from_edition("platinum"), Product::Go);
    }

    #[test]
    fn id_round_trips() {
        for p in [Product::Go, Product::Vault, Product::Hub] {
            assert_eq!(Product::from_edition(p.id()), p);
        }
    }

    #[test]
    fn names_use_the_confirmed_spelling() {
        for p in [Product::Go, Product::Vault, Product::Hub] {
            assert!(p.name().starts_with("Cindrix Ops "));
        }
    }

    #[test]
    fn budgets_match_the_content_manifest_doc() {
        assert_eq!(Product::Go.budget_gb(), Some(100.0));
        assert_eq!(Product::Vault.budget_gb(), Some(3400.0));
        assert_eq!(Product::Hub.budget_gb(), None);
    }

    #[test]
    fn budget_check_passes_at_the_limit_and_fails_over_it() {
        assert!(check_budget(Product::Go, 100.0).is_ok());
        let e = check_budget(Product::Go, 100.5).unwrap_err();
        assert!(e.contains("Cindrix Ops Go") && e.contains("0.5"));
        assert!(check_budget(Product::Vault, 3400.0).is_ok());
        assert!(check_budget(Product::Vault, 3401.0).is_err());
        assert!(check_budget(Product::Hub, 99999.0).is_ok());
    }

    #[test]
    fn go_and_vault_have_the_same_features_and_hub_has_no_assistant() {
        assert_eq!(Product::Go.features(), Product::Vault.features());
        assert!(Product::Go.features().assistant);
        assert!(!Product::Hub.features().assistant);
        assert!(Product::Hub.features().library);
    }
}
