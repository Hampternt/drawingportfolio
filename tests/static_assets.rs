//! Guards on the static assets served from `static/`.
//!
//! These catch the class of breakage that compiles, passes every other test,
//! and still ships a broken page — the failures a code reviewer reads past
//! because they look like prose.
//!
//! CSS comments don't nest: a `/*` inside a comment is inert, so the comment
//! closes at the first `*/` and the leftover text invalidates the NEXT rule,
//! which browsers silently drop. That bit `drinkinggame/assets/game.css` once
//! (commit e99b723 — `.card-big` vanished). The drinking game guards its own
//! served stylesheet in `drinkinggame/tests/http.rs`; this covers the
//! portfolio's `static/*.css`.
//!
//! JS syntax is checked by `scripts/verify.sh` via `node --check` (commit
//! c72d614 — a nested palette entry broke `palette.js`), since a syntax check
//! needs a JS engine.
//!
//! The `?v=` guard exists because nginx serves `/static/` with a one-year
//! `immutable` Cache-Control (`deploy/nginx.conf`): an unversioned CSS/JS
//! link pins returning browsers to whatever file they cached, across
//! deploys, for up to a year (2026-08-18: a phone rendered the new fitness
//! Today templates against the pre-overhaul stylesheet). Every CSS/JS link
//! in a template must therefore carry `?v={{ crate::assets::asset_version() }}`.
//! Font/icon URLs are exempt: they're referenced from inside `style.css`
//! where the version can't reach, and a font preload must keep the exact
//! URL the CSS uses or the browser fetches both.

use std::fs;
use std::path::{Path, PathBuf};

/// Walks `css` as a comment state machine.
///
/// `Err` names the first nested `/*` or an unterminated comment, with a
/// 1-based line number.
fn check_css_comments(css: &str) -> Result<(), String> {
    let bytes = css.as_bytes();
    let mut in_comment = false;
    let mut line = 1usize;
    let mut i = 0;

    while i + 1 < bytes.len() {
        match (&bytes[i..i + 2], in_comment) {
            (b"/*", false) => {
                in_comment = true;
                i += 2;
                continue;
            }
            (b"/*", true) => {
                return Err(format!(
                    "nested /* inside a CSS comment at line {line} — the comment \
                     closes at the next */ and the rule after it is dropped by \
                     the browser"
                ));
            }
            (b"*/", true) => {
                in_comment = false;
                i += 2;
                continue;
            }
            _ => {}
        }
        if bytes[i] == b'\n' {
            line += 1;
        }
        i += 1;
    }

    if in_comment {
        return Err(format!(
            "unterminated CSS comment opened before line {line}"
        ));
    }
    Ok(())
}

#[test]
fn test_static_css_has_no_nested_comment_markers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    let mut checked = 0;

    for entry in fs::read_dir(&dir).expect("static/ is readable") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }
        let css = fs::read_to_string(&path).expect("stylesheet is readable");
        if let Err(problem) = check_css_comments(&css) {
            panic!("{}: {problem}", path.display());
        }
        checked += 1;
    }

    assert!(checked > 0, "no stylesheets found in {}", dir.display());
}

/// The exact shape that broke `game.css`: a `/* screen */` written inside a
/// running comment. Without this fixture the walker above could regress to
/// "always Ok" and the guard would look green forever.
#[test]
fn test_detector_catches_the_game_css_regression() {
    let broken = "/* Playing-card face: rank top-left, suit glyph under it.\n   \
                  Scale is set below in the /* screen */ section. */\n\
                  .card-big { width: 104px; }\n";

    let err = check_css_comments(broken).expect_err("nested /* must be rejected");
    assert!(
        err.contains("line 2"),
        "expected the nested marker's line: {err}"
    );
}

#[test]
fn test_detector_accepts_the_fixed_form() {
    let fixed = "/* Playing-card face: rank top-left, suit glyph under it.\n   \
                 Scale is set below in the \"screen\" section. */\n\
                 .card-big { width: 104px; }\n";

    check_css_comments(fixed).expect("a comment without a nested /* is fine");
}

/// Collects every `.html` file under `dir`, recursively.
fn walk_templates(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("templates dir is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            walk_templates(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            out.push(path);
        }
    }
}

/// Every `/static/*.css` / `/static/*.js` URL in `html`, split into the part
/// before `?` and the query (empty if none).
fn static_asset_refs(html: &str) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    for (start, _) in html.match_indices("/static/") {
        let rest = &html[start..];
        let end = rest.find(['"', '\'']).unwrap_or(rest.len());
        let url = &rest[..end];
        let (path, query) = match url.split_once('?') {
            Some((p, q)) => (p, q),
            None => (url, ""),
        };
        if path.ends_with(".css") || path.ends_with(".js") {
            refs.push((path.to_string(), query.to_string()));
        }
    }
    refs
}

