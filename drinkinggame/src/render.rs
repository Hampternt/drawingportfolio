//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.

/// Same escape set as the portfolio's html_escape.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
