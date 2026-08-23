//! OCL graph conformance runner (SCHEMA.md "Verification").
//!
//! Fault modes retain their existing behavior. `--routines` validates the generated deployment
//! registry; L0 accepts only the exact empty contract.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oce_api::{Engine, PointDirection, PointValueType, Value};
use serde::{Deserialize, Serialize};

mod lint;

#[derive(Deserialize)]
struct Vectors {
    schema: String,
    clock: Clock,
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedRoutineRegistry {
    schema: String,
    deployments: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Clock {
    step_s: f64,
    horizon_s: f64,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
    inputs: serde_json::Map<String, serde_json::Value>,
    expect: Vec<Expect>,
}

#[derive(Deserialize)]
struct Expect {
    output: String,
    from_s: f64,
    to_s: f64,
    equals: serde_json::Value,
    #[serde(default)]
    tolerance: Option<f64>,
}

/// One staged input change: stage `value` on `path` before the first tick whose time >= `t`.
struct InputEvent {
    t: f64,
    path: String,
    value: Value,
}

#[derive(Serialize)]
struct TraceDocument {
    schema: &'static str,
    engine_pin: &'static str,
    engine_source_revision: &'static str,
    rule_content_id: String,
    clock: TraceClock,
    scenarios: Vec<TraceScenario>,
}

#[derive(Serialize)]
struct TraceClock {
    step_s: f64,
}

#[derive(Serialize)]
struct TraceScenario {
    name: String,
    samples: Vec<TraceSample>,
}

#[derive(Serialize)]
struct TraceSample {
    t: f64,
    outputs: BTreeMap<String, serde_json::Value>,
}

/// Convert a JSON literal to an engine `Value`, coerced to the destination point's declared
/// type: a JSON number stages as `Integer` on an `Int` point and as `Real` on a `Real` point,
/// so vectors spell integer inputs as plain numbers with no format extension.
fn json_to_value(v: &serde_json::Value, want: PointValueType) -> Result<Value, String> {
    match (v, want) {
        (serde_json::Value::Bool(b), PointValueType::Bool) => Ok(Value::Boolean(*b)),
        (serde_json::Value::Number(n), PointValueType::Real) => n
            .as_f64()
            .map(Value::Real)
            .ok_or_else(|| format!("non-finite number {n}")),
        (serde_json::Value::Number(n), PointValueType::Int) => n
            .as_i64()
            .map(Value::Integer)
            .ok_or_else(|| format!("number {n} is not an exact i64 for an Int point")),
        (other, want) => Err(format!("value {other} cannot stage on a {want:?} point")),
    }
}

/// Resolve a canonical point name to the engine's full point path and declared value type.
/// Boundary connectors are `<root IRI>.<name>` per SCHEMA.md, so match on a `.<name>` or
/// `#<name>` suffix.
fn resolve_point(
    points: &[(String, PointDirection, PointValueType)],
    name: &str,
    want: PointDirection,
) -> Result<(String, PointValueType), String> {
    let dot = format!(".{name}");
    let hash = format!("#{name}");
    let hits: Vec<(&String, PointValueType)> = points
        .iter()
        .filter(|(p, d, _)| *d == want && (p.ends_with(&dot) || p.ends_with(&hash)))
        .map(|(p, _, vt)| (p, *vt))
        .collect();
    match hits.as_slice() {
        [(one, vt)] => Ok(((*one).clone(), *vt)),
        [] => Err(format!(
            "no {want:?} point matching `{name}`; available: {}",
            points
                .iter()
                .filter(|(_, d, _)| *d == want)
                .map(|(p, _, _)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        many => Err(format!("point `{name}` is ambiguous: {many:?}")),
    }
}

fn values_match(got: &Value, expected: &serde_json::Value, tolerance: f64) -> bool {
    match (got, expected) {
        (Value::Boolean(g), serde_json::Value::Bool(e)) => g == e,
        (Value::Real(g), serde_json::Value::Number(n)) => {
            n.as_f64().is_some_and(|e| (g - e).abs() <= tolerance)
        }
        (Value::Integer(g), serde_json::Value::Number(n)) => n.as_i64().is_some_and(|e| *g == e),
        _ => false,
    }
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::Boolean(b) => b.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Integer(i) => i.to_string(),
        other => format!("{other:?}"),
    }
}

fn value_to_json(v: &Value) -> Result<serde_json::Value, String> {
    match v {
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Real(r) => serde_json::Number::from_f64(*r)
            .map(serde_json::Value::Number)
            .ok_or_else(|| format!("cannot serialize non-finite output {r}")),
        Value::Integer(i) => Ok(serde_json::Value::Number((*i).into())),
        other => Err(format!("unsupported trace output value {other:?}")),
    }
}

fn boundary_name(path: &str) -> String {
    path.rsplit_once(['.', '#'])
        .map_or_else(|| path.to_string(), |(_, name)| name.to_string())
}

fn prepare_scenario(
    rule_bytes: &[u8],
    scenario: &Scenario,
) -> Result<
    (
        Engine,
        Vec<(String, PointDirection, PointValueType)>,
        Vec<InputEvent>,
    ),
    String,
> {
    let mut engine = Engine::in_memory();
    let report = engine
        .load_cxf(rule_bytes)
        .map_err(|e| format!("load_cxf failed: {e}"))?;
    for warning in &report.warnings {
        eprintln!("      load warning: {warning:?}");
    }
    let mut points: Vec<(String, PointDirection, PointValueType)> = engine
        .point_list(None)
        .map_err(|e| format!("point_list failed: {e}"))?
        .into_iter()
        .map(|p| (p.path, p.direction, p.value_type))
        .collect();
    for declared in engine.topology().boundary_outputs {
        points.push((declared.path, PointDirection::Out, PointValueType::Real));
    }

    let mut events: Vec<InputEvent> = Vec::new();
    for (name, spec) in &scenario.inputs {
        let (path, value_type) = resolve_point(&points, name, PointDirection::In)?;
        match spec {
            serde_json::Value::Array(steps) => {
                for step in steps {
                    let t = step
                        .get("t")
                        .and_then(serde_json::Value::as_f64)
                        .ok_or_else(|| format!("input `{name}`: step missing numeric `t`"))?;
                    let value = step
                        .get("value")
                        .ok_or_else(|| format!("input `{name}`: step missing `value`"))?;
                    events.push(InputEvent {
                        t,
                        path: path.clone(),
                        value: json_to_value(value, value_type)
                            .map_err(|e| format!("input `{name}`: {e}"))?,
                    });
                }
            }
            constant => events.push(InputEvent {
                t: 0.0,
                path: path.clone(),
                value: json_to_value(constant, value_type)
                    .map_err(|e| format!("input `{name}`: {e}"))?,
            }),
        }
    }
    events.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((engine, points, events))
}

fn run_scenario(rule_bytes: &[u8], clock: &Clock, scenario: &Scenario) -> Result<(), String> {
    let (mut engine, points, events) = prepare_scenario(rule_bytes, scenario)?;

    // Pre-resolve assertion outputs.
    let mut expects: Vec<(String, &Expect)> = Vec::new();
    for e in &scenario.expect {
        expects.push((resolve_point(&points, &e.output, PointDirection::Out)?.0, e));
    }

    let n_ticks = (clock.horizon_s / clock.step_s).floor() as u64;
    let mut next_event = 0usize;
    for k in 0..=n_ticks {
        let t = k as f64 * clock.step_s;
        while next_event < events.len() && events[next_event].t <= t {
            let ev = &events[next_event];
            engine
                .set_input(&ev.path, ev.value.clone())
                .map_err(|e| format!("set_input({}) failed: {e}", ev.path))?;
            next_event += 1;
        }
        engine
            .tick(t)
            .map_err(|e| format!("tick({t}) failed: {e}"))?;
        for (path, exp) in &expects {
            if t < exp.from_s || t > exp.to_s {
                continue;
            }
            let got = engine
                .get_output(path)
                .map_err(|e| format!("get_output({path}) failed: {e}"))?;
            if !values_match(&got, &exp.equals, exp.tolerance.unwrap_or(1e-9)) {
                return Err(format!(
                    "t={t}s: `{}` = {} but expected {} (window {}..{}s)",
                    exp.output,
                    fmt_value(&got),
                    exp.equals,
                    exp.from_s,
                    exp.to_s
                ));
            }
        }
    }
    Ok(())
}

fn trace_scenario(
    rule_bytes: &[u8],
    clock: &Clock,
    scenario: &Scenario,
) -> Result<TraceScenario, String> {
    let (mut engine, _points, events) = prepare_scenario(rule_bytes, scenario)?;
    let mut output_names = BTreeSet::new();
    let mut outputs = Vec::new();
    for declared in engine.topology().boundary_outputs {
        let name = boundary_name(&declared.path);
        if !output_names.insert(name.clone()) {
            return Err(format!("duplicate boundary output name `{name}`"));
        }
        outputs.push((name, declared.path));
    }
    let n_ticks = (clock.horizon_s / clock.step_s).floor() as u64;
    let mut next_event = 0usize;
    let mut samples = Vec::with_capacity(n_ticks as usize + 1);
    for k in 0..=n_ticks {
        let t = k as f64 * clock.step_s;
        while next_event < events.len() && events[next_event].t <= t {
            let ev = &events[next_event];
            engine
                .set_input(&ev.path, ev.value.clone())
                .map_err(|e| format!("set_input({}) failed: {e}", ev.path))?;
            next_event += 1;
        }
        engine
            .tick(t)
            .map_err(|e| format!("tick({t}) failed: {e}"))?;
        let mut values = BTreeMap::new();
        for (name, path) in &outputs {
            let value = engine
                .get_output(path)
                .map_err(|e| format!("get_output({path}) failed: {e}"))?;
            values.insert(name.clone(), value_to_json(&value)?);
        }
        samples.push(TraceSample { t, outputs: values });
    }
    Ok(TraceScenario {
        name: scenario.name.clone(),
        samples,
    })
}

fn validate_trace_clock(clock: &Clock) -> Result<(), String> {
    if !clock.step_s.is_finite() || clock.step_s <= 0.0 {
        return Err("trace clock step_s must be finite and greater than zero".to_string());
    }
    if !clock.horizon_s.is_finite() || clock.horizon_s < 0.0 {
        return Err("trace clock horizon_s must be finite and non-negative".to_string());
    }
    let n_ticks = (clock.horizon_s / clock.step_s).floor();
    if !n_ticks.is_finite() || n_ticks >= 1_000_000.0 {
        return Err("trace clock exceeds the 1,000,000-sample safety limit".to_string());
    }
    Ok(())
}

fn trace_vectors(fault_dir: &Path, vectors_path: &Path) -> Result<TraceDocument, String> {
    let rule_path = fault_dir.join("rule.cxf.jsonld");
    let rule_bytes =
        std::fs::read(&rule_path).map_err(|e| format!("{}: {e}", rule_path.display()))?;
    let vectors: Vectors = serde_json::from_slice(
        &std::fs::read(vectors_path).map_err(|e| format!("{}: {e}", vectors_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", vectors_path.display()))?;
    if vectors.schema != "cxf-library/vectors/v1" {
        return Err(format!("unsupported vectors schema `{}`", vectors.schema));
    }
    validate_trace_clock(&vectors.clock)?;
    if vectors.scenarios.len() > 512 {
        return Err("trace request exceeds the 512-scenario safety limit".to_string());
    }
    let samples_per_scenario =
        (vectors.clock.horizon_s / vectors.clock.step_s).floor() as usize + 1;
    if !matches!(
        samples_per_scenario.checked_mul(vectors.scenarios.len()),
        Some(total) if total <= 1_000_000
    ) {
        return Err("trace request exceeds the 1,000,000-total-sample safety limit".to_string());
    }
    let mut identity_engine = Engine::in_memory();
    identity_engine
        .load_cxf(&rule_bytes)
        .map_err(|e| format!("load_cxf failed while identifying rule: {e}"))?;
    let rule_content_id = identity_engine
        .export_cxf()
        .map_err(|e| format!("export_cxf failed while identifying rule: {e}"))?
        .content_id_complete()
        .map_err(|e| format!("rule content id unavailable: {e}"))?;
    let mut scenarios = Vec::with_capacity(vectors.scenarios.len());
    for scenario in &vectors.scenarios {
        scenarios.push(trace_scenario(&rule_bytes, &vectors.clock, scenario)?);
    }
    Ok(TraceDocument {
        schema: "cxf-library/replay-trace/v1",
        engine_pin: include_str!("../../../ENGINE_PIN").trim(),
        engine_source_revision: env!("CXF_ENGINE_SOURCE_REV"),
        rule_content_id,
        clock: TraceClock {
            step_s: vectors.clock.step_s,
        },
        scenarios,
    })
}

fn validate_generated_registry(bytes: &[u8]) -> Result<usize, String> {
    let registry: GeneratedRoutineRegistry = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid generated routine registry: {error}"))?;
    if registry.schema != "cxf-library/generated-routine-registry/v1" {
        return Err(format!(
            "unsupported generated routine registry schema `{}`",
            registry.schema
        ));
    }
    if !registry.deployments.is_empty() {
        return Err(format!(
            "generated routine registry must remain empty in L0; found {} deployment(s)",
            registry.deployments.len()
        ));
    }
    Ok(registry.deployments.len())
}

fn verify_generated_routines(repo_root: &Path) -> Result<(), String> {
    let registry_path = repo_root.join("routines/generated-registry.json");
    let bytes =
        std::fs::read(&registry_path).map_err(|e| format!("{}: {e}", registry_path.display()))?;
    let count = validate_generated_registry(&bytes)
        .map_err(|error| format!("{}: {error}", registry_path.display()))?;
    println!("discovered {count} generated routine deployments");
    Ok(())
}

fn verify_fault_dir(dir: &Path, replay_only: bool) -> Result<bool, String> {
    let rule_path = dir.join("rule.cxf.jsonld");
    let vectors_path = dir.join("vectors.json");
    let rule_bytes =
        std::fs::read(&rule_path).map_err(|e| format!("{}: {e}", rule_path.display()))?;
    let vectors: Vectors = serde_json::from_slice(
        &std::fs::read(&vectors_path).map_err(|e| format!("{}: {e}", vectors_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", vectors_path.display()))?;
    if vectors.schema != "cxf-library/vectors/v1" {
        return Err(format!("unsupported vectors schema `{}`", vectors.schema));
    }

    println!("{}", dir.display());

    // Diagnostic identity: load once and report the engine's exported content id.
    let mut engine = Engine::in_memory();
    let mut content_id = None;
    match engine.load_cxf(&rule_bytes) {
        Ok(_) => match engine.export_cxf() {
            Ok(report) => match report.content_id_complete() {
                Ok(id) => {
                    println!("  content_id: {id}");
                    content_id = Some(id);
                }
                Err(e) => println!("  content_id unavailable (export warnings): {e}"),
            },
            Err(e) => println!("  export_cxf failed: {e}"),
        },
        Err(e) => return Err(format!("load_cxf failed: {e}")),
    }

    let mut all_pass = true;

    // Generated simulation replays contain only a graph plus transient vectors. Their source
    // packages are linted separately; --replay-only makes the exit code describe replay success.
    if replay_only {
        println!("  LINT  skipped (generated replay)");
    } else {
        let repo_root = dir
            .canonicalize()
            .ok()
            .and_then(|d| {
                d.parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(Path::to_path_buf)
            })
            .ok_or("cannot locate repo root from fault dir")?;
        match lint::lint_fault_dir(dir, &repo_root) {
            Ok(report) => {
                let mut errors = report.errors;
                if matches!(report.status.as_str(), "verified" | "adopted")
                    && let (Some(recorded), Some(actual)) =
                        (&report.recorded_content_id, &content_id)
                    && recorded != actual
                {
                    errors.push(format!(
                        "verified.content_id `{recorded}` != engine export `{actual}` — re-verify and update the card"
                    ));
                }
                if errors.is_empty() {
                    println!("  LINT  ok");
                } else {
                    all_pass = false;
                    for e in errors {
                        println!("  LINT  {e}");
                    }
                }
            }
            Err(e) => {
                all_pass = false;
                println!("  LINT  {e}");
            }
        }
    }
    for scenario in &vectors.scenarios {
        match run_scenario(&rule_bytes, &vectors.clock, scenario) {
            Ok(()) => println!("  PASS  {}", scenario.name),
            Err(msg) => {
                all_pass = false;
                println!("  FAIL  {} — {msg}", scenario.name);
            }
        }
    }
    Ok(all_pass)
}

/// Every `faults/<equip>/<FAULT-ID>/` directory containing a rule document, sorted.
fn discover_fault_dirs(faults_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = Vec::new();
    for equip in
        std::fs::read_dir(faults_root).map_err(|e| format!("{}: {e}", faults_root.display()))?
    {
        let equip = equip.map_err(|e| e.to_string())?.path();
        if !equip.is_dir() {
            continue;
        }
        for fault in std::fs::read_dir(&equip).map_err(|e| e.to_string())? {
            let fault = fault.map_err(|e| e.to_string())?.path();
            if fault.is_dir() && fault.join("rule.cxf.jsonld").is_file() {
                dirs.push(fault);
            }
        }
    }
    dirs.sort();
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::{Clock, Vectors, validate_generated_registry, validate_trace_clock};

    #[test]
    fn trace_clock_rejects_unsafe_values() {
        for clock in [
            Clock {
                step_s: 0.0,
                horizon_s: 60.0,
            },
            Clock {
                step_s: f64::NAN,
                horizon_s: 60.0,
            },
            Clock {
                step_s: 60.0,
                horizon_s: -1.0,
            },
            Clock {
                step_s: 1.0,
                horizon_s: 1_000_000.0,
            },
        ] {
            assert!(validate_trace_clock(&clock).is_err());
        }
    }

    #[test]
    fn trace_clock_accepts_bounded_native_cadence() {
        assert!(
            validate_trace_clock(&Clock {
                step_s: 60.0,
                horizon_s: 31_536_000.0,
            })
            .is_ok()
        );
    }

    #[test]
    fn fault_vectors_remain_valid_without_routine_identity() {
        let vectors: Vectors = serde_json::from_str(
            r#"{
                "schema":"cxf-library/vectors/v1",
                "clock":{"step_s":1.0,"horizon_s":0.0},
                "scenarios":[]
            }"#,
        )
        .expect("fault vectors deserialize");
        assert_eq!(vectors.schema, "cxf-library/vectors/v1");
    }

    #[test]
    fn exact_empty_generated_registry_is_valid() {
        let count = validate_generated_registry(
            br#"{
                "schema":"cxf-library/generated-routine-registry/v1",
                "deployments":[]
            }"#,
        )
        .expect("empty generated registry is valid");
        assert_eq!(count, 0);
    }

    #[test]
    fn generated_registry_rejects_wrong_schema_shape_and_extra_keys() {
        for document in [
            r#"{"schema":"wrong","deployments":[]}"#,
            r#"{"schema":"cxf-library/generated-routine-registry/v1","deployments":{}}"#,
            r#"{"schema":"cxf-library/generated-routine-registry/v1"}"#,
            r#"{"schema":"cxf-library/generated-routine-registry/v1","deployments":[],"extra":true}"#,
            r#"[]"#,
            r#"{"#,
        ] {
            assert!(
                validate_generated_registry(document.as_bytes()).is_err(),
                "{document}"
            );
        }
    }

    #[test]
    fn generated_registry_rejects_deployments_until_the_contract_lands() {
        let error = validate_generated_registry(
            br#"{
                "schema":"cxf-library/generated-routine-registry/v1",
                "deployments":[{"id":"future"}]
            }"#,
        )
        .expect_err("nonempty generated registry must fail closed");
        assert!(error.contains("must remain empty in L0; found 1 deployment(s)"));
    }
}

fn main() -> ExitCode {
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if raw_args.first().is_some_and(|arg| arg == "--trace-json") {
        if raw_args.len() != 3 {
            eprintln!("usage: cxf-verify --trace-json <fault-dir> <vectors.json>");
            return ExitCode::from(2);
        }
        match trace_vectors(Path::new(&raw_args[1]), Path::new(&raw_args[2])) {
            Ok(trace) => match serde_json::to_string(&trace) {
                Ok(json) => {
                    println!("{json}");
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("trace serialization failed: {e}");
                    return ExitCode::from(2);
                }
            },
            Err(e) => {
                eprintln!("trace replay failed: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if raw_args.iter().any(|arg| arg == "--routines") {
        if raw_args.len() != 1 {
            eprintln!("usage: cxf-verify --routines");
            return ExitCode::from(2);
        }
        return match verify_generated_routines(Path::new(".")) {
            Ok(()) => {
                println!("all generated routine deployment scenarios passed");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("--routines: {e}");
                ExitCode::from(2)
            }
        };
    }
    let mut args: Vec<PathBuf> = raw_args.into_iter().map(PathBuf::from).collect();
    let replay_only = args.iter().any(|a| a.as_os_str() == "--replay-only");
    if replay_only && args.iter().any(|a| a.as_os_str() == "--all") {
        eprintln!("--replay-only is only valid with explicit generated replay directories");
        return ExitCode::from(2);
    }
    args.retain(|a| a.as_os_str() != "--replay-only");
    if args.iter().any(|a| a.as_os_str() == "--all") {
        args = match discover_fault_dirs(Path::new("faults")) {
            Ok(dirs) => dirs,
            Err(e) => {
                eprintln!("--all: {e}");
                return ExitCode::from(2);
            }
        };
        println!("discovered {} fault dirs", args.len());
    }
    if args.is_empty() {
        eprintln!(
            "usage: cxf-verify [--replay-only] (--all | <fault-dir>…) (each containing rule.cxf.jsonld + vectors.json)\n       cxf-verify --trace-json <fault-dir> <vectors.json>\n       cxf-verify --routines"
        );
        return ExitCode::from(2);
    }
    let mut ok = true;
    for dir in &args {
        match verify_fault_dir(dir, replay_only) {
            Ok(pass) => ok &= pass,
            Err(msg) => {
                ok = false;
                println!("{}\n  ERROR {msg}", dir.display());
            }
        }
    }
    if ok {
        println!("all scenarios passed");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
