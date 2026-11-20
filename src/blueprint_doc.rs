//! Blueprints: what a plan document is, on the server.
//!
//! The editor's model is `static/blueprint-model.js`. This is its server-side
//! twin, and it does three things with a plan:
//!
//! - **Validate** a save. A document that cannot be read is refused with a
//!   400 that names each problem. A dangling wire or a type mismatch is not a
//!   problem: a plan is allowed to be unfinished, so those are counted and
//!   reported, never refused.
//! - **Count** it, for the save response and the outline's headline.
//! - **Outline** it, for the Askama view beside the canvas (the no-JS and
//!   screen-reader view).
//!
//! It never rewrites a document. The route stores the text exactly as it
//! arrived, and parsing here keeps key order ([`IndexMap`]), because a port's
//! place among its keys is its row on the node.
//!
//! The two validators must agree on every document, or the editor saves plans
//! the server refuses. `tests/fixtures/blueprint-validation.json` holds the
//! cases both of them run.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STATUSES: [&str; 4] = ["idea", "planned", "building", "done"];
pub const KINDS: [&str; 2] = ["machine", "blueprint"];

/// The largest plan a save may send. The demo is about 4 KB; a 200-machine
/// plan is about 60 KB.
pub const MAX_PAYLOAD: usize = 1024 * 1024;

/// The names every JavaScript object already answers to. The editor keeps a
/// plan in plain objects, where `nodes.constructor` exists before anyone adds
/// it and `nodes.__proto__ = …` replaces the prototype, so both sides refuse
/// them as names. The JS test checks the list against the engine.
const RESERVED: [&str; 12] = [
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "__proto__",
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
];

/// A node, port or graph name: a letter or `_`, then up to 63 letters,
/// digits, `_` or `-`. No `.` (it splits a wire into node and port), no `/`
/// (it joins a layout key), and never a leading digit, which JavaScript would
/// move to the front of an object and so to the top row of a node.
pub fn is_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
        && !RESERVED.contains(&s)
}

