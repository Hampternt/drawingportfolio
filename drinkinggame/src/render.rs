//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.

use crate::models::LeaderboardRow;

/// Same escape set as the portfolio's html_escape.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Returns the <li> rows for the leaderboard. The surrounding
/// <ol id="leaderboard"> lives in the templates so SSE can replace
/// innerHTML wholesale.
pub fn leaderboard_items(rows: &[LeaderboardRow]) -> String {
    if rows.is_empty() {
        return r#"<li class="lb-empty">Nobody here yet</li>"#.to_string();
    }
    rows.iter()
        .map(|r| {
            format!(
                r#"<li><span class="lb-name">{}</span><span class="lb-counts">{} drinks &middot; {} shots</span></li>"#,
                html_escape(&r.name),
                r.drinks,
                r.shots
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape() {
        assert_eq!(html_escape("<b>&\"'"), "&lt;b&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn test_leaderboard_items_escapes_names() {
        let rows = vec![LeaderboardRow {
            name: "<script>".into(),
            drinks: 2,
            shots: 1,
        }];
        let html = leaderboard_items(&rows);
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("2 drinks"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_leaderboard_items_empty() {
        assert!(leaderboard_items(&[]).contains("Nobody here yet"));
    }
}
