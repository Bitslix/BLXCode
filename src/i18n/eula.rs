//! EULA Markdown (per locale): compile-time [`include_str!`] → HTML via [`pulldown_cmark`].

use crate::i18n::locale::Locale;
use pulldown_cmark::{html, Options, Parser};

#[must_use]
pub fn markdown_source(locale: Locale) -> &'static str {
    match locale {
        Locale::DeDe => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/content/eula/de-DE.md"
        )),
        Locale::EnUs => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/content/eula/en-US.md"
        )),
    }
}

/// Converts trusted repository Markdown into HTML fragments for embedding in the EULA modal.
///
/// Content is authored by maintainers — not sanitized (same trust model as string literals).
#[must_use]
pub fn markdown_to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, opts);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    inject_eula_heading_id(&mut html_out);
    html_out
}

#[must_use]
pub fn localized_eula_html(locale: Locale) -> String {
    markdown_to_html(markdown_source(locale))
}

/// Ensures `<dialog aria-labelledby="eula-heading">` resolves to the first title.
fn inject_eula_heading_id(html: &mut String) {
    let Some(pos) = html.find("<h1>") else {
        return;
    };
    // Insert before the closing `>` of the opening `<h1>` tag.
    html.insert_str(pos + 3, r#" id="eula-heading""#);
}
