use std::collections::HashMap;
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};

pub struct EmbeddedFile {
    pub content: &'static str,
    pub content_type: &'static str,
}

fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    format!("{}.js", &format!("{:x}", result)[..12])
}

static SCRIPT_ORDER: &[(&str, &str)] = &[
    ("swup4.min.js", include_str!("./scripts/swup4.min.js")),
    ("swup-preload3.min.js", include_str!("./scripts/swup-preload3.min.js")),
    ("swup-scripts2.min.js", include_str!("./scripts/swup-scripts2.min.js")),
    ("swup-head2.min.js", include_str!("./scripts/swup-head2.min.js")),
    ("frontend.js", include_str!("./scripts/frontend.js")),
];

pub static EMBEDDED_FILES: Lazy<HashMap<String, EmbeddedFile>> = Lazy::new(|| {
    SCRIPT_ORDER
        .iter()
        .map(|&(_name, content)| {
            let hash = hash_content(content);
            (
                hash,
                EmbeddedFile {
                    content,
                    content_type: "application/javascript",
                },
            )
        })
        .collect()
});

use crate::config::CONFIG;

pub fn get_script_tags() -> String {
    let prefix = CONFIG.static_url_prefix.as_deref().unwrap_or("/static");
    SCRIPT_ORDER
        .iter()
        .map(|&(_name, content)| {
            let hash = hash_content(content);
            format!("<script defer src=\"{}/noventa-static/{}\"></script>\n", prefix, hash)
        })
        .collect::<String>()
}