#[test]
fn test_template_asset_links_carry_a_cache_busting_version() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let mut templates = Vec::new();
    walk_templates(&dir, &mut templates);
    assert!(
        !templates.is_empty(),
        "no templates found in {}",
        dir.display()
    );

    let mut checked = 0;
    for template in &templates {
        let html = fs::read_to_string(template).expect("template is readable");
        for (path, query) in static_asset_refs(&html) {
            assert!(
                query.contains("v="),
                "{}: `{path}` has no ?v= — nginx's year-long immutable cache \
                 will pin returning browsers to a stale copy across deploys \
                 (append ?v={{{{ crate::assets::asset_version() }}}})",
                template.display(),
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no /static/ CSS/JS references found — did the link format change?"
    );
}

#[test]
fn test_asset_ref_extractor_sees_the_unversioned_form() {
    let refs = static_asset_refs(r#"<link rel="stylesheet" href="/static/style.css">"#);
    assert_eq!(refs, vec![("/static/style.css".to_string(), String::new())]);

    let refs = static_asset_refs(
        r#"<script src="/static/palette.js?v={{ crate::assets::asset_version() }}" defer></script>"#,
    );
    assert_eq!(refs.len(), 1);
    assert!(refs[0].1.contains("v="));

    // Fonts are exempt — referenced from inside style.css, no version possible.
    assert!(static_asset_refs(r#"<link href="/static/fonts/archivo-800.woff2">"#).is_empty());
}

#[test]
fn test_detector_catches_unterminated_comment() {
    let err = check_css_comments("/* opened and never closed\n.card { color: red; }\n")
        .expect_err("unterminated comment must be rejected");
    assert!(err.contains("unterminated"), "{err}");
}

/// Everything from the sorting section's banner to the end of the file. The
/// section is last on purpose — it is the newest — so "to the end" is the whole
/// of it, and a future section added after it would simply widen what these two
/// guards cover rather than silently narrowing it.
fn sorting_section(css: &str) -> &str {
    let marker = "── Sorting: crate sort and van load";
    let at = css
        .find(marker)
        .expect("style.css still has the sorting section banner");
    &css[at..]
}

/// Drops `/* … */` runs so a guard reads declarations rather than prose. The
/// nesting check above is what guarantees this can be a single pass.
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The sorting screens are sized in rem so a driver who has turned the text
/// size up gets a bigger board rather than the same board with the words
/// spilling out of it. One pixel put back is enough to break that for the
/// control it sizes, and it will look right on the machine it was written on.
///
/// The van diagram is the exception, and it is not really one: inside an
/// `<svg>` a length is a user unit of that element's own viewBox, so `12px`
/// there is a twelfth of the drawing rather than twelve screen pixels, and
/// writing it in rem would peg the labels to the root font size while the
/// drawing kept scaling — the text would grow out of the cells it names.
#[test]
fn test_sorting_section_sizes_in_rem_not_pixels() {
    let css = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("static/style.css"))
        .expect("style.css is readable");
    let section = strip_css_comments(sorting_section(&css));

    let mut checked = 0;
    for rule in section.split('}') {
        let Some((selector, body)) = rule.split_once('{') else {
            continue;
        };
        if selector.contains("sort-vansvg") {
            continue;
        }
        checked += 1;
        assert!(
            !body.contains("px"),
            "a pixel length is back in the sorting section:\n  {}{{{}}}\n\
             sizes here are rem — see the section banner in static/style.css. \
             Inside the van diagram (`.sort-vansvg…`) px is an SVG user unit \
             and is allowed; everywhere else divide by 16.",
            selector.trim(),
            body.trim(),
        );
    }
    assert!(
        checked > 10,
        "only {checked} rules scanned — did the sorting section move?"
    );
}

/// The board's landscape layout is a CSS media query; which pane is even
/// selectable at that width is a `matchMedia` in `sorting.js`. They describe
/// one moment — the van stops being a tab and takes its own column — and
/// nothing but agreement between two files makes it one moment. Drift shows the
/// van twice, or hides its tab on a screen still laying the van out as a tab.
///
/// The unit matters as much as the number: a media query resolves rem against
/// the browser's default text size, so a driver who has turned that up moves
/// the breakpoint, and `62.5rem` and `1000px` stop being the same width for
/// exactly the person the rem is there for.
#[test]
fn test_sorting_breakpoint_agrees_between_css_and_js() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let css = fs::read_to_string(root.join("static/style.css")).expect("style.css is readable");
    let js = fs::read_to_string(root.join("static/sorting.js")).expect("sorting.js is readable");

    let after = |hay: &str, needle: &str, what: &str| -> String {
        let at = hay
            .find(needle)
            .unwrap_or_else(|| panic!("{what}: `{needle}` not found — did the format change?"));
        let tail = &hay[at + needle.len()..];
        let end = tail.find(')').expect("the width is closed by a paren");
        tail[..end].trim().to_string()
    };

    let in_css = after(
        &strip_css_comments(sorting_section(&css)),
        "@media (min-width:",
        "static/style.css",
    );
    let in_js = after(&js, "matchMedia('(min-width:", "static/sorting.js");

    assert_eq!(
        in_css, in_js,
        "the sorting breakpoint disagrees: style.css switches the van into its \
         own column at {in_css}, sorting.js stops offering the van tab at \
         {in_js}. Between the two widths the board lays the van out twice or \
         not at all."
    );
}
