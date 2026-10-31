use crate::{middleware::OptionalAdmin, AppState};
use askama::Template;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "hub/hub.html")]
struct HubTemplate {
    is_admin: bool,
}

async fn hub_page(OptionalAdmin(is_admin): OptionalAdmin) -> impl IntoResponse {
    Html(HubTemplate { is_admin }.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(hub_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(is_admin: bool) -> String {
        HubTemplate { is_admin }.render().unwrap()
    }

    /// The opening tag of the first link to `href`, attributes included.
    fn anchor<'a>(html: &'a str, href: &str) -> &'a str {
        let start = html
            .find(&format!(r#"<a href="{href}""#))
            .unwrap_or_else(|| panic!("no link to {href}"));
        let tag = &html[start..];
        &tag[..tag.find('>').unwrap()]
    }

    fn footer(html: &str) -> &str {
        &html[html.find(r#"<footer class="hub-foot">"#).unwrap()..]
    }

    #[test]
    fn test_hub_links_to_standalone_pages_are_not_boosted() {
        // /drinks is a nest_service mount whose templates do not extend
        // base.html, and /admin keeps its styles in its own <head>: a boosted
        // swap drops both. The first /admin link is the header's settings
        // button, the second the footer's.
        let html = render(true);
        assert!(anchor(&html, "/drinks").contains(r#"hx-boost="false""#));
        assert!(anchor(&html, "/admin").contains(r#"hx-boost="false""#));
        assert!(anchor(footer(&html), "/admin").contains(r#"hx-boost="false""#));
    }

    #[test]
    fn test_hub_footer_claims_only_what_holds_and_names_the_contacts() {
        let html = render(false);
        let foot = footer(&html);
        // A Rust binary is a build step; the front end has none.
        assert!(!html.contains("no build step"));
        assert!(foot.contains("no front-end build step"));
        assert!(foot.contains(r#"href="mailto:jl@dblo.net""#));
        assert!(foot.contains(r#"href="https://github.com/Hampternt""#));
        // Logged out there is no /admin link. /docs is scoped to the footer:
        // the header nav carries it too, so a whole-page check proves nothing.
        assert!(!foot.contains(r#"href="/admin""#));
        assert!(foot.contains(r#"href="/docs""#));
    }

    #[test]
    fn test_hub_names_the_owner_by_the_short_form_and_publishes_no_number() {
        let html = render(false);
        assert!(html.contains("<strong>Jesper L.</strong>"));
        // Checked by shape, never by value, and only inside <main>: base.html's
        // `?v=` hashes are hex and would trip a digit-run check on the head.
        let main = &html[html.find("<main>").unwrap()..html.find("</main>").unwrap()];
        assert!(!main.contains("+47"), "the hub carries a country code");
        let digits: String = main.chars().filter(|c| *c != ' ').collect();
        let longest_run = digits
            .split(|c: char| !c.is_ascii_digit())
            .map(str::len)
            .max()
            .unwrap_or(0);
        assert!(longest_run < 8, "the hub carries a phone-length number");
    }

    #[test]
    fn test_every_hub_tile_carries_one_status_badge() {
        let html = render(false);
        let tiles = html.matches(r#"class="hm-hub""#).count();
        let badges = html.matches(r#"<span class="hm-badge hm-badge--"#).count();
        assert_eq!(tiles, 6);
        assert_eq!(badges, tiles, "a tile lost its status badge");
    }
}
