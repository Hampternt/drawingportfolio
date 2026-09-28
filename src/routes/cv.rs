use crate::{middleware::OptionalAdmin, AppState};
use askama::Template;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

// The CV is static: every word of it lives in templates/cv.html, so a finished
// version replaces the template and never touches this file. `is_admin` is
// only here because base.html reads it for the header's settings button.
#[derive(Template)]
#[template(path = "cv.html")]
struct CvTemplate {
    is_admin: bool,
}

async fn cv_page(OptionalAdmin(is_admin): OptionalAdmin) -> impl IntoResponse {
    Html(CvTemplate { is_admin }.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/cv", get(cv_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render() -> String {
        CvTemplate { is_admin: false }.render().unwrap()
    }

    #[test]
    fn test_cv_carries_the_site_dark_marker_and_lights_its_nav_link() {
        let html = render();
        // base.html's syncBodyTheme derives body.site-dark from this marker on
        // every boosted navigation; without it the page renders unstyled.
        assert!(html.contains(r#"class="site-page cv-page""#));
        assert!(html.contains(r#"data-active="cv""#));
        assert!(html.contains(r#"<a href="/cv">CV</a>"#));
    }

    #[test]
    fn test_cv_publishes_no_phone_number_postcode_or_placeholder() {
        let html = render();
        // Ruled 2026-09-25: a public page carries the email and GitHub only.
        assert!(html.contains("mailto:jl@dblo.net"));
        // Checked by shape, never by value: a test that names the number
        // publishes it in the repo it is guarding.
        assert!(!html.contains("+47"), "public CV carries a country code");
        let main = &html[html.find("<main>").unwrap()..html.find("</main>").unwrap()];
        let digits: String = main.chars().filter(|c| *c != ' ').collect();
        let longest_run = digits
            .split(|c: char| !c.is_ascii_digit())
            .map(str::len)
            .max()
            .unwrap_or(0);
        assert!(longest_run < 8, "public CV carries a phone-length number");
        let contact = &main[main.find("cv-contact").unwrap()..];
        let contact = &contact[..contact.find("</div>").unwrap()];
        assert!(
            !contact.chars().any(|c| c.is_ascii_digit()),
            "the contact row carries a number (postcode or phone)"
        );
        // The handoff's amber blanks must never reach an employer.
        for blank in ["[ARBEIDSGIVER]", "[ÅRSTALL]"] {
            assert!(!html.contains(blank), "public CV ships {blank:?}");
        }
        assert!(html.contains("Matvare-Expressen"));
    }

    #[test]
    fn test_cv_content_links_nowhere_on_this_site() {
        // Ruled 2026-09-28: the CV's own content links nowhere on this site, so
        // the rail's how-it-works card is dropped for good. Only <main> is
        // checked — the header nav on /cv is site chrome and gains /docs later.
        let html = render();
        // Positive control: the header does carry site links, so the needle
        // below would match if <main> ever did.
        assert!(html.contains(r#"href="/"#));
        let main = &html[html.find("<main>").unwrap()..html.find("</main>").unwrap()];
        assert!(main.contains("mailto:jl@dblo.net"), "<main> lost the contact row");
        assert!(!main.contains(r#"href="/"#), "the CV links into the site");
        assert!(!main.contains("portfolio.dblo.net"), "the CV names the site");
    }
}
