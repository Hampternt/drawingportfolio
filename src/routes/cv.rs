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
        for leaked in ["405 50 447", "+47", "4042"] {
            assert!(!html.contains(leaked), "public CV leaks {leaked:?}");
        }
        // The handoff's amber blanks must never reach an employer.
        for blank in ["[ARBEIDSGIVER]", "[ÅRSTALL]"] {
            assert!(!html.contains(blank), "public CV ships {blank:?}");
        }
        assert!(html.contains("Matvare-Expressen"));
    }

    #[test]
    fn test_cv_links_nowhere_that_does_not_exist_yet() {
        // The how-it-works page is Pack 7; until its route exists the rail's
        // link card to it stays out, same rule as the hub footer.
        let html = render();
        assert!(!html.contains(r#"href="/docs""#));
        assert!(!html.contains(r#"href="/how-it-works""#));
    }
}