/// A project or plan name: lower case, digits and dashes, starting with a
/// letter or digit. It is the name the AI will ask for.
pub fn is_slug(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Splits a wire end at its first `.`.
fn split(r: &str) -> (&str, &str) {
    r.split_once('.').unwrap_or((r, ""))
}

fn is_wire_end(v: &Value) -> bool {
    match v.as_str() {
        Some(r) => {
            let (node, port) = split(r);
            r.contains('.') && is_name(node) && is_name(port)
        }
        None => false,
    }
}

/// Every reason a document cannot be read, one sentence each. Empty means it
/// is a plan. Mirrors `validate()` in `static/blueprint-model.js`.
pub fn validate(doc: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(top) = doc.as_object() else {
        return vec!["the plan is not a JSON object".into()];
    };
    if top.get("v").and_then(Value::as_f64) != Some(1.0) {
        errs.push(r#""v" must be 1"#.into());
    }
    let Some(graphs) = top.get("graphs").and_then(Value::as_object) else {
        errs.push(r#""graphs" must be an object"#.into());
        return errs;
    };
    match top.get("root").and_then(Value::as_str) {
        Some(root) if graphs.contains_key(root) => {}
        _ => errs.push(r#""root" must name a graph"#.into()),
    }
    for (g, graph) in graphs {
        if !is_name(g) {
            errs.push(format!(r#"graph "{g}": not a valid name"#));
        }
        let Some(graph) = graph.as_object() else {
            errs.push(format!(r#"graph "{g}" must be an object"#));
            continue;
        };
        match graph.get("nodes").and_then(Value::as_object) {
            None => errs.push(format!(r#"graph "{g}": "nodes" must be an object"#)),
            Some(nodes) => {
                for (name, n) in nodes {
                    validate_node(graphs, g, name, n, &mut errs);
                }
            }
        }
        match graph.get("wires").and_then(Value::as_array) {
            None => errs.push(format!(r#"graph "{g}": "wires" must be a list"#)),
            Some(wires) => {
                for (i, w) in wires.iter().enumerate() {
                    let good = w
                        .as_array()
                        .is_some_and(|pair| pair.len() == 2 && pair.iter().all(is_wire_end));
                    if !good {
                        errs.push(format!(
                            r#"graph "{g}": wire {i} is not a pair of "node.port""#
                        ));
                    }
                }
            }
        }
    }
    if let Some(layout) = top.get("layout") {
        match layout.as_object() {
            None => errs.push(r#""layout" must be an object"#.into()),
            Some(layout) => {
                for (k, p) in layout {
                    let good = p
                        .as_array()
                        .is_some_and(|xy| xy.len() == 2 && xy.iter().all(Value::is_number));
                    if !good {
                        errs.push(format!(r#"layout "{k}" must be [x, y]"#));
                    }
                }
            }
        }
    }
    errs
}

fn validate_node(
    graphs: &serde_json::Map<String, Value>,
    g: &str,
    name: &str,
    n: &Value,
    errs: &mut Vec<String>,
) {
    let at = format!("{g}/{name}");
    if !is_name(name) {
        errs.push(format!("node {at}: not a valid name"));
    }
    let Some(n) = n.as_object() else {
        errs.push(format!("node {at} must be an object"));
        return;
    };
    let kind = n.get("kind").and_then(Value::as_str);
    if !kind.is_some_and(|k| KINDS.contains(&k)) {
        errs.push(format!("node {at}: unknown kind {}", show(n.get("kind"))));
    }
    // Absent is fine; present (null included) must be one of the four.
    if let Some(status) = n.get("status") {
        if !status.as_str().is_some_and(|s| STATUSES.contains(&s)) {
            errs.push(format!("node {at}: unknown status {status}"));
        }
    }
    if n.get("note").is_some_and(|note| !note.is_string()) {
        errs.push(format!(r#"node {at}: "note" must be text"#));
    }
    for side in ["in", "out"] {
        let Some(ports) = n.get(side) else { continue };
        let Some(ports) = ports.as_object() else {
            errs.push(format!(r#"node {at}: "{side}" must be an object"#));
            continue;
        };
        for (p, t) in ports {
            if !is_name(p) {
                errs.push(format!(
                    r#"node {at}: {side} port "{p}" is not a valid name"#
                ));
            }
            if !t.is_string() {
                errs.push(format!(r#"node {at}: {side} port "{p}" needs a text type"#));
            }
        }
    }
    if kind == Some("blueprint") {
        let opens = n.get("graph").and_then(Value::as_str);
        if !opens.is_some_and(|sub| graphs.contains_key(sub)) {
            errs.push(format!(
                "node {at}: blueprint names a missing graph {}",
                show(n.get("graph"))
            ));
        }
    }
}

fn show(v: Option<&Value>) -> String {
    v.map(Value::to_string).unwrap_or_else(|| "(none)".into())
}

// ── The parsed plan ─────────────────────────────────────────────────────────

/// A plan that passed [`validate`], parsed with its key order kept. Fields the
/// server does not read (layout, anything an AI adds) are not modelled here;
/// they stay in the stored text untouched.
#[derive(Debug, Deserialize)]
pub struct Doc {
    pub root: String,
    pub graphs: IndexMap<String, Graph>,
}

#[derive(Debug, Deserialize)]
pub struct Graph {
    pub nodes: IndexMap<String, Node>,
    pub wires: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// Read only for a blueprint. A machine may carry anything here.
    #[serde(default)]
    pub graph: Option<Value>,
    #[serde(rename = "in", default)]
    pub ins: IndexMap<String, String>,
    #[serde(default)]
    pub out: IndexMap<String, String>,
}

/// Parses and validates a plan's text. `Err` carries every problem found.
pub fn parse(text: &str) -> Result<Doc, Vec<String>> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| vec![format!("not valid JSON: {e}")])?;
    let errs = validate(&value);
    if !errs.is_empty() {
        return Err(errs);
    }
    // Validation has already checked every field this reads. This second pass
    // (the one that keeps key order) is stricter in one way: a key repeated in
    // one object is refused here, where the first pass let the last one win.
    // JSON.stringify never repeats a key, so the editor cannot hit it.
    serde_json::from_str(text).map_err(|e| vec![format!("not a plan: {e}")])
}

// ── Checks and counts ───────────────────────────────────────────────────────

/// Case and whitespace are folded before two type labels are compared
/// (decision 2026-11-10, open issue 6).
pub fn type_key(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// One wire and what is wrong with it, as `checkGraph()` reports it.
#[derive(Debug)]
pub struct WireCheck {
    pub a: String,
    pub b: String,
    /// The output's type, if that end resolves.
    pub from_type: Option<String>,
    /// The input's type, if that end resolves.
    pub to_type: Option<String>,
    /// Why the wire dangles; empty when it does not.
    pub why: String,
    pub mismatch: bool,
}

impl WireCheck {
    pub fn dangling(&self) -> bool {
        !self.why.is_empty()
    }
}

pub fn check_graph(graph: &Graph) -> Vec<WireCheck> {
    graph
        .wires
        .iter()
        .map(|(a, b)| {
            let (an, ap) = split(a);
            let (bn, bp) = split(b);
            let na = graph.nodes.get(an);
            let nb = graph.nodes.get(bn);
            let ft = na.and_then(|n| n.out.get(ap)).cloned();
            let tt = nb.and_then(|n| n.ins.get(bp)).cloned();
            let why = if na.is_none() {
                format!(r#"no node "{an}""#)
            } else if ft.is_none() {
                format!(r#"no out port "{a}""#)
            } else if nb.is_none() {
                format!(r#"no node "{bn}""#)
            } else if tt.is_none() {
                format!(r#"no in port "{b}""#)
            } else {
                String::new()
            };
            let mismatch = match (&ft, &tt) {
                (Some(f), Some(t)) => why.is_empty() && type_key(f) != type_key(t),
                _ => false,
            };
            WireCheck {
                a: a.clone(),
                b: b.clone(),
                from_type: ft,
                to_type: tt,
                why,
                mismatch,
            }
        })
        .collect()
}

/// The root first, then the other graphs in document order.
pub fn graph_order(doc: &Doc) -> Vec<&str> {
    let mut order = vec![doc.root.as_str()];
    order.extend(
        doc.graphs
            .keys()
            .map(String::as_str)
            .filter(|g| *g != doc.root),
    );
    order
}

/// The whole plan's counts, every graph added up: the header's line and what
/// a save is answered with. Matches `summarizeDoc()`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub nodes: usize,
    pub wires: usize,
    pub dangling: usize,
    pub mismatch: usize,
}

pub fn summarize(doc: &Doc) -> Summary {
    let mut s = Summary::default();
    for g in graph_order(doc) {
        let graph = &doc.graphs[g];
        s.nodes += graph.nodes.len();
        for w in check_graph(graph) {
            s.wires += 1;
            s.dangling += usize::from(w.dangling());
            s.mismatch += usize::from(w.mismatch);
        }
    }
    s
}

impl Summary {
    /// "8 machines · 12 wires · 1 dangling · 1 mismatch".
    pub fn line(&self) -> String {
        format!(
            "{} {} · {} {} · {} dangling · {} mismatch",
            self.nodes,
            if self.nodes == 1 {
                "machine"
            } else {
                "machines"
            },
            self.wires,
            if self.wires == 1 { "wire" } else { "wires" },
            self.dangling,
            self.mismatch
        )
    }
}

// ── The outline ─────────────────────────────────────────────────────────────
//
// What the Askama outline draws, computed here so the template only loops and
// prints. It lists every graph, root first, each node with its ports and
// where its outputs go, then the wires with what is wrong with them.

pub struct Outline {
    pub summary: Summary,
    pub graphs: Vec<OutlineGraph>,
}

pub struct OutlineGraph {
    pub title: String,
    pub sub: String,
    pub nodes: Vec<OutlineNode>,
    pub wires: Vec<OutlineWire>,
}

pub struct OutlineNode {
    pub name: String,
    /// The status, or "—" when the node has none.
    pub status: String,
    /// `idea` … `done`, or `none`: the badge's modifier class.
    pub status_class: String,
    /// "machine", or "blueprint → db".
    pub kind: String,
    pub outs: String,
    pub ins: String,
    pub note: String,
}

pub struct OutlineWire {
    pub text: String,
    pub flag: String,
    /// `dangling`, `mismatch` or empty: the flag's modifier class.
    pub flag_class: String,
}

fn opener_of(doc: &Doc, g: &str) -> Option<String> {
    doc.graphs.iter().find_map(|(pg, graph)| {
        graph.nodes.iter().find_map(|(name, n)| {
            (n.kind == "blueprint" && n.graph.as_ref().and_then(Value::as_str) == Some(g))
                .then(|| format!("{pg}/{name}"))
        })
    })
}

pub fn outline(doc: &Doc) -> Outline {
    let graphs = graph_order(doc)
        .into_iter()
        .map(|g| {
            let graph = &doc.graphs[g];
            let checks = check_graph(graph);
            let nodes = graph
                .nodes
                .iter()
                .map(|(name, n)| {
                    let outs: Vec<String> = n
                        .out
                        .iter()
                        .map(|(k, t)| {
                            let from = format!("{name}.{k}");
                            let to: Vec<&str> = checks
                                .iter()
                                .filter(|w| w.a == from)
                                .map(|w| w.b.as_str())
                                .collect();
                            if to.is_empty() {
                                format!("{k}: {t}")
                            } else {
                                format!("{k}: {t} → {}", to.join(", "))
                            }
                        })
                        .collect();
                    let ins: Vec<String> = n.ins.iter().map(|(k, t)| format!("{k}: {t}")).collect();
                    let kind = if n.kind == "blueprint" {
                        let sub = n.graph.as_ref().and_then(Value::as_str).unwrap_or("?");
                        format!("blueprint → {sub}")
                    } else {
                        n.kind.clone()
                    };
                    OutlineNode {
                        name: name.clone(),
                        status: n.status.clone().unwrap_or_else(|| "—".into()),
                        status_class: n.status.clone().unwrap_or_else(|| "none".into()),
                        kind,
                        outs: if outs.is_empty() {
                            "—".into()
                        } else {
                            outs.join("; ")
                        },
                        ins: if ins.is_empty() {
                            "—".into()
                        } else {
                            ins.join(", ")
                        },
                        note: n.note.clone().unwrap_or_default(),
                    }
                })
                .collect();
            let wires = checks
                .iter()
                .map(|w| {
                    let (flag, flag_class) = if w.dangling() {
                        (format!("dangling: {}", w.why), "dangling")
                    } else if w.mismatch {
                        (
                            format!(
                                "mismatch: {} ≠ {}",
                                w.from_type.as_deref().unwrap_or(""),
                                w.to_type.as_deref().unwrap_or("")
                            ),
                            "mismatch",
                        )
                    } else {
                        (String::new(), "")
                    };
                    OutlineWire {
                        text: format!("{} → {}", w.a, w.b),
                        flag,
                        flag_class: flag_class.into(),
                    }
                })
                .collect();
            let (title, sub) = if g == doc.root {
                (format!("{g} (root)"), "the plan’s top level".to_string())
            } else {
                (
                    g.to_string(),
                    format!(
                        "opened by {}",
                        opener_of(doc, g).unwrap_or_else(|| "nothing".into())
                    ),
                )
            };
            OutlineGraph {
                title,
                sub,
                nodes,
                wires,
            }
        })
        .collect();
    Outline {
        summary: summarize(doc),
        graphs,
    }
}

/// The document a new plan starts as: one empty root graph.
pub const EMPTY_PLAN: &str =
    r#"{"v":1,"root":"main","graphs":{"main":{"nodes":{},"wires":[]}},"layout":{}}"#;

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = include_str!("../docs/design/blueprint-planner/demo-plan.json");
    const FIXTURES: &str = include_str!("../tests/fixtures/blueprint-validation.json");

    /// The same cases `tests/js/blueprint-model.test.js` runs. If the two
    /// validators disagree on one, the editor saves plans the server refuses.
    #[test]
    fn test_validator_agrees_with_the_shared_fixtures() {
        let fix: Value = serde_json::from_str(FIXTURES).unwrap();
        let cases = fix["cases"].as_array().unwrap();
        assert!(cases.len() >= 40, "only {} cases", cases.len());
        let (mut accepted, mut refused) = (0, 0);
        for c in cases {
            let name = c["name"].as_str().unwrap();
            let text = if c["doc"] == "demo" {
                DEMO.to_string()
            } else {
                c["doc"].to_string()
            };
            match (c["ok"].as_bool().unwrap(), parse(&text)) {
                (true, Ok(doc)) => {
                    let want: Summary = serde_json::from_value(c["summary"].clone()).unwrap();
                    assert_eq!(summarize(&doc), want, "counts: {name}");
                    accepted += 1;
                }
                (true, Err(e)) => panic!("refused {name}: {e:?}"),
                (false, Ok(_)) => panic!("accepted {name}"),
                (false, Err(e)) => {
                    assert!(!e.is_empty(), "{name}");
                    refused += 1;
                }
            }
        }
        assert!(accepted > 0 && refused > 0);
    }

    #[test]
    fn test_parse_keeps_key_order() {
        // serde_json's own map would sort these: "ai" < "api" < "page".
        let doc = parse(DEMO).unwrap();
        let router = &doc.graphs["main"].nodes["router"];
        assert_eq!(
            router.out.keys().collect::<Vec<_>>(),
            ["page", "api", "ai"],
            "ports came back in another order, so they would change rows"
        );
        assert_eq!(
            doc.graphs["main"].nodes.keys().next().map(String::as_str),
            Some("nginx")
        );
    }

    #[test]
    fn test_not_json_is_one_clear_error() {
        let e = parse("{ nope").unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(e[0].starts_with("not valid JSON"), "{e:?}");
    }

    #[test]
    fn test_every_problem_is_reported_not_just_the_first() {
        let e = parse(
            r#"{"v":1,"root":"main","graphs":{"main":{"nodes":{
                "a":{"kind":"source"},"b":{"kind":"machine","status":"blocked"}},"wires":[["a","b.x"]]}}}"#,
        )
        .unwrap_err();
        assert_eq!(e.len(), 3, "{e:?}");
        assert!(e.iter().any(|m| m.contains("unknown kind")));
        assert!(e.iter().any(|m| m.contains("unknown status")));
        assert!(e.iter().any(|m| m.contains("wire 0")));
    }

    #[test]
    fn test_names_and_slugs() {
        for good in ["a", "_x", "token-gate_2", &"A".repeat(64)] {
            assert!(is_name(good), "{good}");
        }
        for bad in [
            "",
            "2fa",
            "a.b",
            "a/b",
            "a b",
            "ä",
            "constructor",
            "__proto__",
            &"A".repeat(65),
        ] {
            assert!(!is_name(bad), "{bad}");
        }
        for good in ["drawingportfolio", "request-flow", "s3", "2026-plans"] {
            assert!(is_slug(good), "{good}");
        }
        for bad in [
            "",
            "Request",
            "-x",
            "a_b",
            "a b",
            "a.b",
            "a/b",
            &"a".repeat(65),
        ] {
            assert!(!is_slug(bad), "{bad}");
        }
    }

    #[test]
    fn test_type_key_folds_case_and_whitespace() {
        assert_eq!(type_key("Vec<Post>"), type_key("vec < post >"));
        assert_eq!(type_key(" Html\t"), "html");
        assert_ne!(type_key("Request"), type_key("Header"));
        // Unicode: White_Space is dropped (U+0085 is, U+FEFF is not) and each
        // code point is lowercased alone (no final-sigma rule). The JS side
        // folds the same way; the shared fixtures pin both.
        assert_eq!(type_key("ΑΣ"), type_key("ασ"));
        assert_eq!(type_key("Html\u{85}"), "html");
        assert_ne!(type_key("Html\u{feff}"), "html");
    }

    #[test]
    fn test_outline_of_the_demo() {
        let o = outline(&parse(DEMO).unwrap());
        assert_eq!(
            o.summary.line(),
            "11 machines · 14 wires · 1 dangling · 1 mismatch"
        );
        assert_eq!(
            o.graphs
                .iter()
                .map(|g| g.title.as_str())
                .collect::<Vec<_>>(),
            ["main (root)", "db"]
        );
        assert_eq!(o.graphs[1].sub, "opened by main/db");
        let router = o.graphs[0]
            .nodes
            .iter()
            .find(|n| n.name == "router")
            .unwrap();
        assert_eq!(
            router.outs,
            "page: Request → session.req, handler.req; api: Request → session.req; ai: Request → token_gate.bearer"
        );
        assert_eq!(router.ins, "req: Request");
        let db = o.graphs[0].nodes.iter().find(|n| n.name == "db").unwrap();
        assert_eq!(db.kind, "blueprint → db");
        let flags: Vec<(&str, &str)> = o.graphs[0]
            .wires
            .iter()
            .filter(|w| !w.flag.is_empty())
            .map(|w| (w.text.as_str(), w.flag.as_str()))
            .collect();
        assert_eq!(
            flags,
            [
                (
                    "router.ai → token_gate.bearer",
                    "mismatch: Request ≠ Header"
                ),
                (
                    "exporter.outline → claude.context",
                    r#"dangling: no node "claude""#
                ),
            ]
        );
    }

    #[test]
    fn test_empty_plan_is_a_valid_plan() {
        let doc = parse(EMPTY_PLAN).unwrap();
        assert_eq!(summarize(&doc), Summary::default());
    }
}
