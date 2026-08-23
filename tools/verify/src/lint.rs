//! Schema lint (SCHEMA.md conformance): cross-checks a fault folder's card frontmatter against
//! its CXF document, the point dictionary, clusters, playbooks, vectors, and the engine pin.
//! Behavioral correctness is the scenario runner's job; this catches the referential mistakes.

use std::collections::BTreeSet;
use std::path::Path;

use crate::point_resolution::PointResolver;

const CARD_SCHEMA: &str = "cxf-library/fault-card/v1";
const STATUSES: &[&str] = &["draft", "verified", "adopted", "deprecated"];
const METHODS: &[&str] = &["rule", "statistical", "ml", "meta"];
const CATEGORIES: &[&str] = &[
    "CRITICAL_WASTE",
    "EFFICIENCY_LOSS",
    "EXCESS_CONSUMPTION",
    "COMFORT_ENERGY",
    "PROTECTIVE",
];

pub struct LintReport {
    pub errors: Vec<String>,
    /// The card's declared status (used by main to decide whether content-id equality is required).
    pub status: String,
    pub recorded_content_id: Option<String>,
    pub recorded_engine_rev: Option<String>,
}

fn yaml_str(v: &serde_yaml::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|s| s.as_str()).map(str::to_string)
}

fn yaml_str_list(v: &serde_yaml::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|l| l.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract YAML frontmatter (between the leading `---` fence pair) and the remaining body.
fn split_frontmatter(card: &str) -> Option<(&str, &str)> {
    let rest = card.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some((&rest[..end], &rest[end + 5..]))
}

/// Local name of an IRI-path node relative to the root: the segment after `<root>.`.
fn local_name<'a>(id: &'a str, root_id: &str) -> Option<&'a str> {
    id.strip_prefix(root_id)?.strip_prefix('.')
}

fn id_refs(node: &serde_json::Value, key: &str) -> Vec<String> {
    match node.get(key) {
        None => Vec::new(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.get("@id").and_then(|s| s.as_str()).map(str::to_string))
            .collect(),
        Some(one) => one
            .get("@id")
            .and_then(|s| s.as_str())
            .map(str::to_string)
            .into_iter()
            .collect(),
    }
}

