use crate::{middleware::OptionalAdmin, AppState};
use askama::Template;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

// How this site works: a static explainer for a reader who does not code. It
// reads nothing, so there is no db call and no migration. `is_admin` is only
// here because base.html reads it for the header's settings button.
#[derive(Template)]
#[template(path = "docs.html")]
struct DocsTemplate {
    is_admin: bool,
}

async fn docs_page(OptionalAdmin(is_admin): OptionalAdmin) -> impl IntoResponse {
    Html(DocsTemplate { is_admin }.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/docs", get(docs_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render() -> String {
        DocsTemplate { is_admin: false }.render().unwrap()
    }

    #[test]
    fn test_docs_carries_the_site_dark_marker_and_lights_its_nav_link() {
        let html = render();
        // base.html's syncBodyTheme derives body.site-dark from this marker on
        // every boosted navigation; without it the page renders unstyled.
        assert!(html.contains(r#"class="site-page docs-page""#));
        assert!(html.contains(r#"data-active="docs""#));
        assert!(html.contains(r#"<a href="/docs">How it works</a>"#));
    }
}
