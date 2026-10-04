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
//
// Every fact the page states as a number or a list is a constant below, which
// the template renders and the tests pin against the tree. Prose that cannot
// be pinned lives in templates/docs.html; when a change alters what it
// describes, the page changes in the same commit.

/// One row of the sections table. The routes are pinned to the hub's tiles.
struct Section {
    name: &'static str,
    route: &'static str,
    what: &'static str,
    who: &'static str,
}

/// The six sections, in the hub's tile order.
const SECTIONS: &[Section] = &[
    Section {
        name: "Drawing Portfolio",
        route: "/artportfolio",
        what: "A feed of my drawings, grouped by month and filterable by tag or collection.",
        who: "Anyone. Admins upload.",
    },
    Section {
        name: "Drawing Tasks",
        route: "/tasks",
        what: "Practice prompts attached to reference images, sorted by subject, difficulty and type.",
        who: "Anyone can read it. Admins manage it.",
    },
    Section {
        name: "Drinks",
        route: "/drinks",
        what: "Four party games in one room — pick a name and PIN, then join with a room code. Live standings, and a big-screen view for the room.",
        who: "Anyone, after picking a name and PIN.",
    },
    Section {
        name: "Fitness Tracker",
        route: "/fitness",
        what: "A food log with calorie and macro targets, a week view, and barcode scanning in the phone camera.",
        who: "Signed in, and private per person.",
    },
    Section {
        name: "Sorting",
        route: "/sorting",
        what: "Crate sort and van load — a pick checklist, a live loading diagram, and the counts checked against each other.",
        who: "Signed in; built for a tablet.",
    },
    Section {
        name: "CV",
        route: "/cv",
        what: "My CV, in Norwegian, with a print-ready PDF.",
        who: "Anyone.",
    },
];

/// Migration files for the site's database (`migrations/`) and the drinks
/// games' own (`drinkinggame/migrations/`). Pinned to the `.sql` files.
const SITE_MIGRATIONS: usize = 24;
const DRINKS_MIGRATIONS: usize = 3;
const MIGRATIONS: usize = SITE_MIGRATIONS + DRINKS_MIGRATIONS;

/// `cargo test --workspace`'s total and `scripts/verify.sh`'s board checks,
/// as measured on `MEASURED_ON`. Re-measure, never copy from another document.
const WORKSPACE_TESTS: u32 = 1117;
const BOARD_CHECKS: u32 = 458;
const MEASURED_ON: &str = "2026-10-05";
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
