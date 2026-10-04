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
        what: "Practice prompts attached to reference images, filterable by subject, difficulty and type.",
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

/// The owner's statement on how AI is used, word for word from
/// docs/handoffs/2026-09-27-portfolio-review.md §"The AI statement", with its
/// two bold phrases as <strong>. The page renders it and README.md must carry
/// the same sentences; a test holds both to this one copy.
const AI_STATEMENT: &str = "<strong>How I use AI.</strong> I design the solution first \
— the data model, the boundaries, the failure cases, what has to be tested — then hand \
Claude Code a precise brief to implement it. Because I know the technology and where it \
usually goes wrong, the brief names the right constraints up front, so the first pass \
lands close and there are few rounds of correction. That keeps both cycle time and token \
cost low. <strong>Review is scaled to risk:</strong> I weigh how critical a change is and \
how likely it is to go wrong. Routine, low-stakes work gets a quick check; anything that \
reaches live users, touches security or data, or is complex enough to hide errors gets a \
thorough review — often of the plan, before any code is written.";

/// `cargo test --workspace`'s total and `scripts/verify.sh`'s board checks,
/// as measured on `MEASURED_ON`. Re-measure, never copy from another document.
const WORKSPACE_TESTS: u32 = 1123;
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

    fn main_of(html: &str) -> &str {
        &html[html.find("<main>").unwrap()..html.find("</main>").unwrap()]
    }

    /// The hrefs of the hub's tiles, in order, found by the same needle
    /// `hub.rs` counts them with.
    fn hub_tile_routes() -> Vec<&'static str> {
        const HUB: &str = include_str!("../../templates/hub/hub.html");
        let needle = r#"class="hm-hub""#;
        let mut routes = Vec::new();
        for (at, _) in HUB.match_indices(needle) {
            let tag = &HUB[HUB[..at].rfind("<a ").unwrap()..at];
            let href = &tag[tag.find(r#"href=""#).unwrap() + 6..];
            routes.push(&href[..href.find('"').unwrap()]);
        }
        routes
    }

    #[test]
    fn test_docs_sections_are_the_hubs_tiles() {
        let tiles = hub_tile_routes();
        // Positive control: the needle finds the hub's tiles at all.
        assert!(tiles.contains(&"/cv"), "no hub tiles found: {tiles:?}");
        let rows: Vec<&str> = SECTIONS.iter().map(|s| s.route).collect();
        assert_eq!(rows, tiles, "the sections table drifted from the hub");

        let html = render();
        let main = main_of(&html);
        assert!(main.contains(&format!(
            r#"<span class="docs-stat__value">{}</span>"#,
            tiles.len()
        )));
        for s in SECTIONS {
            let route = format!(r#"<span class="docs-table__route">{}</span>"#, s.route);
            assert_eq!(main.matches(&route).count(), 1, "table row for {}", s.route);
        }
        // The lede states the count in words; six is what the hub has.
        assert_eq!(tiles.len(), 6, "the lede says six sections");
        assert!(main.contains("six sections, one program, one server"));
    }

    #[test]
    fn test_docs_migration_figure_counts_both_databases_sql_files() {
        // Read from the tree in the test only, never at runtime: the deployed
        // binary has no source tree beside it.
        fn sql_files(dir: &str) -> usize {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
            std::fs::read_dir(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .filter(|e| {
                    e.as_ref()
                        .unwrap()
                        .path()
                        .extension()
                        .is_some_and(|x| x == "sql")
                })
                .count()
        }
        assert_eq!(SITE_MIGRATIONS, sql_files("migrations"));
        assert_eq!(DRINKS_MIGRATIONS, sql_files("drinkinggame/migrations"));
        let html = render();
        assert!(main_of(&html).contains(&format!(
            r#"<span class="docs-stat__value">{MIGRATIONS}</span>"#
        )));
    }

    #[test]
    fn test_docs_ships_none_of_the_mocks_false_claims() {
        let html = render();
        let main = main_of(&html).to_lowercase();
        // The manifest's false-claims table, plus three more the build found.
        for claim in [
            "nothing to compile before a change goes live",
            "no build step",
            ">build step<",
            "four steps, all on the same server",
            "nothing is fetched from someone else's service",
            "five sections",
            "1105",
            "anyone with the room code",
            "only i can upload",
            "one stylesheet.",
            "one header, one stylesheet and one sign-in",
            "one dark theme",
            "no light mode",
            "i built and run everything",
            "serves cached files directly",
            "nothing else exists",
            "before a page could have loaded",
        ] {
            assert!(!main.contains(claim), "/docs claims {claim:?}");
        }
        // Positive control: the needles are matched against real copy.
        assert!(main.contains("no front-end build step"));
        assert!(main.contains("anyone, after picking a name and pin."));
        assert!(main.contains("admins upload."));
    }

    #[test]
    fn test_docs_publishes_no_phone_number() {
        let html = render();
        // Checked by shape, never by value, and only inside <main>: base.html's
        // `?v=` hashes are hex and would trip a digit-run check on the head.
        let main = main_of(&html);
        assert!(!main.contains("+47"), "/docs carries a country code");
        let digits: String = main.chars().filter(|c| *c != ' ').collect();
        let longest_run = digits
            .split(|c: char| !c.is_ascii_digit())
            .map(str::len)
            .max()
            .unwrap_or(0);
        assert!(longest_run < 8, "/docs carries a phone-length number");
    }

    #[test]
    fn test_docs_and_readme_carry_the_same_ai_statement() {
        fn plain(text: &str) -> String {
            text.replace("<strong>", "")
                .replace("</strong>", "")
                .replace("**", "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        }
        let html = render();
        assert!(html.contains(AI_STATEMENT), "/docs lost the AI statement");
        assert!(AI_STATEMENT.contains("<strong>How I use AI.</strong>"));
        assert!(AI_STATEMENT.contains("<strong>Review is scaled to risk:</strong>"));

        let readme = plain(include_str!("../../README.md"));
        let statement = plain(AI_STATEMENT);
        // Sentence by sentence, so a failure names the one that drifted.
        for sentence in statement.split_inclusive(['.', ':', ';']) {
            let sentence = sentence.trim();
            assert!(readme.contains(sentence), "README lacks {sentence:?}");
        }
        assert!(readme.contains(&statement));
        // The bold phrases are bold there too.
        let raw = include_str!("../../README.md");
        assert!(raw.contains("**How I use AI.**"));
        assert!(raw.contains("**Review is scaled to risk:**"));
    }

    #[test]
    fn test_docs_renders_the_pin_lockout_from_crate_pin() {
        let html = render();
        assert!(html.contains(&format!(
            "{} wrong ones lock that account for {} minutes",
            crate::pin::MAX_PIN_ATTEMPTS,
            crate::pin::LOCKOUT_MINUTES
        )));
    }
}
