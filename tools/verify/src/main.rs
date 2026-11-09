//! OCL graph conformance runner (SCHEMA.md "Verification").
//!
//! Fault modes retain their existing behavior. `--routines` validates the generated deployment
//! registry; the current contract accepts only the exact empty registry. `--g36-declarations`
//! checks two release declarations without resolving their dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oce_api::{CompletedFrame, Engine, Value, ValueType};
use serde::{Deserialize, Serialize};

mod lint;
mod point_resolution;

const G36_DECLARATIONS_USAGE: &str = "usage: cxf-verify --g36-declarations <release-root>";

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

/// One staged input change: `value` replaces what boundary input `input` holds, before the first
/// tick whose time >= `t`.
struct InputEvent {
    t: f64,
    /// Index into [`InputSchedule::held`].
    input: usize,
    value: Value,
}

/// One scenario's inputs as a zero-order hold over the graph's boundary inputs.
///
/// The engine executes complete frames only: every boundary input is supplied on every tick, and
/// nothing is carried over inside the engine. The hold therefore lives here. An input keeps the
/// value of its latest event at or before the tick time; an input with no such event has no value,
/// and the frame is refused rather than filled with a type seed.
struct InputSchedule {
    /// Every boundary input of the loaded graph, in the engine's canonical order, with the value
    /// it holds after the events staged so far.
    held: Vec<(String, Option<Value>)>,
    /// Ascending by `t`; events at one `t` keep their order in the vectors, so the later one wins.
    events: Vec<InputEvent>,
    next_event: usize,
}

impl InputSchedule {
    fn new(inputs: Vec<String>, mut events: Vec<InputEvent>) -> Self {
        events.sort_by(|a, b| a.t.total_cmp(&b.t));
        Self {
            held: inputs.into_iter().map(|path| (path, None)).collect(),
            events,
            next_event: 0,
        }
    }

    /// Stage every event whose `t` is at or before `t`, then return the complete frame for the
    /// tick at `t`. Ticks are asked for in ascending time.
    fn frame_at(&mut self, t: f64) -> Result<Vec<(&str, Value)>, String> {
        while let Some(event) = self.events.get(self.next_event)
            && event.t <= t
        {
            self.held[event.input].1 = Some(event.value.clone());
            self.next_event += 1;
        }
        self.held
            .iter()
            .map(|(path, value)| match value {
                Some(value) => Ok((path.as_str(), value.clone())),
                None => Err(format!(
                    "boundary input `{path}` has no value at t={t}s: the scenario must supply every boundary input from the first tick"
                )),
            })
            .collect()
    }
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

/// Convert a JSON literal to an engine `Value` of the boundary input's exact declared type: a
/// JSON number stages as `Integer` on an Integer input and as `Real` on a Real input, so vectors
/// spell integer inputs as plain numbers with no format extension.
fn json_to_value(v: &serde_json::Value, want: &ValueType) -> Result<Value, String> {
    match (v, want) {
        (serde_json::Value::Bool(b), ValueType::Boolean) => Ok(Value::Boolean(*b)),
        (serde_json::Value::Number(n), ValueType::Real) => n
            .as_f64()
            .map(Value::Real)
            .ok_or_else(|| format!("non-finite number {n}")),
        (serde_json::Value::Number(n), ValueType::Integer) => n
            .as_i64()
            .map(Value::Integer)
            .ok_or_else(|| format!("number {n} is not an exact i64 for an Integer input")),
        (other, want) => Err(format!("value {other} cannot stage on a {want:?} input")),
    }
}

/// Resolve a canonical point name to its index in `paths`, the engine's full paths of one kind of
/// boundary connector (`kind` names it in errors). Boundary connectors are `<root IRI>.<name>` per
/// SCHEMA.md, so match on a `.<name>` or `#<name>` suffix.
fn resolve_point(paths: &[String], name: &str, kind: &str) -> Result<usize, String> {
    let dot = format!(".{name}");
    let hash = format!("#{name}");
    let hits: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| p.ends_with(&dot) || p.ends_with(&hash))
        .map(|(index, _)| index)
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!(
            "no boundary {kind} matching `{name}`; available: {}",
            paths.join(", ")
        )),
        many => Err(format!(
            "boundary {kind} `{name}` is ambiguous: {:?}",
            many.iter().map(|index| &paths[*index]).collect::<Vec<_>>()
        )),
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