pub fn lint_fault_dir(dir: &Path, repo_root: &Path) -> Result<LintReport, String> {
    let mut errors = Vec::new();
    let fault_id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad fault dir name")?
        .to_string();
    let equip_dir = dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .ok_or("bad equipment dir")?
        .to_string();

    // --- card.md frontmatter ---
    let card_path = dir.join("card.md");
    let card = std::fs::read_to_string(&card_path).map_err(|e| format!("card.md: {e}"))?;
    let (fm_text, body) =
        split_frontmatter(&card).ok_or("card.md: missing YAML frontmatter fences")?;
    let fm: serde_yaml::Value =
        serde_yaml::from_str(fm_text).map_err(|e| format!("card.md frontmatter: {e}"))?;

    if yaml_str(&fm, "schema").as_deref() != Some(CARD_SCHEMA) {
        errors.push(format!("card schema is not `{CARD_SCHEMA}`"));
    }
    if yaml_str(&fm, "id").as_deref() != Some(fault_id.as_str()) {
        errors.push(format!("card id != folder name `{fault_id}`"));
    }
    if yaml_str(&fm, "equipment").as_deref() != Some(equip_dir.as_str()) {
        errors.push(format!("card equipment != folder `{equip_dir}`"));
    }
    let status = yaml_str(&fm, "status").unwrap_or_default();
    if !STATUSES.contains(&status.as_str()) {
        errors.push(format!("invalid status `{status}`"));
    }
    match yaml_str(&fm, "method") {
        Some(m) if METHODS.contains(&m.as_str()) => {}
        other => errors.push(format!("invalid method {other:?}")),
    }
    match yaml_str(&fm, "category") {
        Some(c) if CATEGORIES.contains(&c.as_str()) => {}
        other => errors.push(format!("invalid category {other:?}")),
    }
    match fm.get("severity").and_then(|s| s.as_i64()) {
        Some(1..=4) => {}
        other => errors.push(format!("invalid severity {other:?}")),
    }

    let card_points: Vec<String> = yaml_str_list(&fm, "points");
    if card_points.is_empty() {
        errors.push("card lists no points".into());
    }
    let card_outputs: Vec<String> = fm
        .get("outputs")
        .and_then(|o| o.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|e| yaml_str(e, "name"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if card_outputs.is_empty() {
        errors.push("card lists no outputs".into());
    }

    // --- point dictionary ---
    let dict_path = format!("points/{equip_dir}.points.json");
    match PointResolver::load(repo_root) {
        Ok(resolver) => {
            for p in &card_points {
                if let Err(error) = resolver.resolve_bare(&dict_path, p) {
                    errors.push(format!("point `{p}`: {error}"));
                }
            }
        }
        Err(error) => errors.push(format!("point dictionaries: {error}")),
    }

    // --- clusters and playbooks ---
    let clusters_path = repo_root.join("clusters/clusters.json");
    if let Ok(clusters) = std::fs::read(&clusters_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string()))
    {
        let ids: BTreeSet<String> = clusters
            .get("clusters")
            .and_then(|c| c.as_array())
            .map(|cs| {
                cs.iter()
                    .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        for c in yaml_str_list(&fm, "clusters") {
            if !ids.contains(&c) {
                errors.push(format!("cluster `{c}` not in clusters/clusters.json"));
            }
        }
    } else {
        errors.push("clusters/clusters.json unreadable".into());
    }
    for pb in yaml_str_list(&fm, "playbooks") {
        if !repo_root.join("playbooks").join(format!("{pb}.md")).is_file() {
            errors.push(format!("playbook `{pb}` missing in playbooks/"));
        }
    }

    // --- adjudicates (sensor-health meta-rules; SCHEMA.md frontmatter table) ---
    if let Some(adj) = fm.get("adjudicates") {
        match adj.get("verdict").and_then(|v| v.as_str()) {
            Some("invalid_while_active") | Some("ambiguous") => {}
            other => errors.push(format!(
                "adjudicates.verdict must be invalid_while_active|ambiguous, got {other:?}"
            )),
        }
        let adj_points = yaml_str_list(adj, "points");
        if adj_points.is_empty() {
            errors.push("adjudicates.points is empty".into());
        }
        for p in &adj_points {
            // A rule can only adjudicate a point it actually consumes.
            if !card_points.contains(p) {
                errors.push(format!(
                    "adjudicates point `{p}` is not among the card's own points"
                ));
            }
        }
    }

    // --- CXF cross-checks ---
    let rule_path = dir.join("rule.cxf.jsonld");
    let rule: serde_json::Value = std::fs::read(&rule_path)
        .map_err(|e| format!("rule.cxf.jsonld: {e}"))
        .and_then(|b| {
            serde_json::from_slice(&b).map_err(|e| format!("rule.cxf.jsonld: {e}"))
        })?;
    let graph = rule
        .get("@graph")
        .and_then(|g| g.as_array())
        .ok_or("rule.cxf.jsonld: no @graph array")?;
    let root = graph
        .iter()
        .find(|n| n.get("@type").and_then(|t| t.as_str()) == Some("S231:Block"))
        .ok_or("rule.cxf.jsonld: no root S231:Block node")?;
    let root_id = root
        .get("@id")
        .and_then(|s| s.as_str())
        .ok_or("root node has no @id")?;

    let cxf_inputs: BTreeSet<String> = id_refs(root, "S231:hasInput")
        .iter()
        .filter_map(|id| local_name(id, root_id).map(str::to_string))
        .collect();
    let cxf_outputs: BTreeSet<String> = id_refs(root, "S231:hasOutput")
        .iter()
        .filter_map(|id| local_name(id, root_id).map(str::to_string))
        .collect();
    let point_set: BTreeSet<String> = card_points.iter().cloned().collect();
    let output_set: BTreeSet<String> = card_outputs.iter().cloned().collect();
    if cxf_inputs != point_set {
        errors.push(format!(
            "card points {point_set:?} != CXF boundary inputs {cxf_inputs:?}"
        ));
    }
    if cxf_outputs != output_set {
        errors.push(format!(
            "card outputs {output_set:?} != CXF boundary outputs {cxf_outputs:?}"
        ));
    }

    // params.*.cxf paths must resolve to parameter nodes carrying S231:value.
    let param_node_ids: BTreeSet<&str> = graph
        .iter()
        .filter(|n| n.get("S231:value").is_some())
        .filter_map(|n| n.get("@id").and_then(|s| s.as_str()))
        .collect();
    if let Some(params) = fm.get("params").and_then(|p| p.as_mapping()) {
        for (name, spec) in params {
            let name = name.as_str().unwrap_or("?");
            let paths: Vec<String> = match spec.get("cxf") {
                Some(serde_yaml::Value::String(s)) => vec![s.clone()],
                Some(serde_yaml::Value::Sequence(seq)) => seq
                    .iter()
                    .filter_map(|e| e.as_str().map(str::to_string))
                    .collect(),
                _ => {
                    errors.push(format!("param `{name}` has no cxf path"));
                    continue;
                }
            };
            for p in paths {
                let full = format!("{root_id}.{p}");
                if !param_node_ids.contains(full.as_str()) {
                    errors.push(format!(
                        "param `{name}` cxf path `{p}` has no S231:value node in the CXF"
                    ));
                }
            }
        }
    } else {
        errors.push("card has no params map".into());
    }

    // --- vectors cross-checks ---
    let vectors_path = dir.join("vectors.json");
    let vectors: serde_json::Value = std::fs::read(&vectors_path)
        .map_err(|e| format!("vectors.json: {e}"))
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| format!("vectors.json: {e}")))?;
    if vectors.get("schema").and_then(|s| s.as_str()) != Some("cxf-library/vectors/v1") {
        errors.push("vectors schema is not cxf-library/vectors/v1".into());
    }
    let mut names = BTreeSet::new();
    for s in vectors
        .get("scenarios")
        .and_then(|s| s.as_array())
        .unwrap_or(&Vec::new())
    {
        let sname = s.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        if !names.insert(sname.to_string()) {
            errors.push(format!("duplicate scenario name `{sname}`"));
        }
        for (input, _) in s
            .get("inputs")
            .and_then(|i| i.as_object())
            .unwrap_or(&serde_json::Map::new())
        {
            if !point_set.contains(input) {
                errors.push(format!(
                    "scenario `{sname}` input `{input}` not in card points"
                ));
            }
        }
        for e in s.get("expect").and_then(|e| e.as_array()).unwrap_or(&Vec::new()) {
            if let Some(out) = e.get("output").and_then(|o| o.as_str()) {
                if !output_set.contains(out) {
                    errors.push(format!(
                        "scenario `{sname}` expects output `{out}` not in card outputs"
                    ));
                }
            }
        }
    }

    // --- diagram ---
    if !dir.join("diagram.svg").is_file() {
        errors.push("diagram.svg missing".into());
    } else if !body.contains("diagram.svg") {
        errors.push("card body does not reference diagram.svg".into());
    }

    // --- verified block consistency (content-id equality is checked by main against the engine) ---
    let verified = fm.get("verified").cloned().unwrap_or_default();
    let recorded_content_id = yaml_str(&verified, "content_id");
    let recorded_engine_rev = yaml_str(&verified, "engine_rev");
    if status == "verified" || status == "adopted" {
        if recorded_content_id.is_none() {
            errors.push("status verified but verified.content_id is null".into());
        }
        match &recorded_engine_rev {
            None => errors.push("status verified but verified.engine_rev is null".into()),
            Some(rev) => {
                let pin_path = repo_root.join("ENGINE_PIN");
                match std::fs::read_to_string(&pin_path) {
                    Ok(pin) => {
                        if !pin.trim().starts_with(rev.as_str()) {
                            errors.push(format!(
                                "verified.engine_rev `{rev}` does not prefix ENGINE_PIN `{}`",
                                pin.trim()
                            ));
                        }
                    }
                    Err(e) => errors.push(format!("ENGINE_PIN: {e}")),
                }
            }
        }
        if verified.get("date").is_none() || verified.get("date").is_some_and(|d| d.is_null()) {
            errors.push("status verified but verified.date is null".into());
        }
    }

    Ok(LintReport {
        errors,
        status,
        recorded_content_id,
        recorded_engine_rev,
    })
}