/// A freshly loaded engine with one scenario's inputs resolved against its boundary.
struct Replay {
    engine: Engine,
    schedule: InputSchedule,
    /// Full paths of the boundary outputs every completed frame commits.
    outputs: Vec<String>,
}

impl Replay {
    /// Execute the tick at `t` as one complete frame and return the engine's committed result.
    fn advance(&mut self, t: f64) -> Result<CompletedFrame, String> {
        let frame = self.schedule.frame_at(t)?;
        let prepared = self
            .engine
            .prepare_frame(t, &frame)
            .map_err(|e| format!("prepare_frame({t}) failed: {e}"))?;
        self.engine
            .execute_frame(prepared)
            .map_err(|e| format!("execute_frame({t}) failed: {e}"))
    }
}

/// The committed value of boundary output `path` in a completed frame.
fn committed_output<'a>(frame: &'a CompletedFrame, path: &str) -> Result<&'a Value, String> {
    frame
        .outputs()
        .iter()
        .find(|(committed, _)| committed == path)
        .map(|(_, value)| value)
        .ok_or_else(|| {
            format!(
                "t={}s: the frame committed no boundary output `{path}`",
                frame.time()
            )
        })
}

/// Tick times `k * step_s` for `k = 0 ..= floor(horizon_s / step_s)`.
fn tick_times(clock: &Clock) -> impl Iterator<Item = f64> {
    let n_ticks = (clock.horizon_s / clock.step_s).floor() as u64;
    let step_s = clock.step_s;
    (0..=n_ticks).map(move |k| k as f64 * step_s)
}

fn prepare_scenario(rule_bytes: &[u8], scenario: &Scenario) -> Result<Replay, String> {
    let mut engine = Engine::new();
    let (report, _) = engine
        .load_cxf(rule_bytes)
        .map_err(|e| format!("load_cxf failed: {e}"))?;
    for warning in &report.warnings {
        eprintln!("      load warning: {warning:?}");
    }
    let (inputs, input_types): (Vec<String>, Vec<ValueType>) = engine
        .input_definitions()
        .map_err(|e| format!("input_definitions failed: {e}"))?
        .into_iter()
        .map(|definition| (definition.path, definition.value_type))
        .unzip();
    let outputs = engine
        .topology()
        .boundary_outputs
        .into_iter()
        .map(|declared| declared.path)
        .collect();

    let mut events: Vec<InputEvent> = Vec::new();
    for (name, spec) in &scenario.inputs {
        let input = resolve_point(&inputs, name, "input")?;
        let value_type = &input_types[input];
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
                        input,
                        value: json_to_value(value, value_type)
                            .map_err(|e| format!("input `{name}`: {e}"))?,
                    });
                }
            }
            constant => events.push(InputEvent {
                t: 0.0,
                input,
                value: json_to_value(constant, value_type)
                    .map_err(|e| format!("input `{name}`: {e}"))?,
            }),
        }
    }
    Ok(Replay {
        engine,
        schedule: InputSchedule::new(inputs, events),
        outputs,
    })
}

fn run_scenario(rule_bytes: &[u8], clock: &Clock, scenario: &Scenario) -> Result<(), String> {
    let mut replay = prepare_scenario(rule_bytes, scenario)?;

    // Pre-resolve assertion outputs.
    let mut expects: Vec<(String, &Expect)> = Vec::new();
    for e in &scenario.expect {
        let output = resolve_point(&replay.outputs, &e.output, "output")?;
        expects.push((replay.outputs[output].clone(), e));
    }

    for t in tick_times(clock) {
        let frame = replay.advance(t)?;
        for (path, exp) in &expects {
            if t < exp.from_s || t > exp.to_s {
                continue;
            }
            let got = committed_output(&frame, path)?;
            if !values_match(got, &exp.equals, exp.tolerance.unwrap_or(1e-9)) {
                return Err(format!(
                    "t={t}s: `{}` = {} but expected {} (window {}..{}s)",
                    exp.output,
                    fmt_value(got),
                    exp.equals,
                    exp.from_s,
                    exp.to_s
                ));
            }
        }
    }
    Ok(())
}

/// Replay one scenario and record every boundary output at every tick. A refusal names the
/// scenario, since a trace request carries many and has no per-scenario report line.
fn trace_scenario(
    rule_bytes: &[u8],
    clock: &Clock,
    scenario: &Scenario,
) -> Result<TraceScenario, String> {
    match trace_samples(rule_bytes, clock, scenario) {
        Ok(samples) => Ok(TraceScenario {
            name: scenario.name.clone(),
            samples,
        }),
        Err(error) => Err(format!("scenario `{}`: {error}", scenario.name)),
    }
}

fn trace_samples(
    rule_bytes: &[u8],
    clock: &Clock,
    scenario: &Scenario,
) -> Result<Vec<TraceSample>, String> {
    let mut replay = prepare_scenario(rule_bytes, scenario)?;
    let mut output_names = BTreeSet::new();
    let mut outputs = Vec::new();
    for path in &replay.outputs {
        let name = boundary_name(path);
        if !output_names.insert(name.clone()) {
            return Err(format!("duplicate boundary output name `{name}`"));
        }
        outputs.push((name, path.clone()));
    }
    let mut samples = Vec::new();
    for t in tick_times(clock) {
        let frame = replay.advance(t)?;
        let mut values = BTreeMap::new();
        for (name, path) in &outputs {
            values.insert(
                name.clone(),
                value_to_json(committed_output(&frame, path)?)?,
            );
        }
        samples.push(TraceSample { t, outputs: values });
    }
    Ok(samples)
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
    let mut identity_engine = Engine::new();
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
            "generated routine registry must remain empty until generated routine deployments are implemented; found {} deployment(s)",
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
    let mut engine = Engine::new();
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

fn g36_declaration_root(raw_args: &[std::ffi::OsString]) -> Result<Option<&Path>, &'static str> {
    if !raw_args.iter().any(|arg| arg == "--g36-declarations") {
        return Ok(None);
    }
    match raw_args {
        [mode, root] if mode == "--g36-declarations" && !Path::new(root).as_os_str().is_empty() => {
            Ok(Some(Path::new(root)))
        }
        _ => Err(G36_DECLARATIONS_USAGE),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use oce_api::Value;

    use super::{
        Clock, InputEvent, InputSchedule, Scenario, Vectors, g36_declaration_root, run_scenario,
        tick_times, trace_scenario, validate_generated_registry, validate_trace_clock,
        values_match,
    };

    /// A schedule over inputs `a` and `b`, with `(t, input index, value)` events.
    fn schedule(events: &[(f64, usize, f64)]) -> InputSchedule {
        InputSchedule::new(
            vec!["urn:g#root.a".to_string(), "urn:g#root.b".to_string()],
            events
                .iter()
                .map(|&(t, input, value)| InputEvent {
                    t,
                    input,
                    value: Value::Real(value),
                })
                .collect(),
        )
    }

    /// The frame at `t` as `(path, real value)` pairs.
    fn reals(schedule: &mut InputSchedule, t: f64) -> Vec<(String, f64)> {
        schedule
            .frame_at(t)
            .expect("complete frame")
            .into_iter()
            .map(|(path, value)| match value {
                Value::Real(real) => (path.to_string(), real),
                other => panic!("unexpected value {other:?}"),
            })
            .collect()
    }

    fn frame(a: f64, b: f64) -> Vec<(String, f64)> {
        vec![
            ("urn:g#root.a".to_string(), a),
            ("urn:g#root.b".to_string(), b),
        ]
    }

    #[test]
    fn every_frame_carries_every_input_at_its_latest_staged_value() {
        let mut schedule = schedule(&[(0.0, 0, 1.0), (0.0, 1, 10.0), (120.0, 0, 2.0)]);
        assert_eq!(reals(&mut schedule, 0.0), frame(1.0, 10.0));
        assert_eq!(reals(&mut schedule, 60.0), frame(1.0, 10.0));
        assert_eq!(reals(&mut schedule, 120.0), frame(2.0, 10.0));
        assert_eq!(reals(&mut schedule, 180.0), frame(2.0, 10.0));
    }

    #[test]
    fn event_between_ticks_is_staged_before_the_first_tick_at_or_after_it() {
        let mut schedule = schedule(&[(0.0, 0, 1.0), (0.0, 1, 10.0), (61.0, 1, 20.0)]);
        assert_eq!(reals(&mut schedule, 60.0), frame(1.0, 10.0));
        assert_eq!(reals(&mut schedule, 120.0), frame(1.0, 20.0));
    }

    #[test]
    fn later_of_two_events_at_one_time_wins_whatever_order_they_arrive_in() {
        let mut schedule = schedule(&[
            (60.0, 0, 3.0),
            (0.0, 0, 1.0),
            (0.0, 1, 10.0),
            (60.0, 0, 4.0),
        ]);
        assert_eq!(reals(&mut schedule, 0.0), frame(1.0, 10.0));
        assert_eq!(reals(&mut schedule, 60.0), frame(4.0, 10.0));
    }

    #[test]
    fn input_with_no_value_yet_refuses_the_frame_and_names_the_input() {
        let mut schedule = schedule(&[(0.0, 0, 1.0), (60.0, 1, 10.0)]);
        let error = schedule.frame_at(0.0).expect_err("b has no value at t=0");
        assert!(
            error.contains("`urn:g#root.b` has no value at t=0s"),
            "{error}"
        );
        assert_eq!(reals(&mut schedule, 60.0), frame(1.0, 10.0));
    }

    #[test]
    fn tick_times_run_from_zero_through_the_last_whole_step() {
        let times =
            |step_s, horizon_s| tick_times(&Clock { step_s, horizon_s }).collect::<Vec<_>>();
        assert_eq!(times(60.0, 180.0), [0.0, 60.0, 120.0, 180.0]);
        assert_eq!(times(60.0, 150.0), [0.0, 60.0, 120.0]);
        assert_eq!(times(1.0, 0.0), [0.0]);
    }

    /// A test-local graph, so these tests do not move when a library rule does: Real inputs `a`
    /// and `b`, Boolean output `y = (a > 5) and (b > 5)`, no state.
    const BOTH_ABOVE: &[u8] = include_bytes!("../tests/fixtures/both_above_threshold.cxf.jsonld");

    /// Ticks at 0, 60 and 120 s.
    const THREE_TICKS: Clock = Clock {
        step_s: 60.0,
        horizon_s: 120.0,
    };

    fn scenario(inputs: serde_json::Value, expect: serde_json::Value) -> Scenario {
        serde_json::from_value(serde_json::json!({
            "name": "probe",
            "inputs": inputs,
            "expect": expect
        }))
        .expect("scenario deserializes")
    }

    /// `a` stays above its threshold; `b` is above it at the 60 s tick only, so `y` is false,
    /// true, false over the three ticks.
    fn pulse_at_the_middle_tick() -> serde_json::Value {
        serde_json::json!({
            "a": 45.0,
            "b": [{"t": 0, "value": 0.0}, {"t": 60, "value": 45.0}, {"t": 120, "value": 0.0}]
        })
    }

    /// A scenario that supplies only one of the two inputs used to replay with the other held at
    /// the engine's zero seed, and so passed a `y` false expectation. A complete frame cannot be
    /// built for it, so it is refused.
    #[test]
    fn scenario_that_omits_a_boundary_input_is_refused_not_replayed_on_a_seed() {
        let y_stays_false =
            serde_json::json!([{"output": "y", "from_s": 0, "to_s": 120, "equals": false}]);

        let complete = scenario(
            serde_json::json!({"a": 45.0, "b": 0.0}),
            y_stays_false.clone(),
        );
        run_scenario(BOTH_ABOVE, &THREE_TICKS, &complete).expect("a complete scenario replays");

        let partial = scenario(serde_json::json!({"a": 45.0}), y_stays_false.clone());
        let error = run_scenario(BOTH_ABOVE, &THREE_TICKS, &partial).expect_err("incomplete frame");
        assert!(
            error.contains("both_above.b` has no value at t=0s"),
            "{error}"
        );

        let late = scenario(
            serde_json::json!({"a": 45.0, "b": [{"t": 60, "value": 0.0}]}),
            y_stays_false,
        );
        let error =
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &late).expect_err("no value at the first tick");
        assert!(
            error.contains("both_above.b` has no value at t=0s"),
            "{error}"
        );
    }

    #[test]
    fn expectation_with_the_opposite_value_fails_at_the_first_tick_of_its_window() {
        let both_above = serde_json::json!({"a": 45.0, "b": 45.0});
        let expect_y = |equals: bool| serde_json::json!([{"output": "y", "from_s": 0, "to_s": 120, "equals": equals}]);

        let right = scenario(both_above.clone(), expect_y(true));
        run_scenario(BOTH_ABOVE, &THREE_TICKS, &right).expect("y is true at every tick");

        let flipped = scenario(both_above, expect_y(false));
        let error = run_scenario(BOTH_ABOVE, &THREE_TICKS, &flipped)
            .expect_err("y is true, the expectation says false");
        assert!(
            error.contains("t=0s: `y` = true but expected false"),
            "{error}"
        );
    }

    #[test]
    fn expectation_wrong_at_one_tick_inside_its_window_fails_at_that_tick() {
        let tick_by_tick = scenario(
            pulse_at_the_middle_tick(),
            serde_json::json!([
                {"output": "y", "from_s": 0, "to_s": 0, "equals": false},
                {"output": "y", "from_s": 60, "to_s": 60, "equals": true},
                {"output": "y", "from_s": 120, "to_s": 120, "equals": false}
            ]),
        );
        run_scenario(BOTH_ABOVE, &THREE_TICKS, &tick_by_tick)
            .expect("y is false, true, false over the three ticks");

        let one_wide_window = scenario(
            pulse_at_the_middle_tick(),
            serde_json::json!([{"output": "y", "from_s": 0, "to_s": 120, "equals": false}]),
        );
        let error = run_scenario(BOTH_ABOVE, &THREE_TICKS, &one_wide_window)
            .expect_err("y is true at the 60 s tick");
        assert!(
            error.contains("t=60s: `y` = true but expected false"),
            "{error}"
        );

        let last_tick_wrong = scenario(
            pulse_at_the_middle_tick(),
            serde_json::json!([
                {"output": "y", "from_s": 0, "to_s": 0, "equals": false},
                {"output": "y", "from_s": 60, "to_s": 120, "equals": true}
            ]),
        );
        let error = run_scenario(BOTH_ABOVE, &THREE_TICKS, &last_tick_wrong)
            .expect_err("y is false again at the 120 s tick");
        assert!(
            error.contains("t=120s: `y` = false but expected true"),
            "{error}"
        );
    }

    #[test]
    fn expectation_naming_no_boundary_output_fails_before_any_tick() {
        let misspelt = scenario(
            serde_json::json!({"a": 45.0, "b": 45.0}),
            serde_json::json!([{"output": "yy", "from_s": 0, "to_s": 120, "equals": true}]),
        );
        let error = run_scenario(BOTH_ABOVE, &THREE_TICKS, &misspelt)
            .expect_err("the graph has no output `yy`");
        assert!(
            error.contains("no boundary output matching `yy`"),
            "{error}"
        );

        // An internal connector is not a boundary output, whatever its value.
        let internal = scenario(
            serde_json::json!({"a": 45.0, "b": 45.0}),
            serde_json::json!([{"output": "u1", "from_s": 0, "to_s": 120, "equals": true}]),
        );
        let error =
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &internal).expect_err("`both.u1` is internal");
        assert!(
            error.contains("no boundary output matching `u1`"),
            "{error}"
        );
    }

    #[test]
    fn expected_value_matches_only_its_own_type_and_reals_only_within_tolerance() {
        use serde_json::json;
        assert!(values_match(&Value::Boolean(true), &json!(true), 1e-9));
        assert!(!values_match(&Value::Boolean(true), &json!(false), 1e-9));
        assert!(values_match(&Value::Real(1.0), &json!(1.05), 0.1));
        assert!(!values_match(&Value::Real(1.0), &json!(1.2), 0.1));
        assert!(!values_match(&Value::Real(f64::NAN), &json!(1.0), 0.1));
        assert!(values_match(&Value::Integer(3), &json!(3), 0.5));
        assert!(!values_match(&Value::Integer(3), &json!(4), 10.0));
        assert!(!values_match(&Value::Boolean(true), &json!(1), 1e-9));
        assert!(!values_match(&Value::Real(1.0), &json!(true), 1e-9));
    }

    #[test]
    fn trace_records_every_committed_boundary_output_at_every_tick() {
        let traced = trace_scenario(
            BOTH_ABOVE,
            &THREE_TICKS,
            &scenario(pulse_at_the_middle_tick(), serde_json::json!([])),
        )
        .expect("a complete scenario traces");
        assert_eq!(traced.name, "probe");
        let samples: Vec<(f64, Vec<(&str, &serde_json::Value)>)> = traced
            .samples
            .iter()
            .map(|sample| {
                let outputs = sample.outputs.iter().map(|(k, v)| (k.as_str(), v));
                (sample.t, outputs.collect())
            })
            .collect();
        let (no, yes) = (
            serde_json::Value::Bool(false),
            serde_json::Value::Bool(true),
        );
        assert_eq!(
            samples,
            [
                (0.0, vec![("y", &no)]),
                (60.0, vec![("y", &yes)]),
                (120.0, vec![("y", &no)]),
            ]
        );
    }

    #[test]
    fn trace_refusal_names_the_scenario_and_the_input() {
        let Err(error) = trace_scenario(
            BOTH_ABOVE,
            &THREE_TICKS,
            &scenario(serde_json::json!({"a": 45.0}), serde_json::json!([])),
        ) else {
            panic!("a scenario without `b` cannot be traced");
        };
        assert!(error.starts_with("scenario `probe`: "), "{error}");
        assert!(
            error.contains("both_above.b` has no value at t=0s"),
            "{error}"
        );
    }

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
        assert!(error.contains(
            "must remain empty until generated routine deployments are implemented; found 1 deployment(s)"
        ));
    }

    #[test]
    fn g36_declaration_mode_requires_one_nonempty_root() {
        let valid = [
            OsString::from("--g36-declarations"),
            OsString::from("/tmp/release"),
        ];
        assert_eq!(
            g36_declaration_root(&valid).expect("valid mode"),
            Some(Path::new("/tmp/release"))
        );
        assert_eq!(
            g36_declaration_root(&[OsString::from("--all")]).expect("different mode"),
            None
        );

        for invalid in [
            vec![OsString::from("--g36-declarations")],
            vec![OsString::from("--g36-declarations"), OsString::new()],
            vec![
                OsString::from("release"),
                OsString::from("--g36-declarations"),
            ],
            vec![
                OsString::from("--g36-declarations"),
                OsString::from("release"),
                OsString::from("extra"),
            ],
        ] {
            assert!(g36_declaration_root(&invalid).is_err(), "{invalid:?}");
        }
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
                println!(
                    "generated routine registry is empty: no executable deployment scenarios were tested"
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("--routines: {e}");
                ExitCode::from(2)
            }
        };
    }
    match g36_declaration_root(&raw_args) {
        Ok(Some(release_root)) => {
            return match cxf_verify::verify_release_declarations(release_root) {
                Ok(()) => {
                    println!("G36 release declaration conformance passed");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("--g36-declarations: {error}");
                    ExitCode::from(2)
                }
            };
        }
        Ok(None) => {}
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::from(2);
        }
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
            "usage: cxf-verify [--replay-only] (--all | <fault-dir>…) (each containing rule.cxf.jsonld + vectors.json)\n       cxf-verify --trace-json <fault-dir> <vectors.json>\n       cxf-verify --routines\n       cxf-verify --g36-declarations <release-root>"
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
