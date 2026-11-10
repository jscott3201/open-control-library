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

/// The most ticks one scenario may schedule. A clock is refused when
/// `floor(horizon_s / step_s)`, the index of its last tick, is this or more, on the replay path
/// and on the trace path alike: a step far smaller than its horizon would otherwise run without
/// end.
const MAX_TICKS: f64 = 1_000_000.0;

/// How far a committed Real output may lie from its expected value and still match it, as an
/// absolute difference. It is the one comparison rule for Reals: a vector cannot widen it.
const REAL_TOLERANCE: f64 = 1e-9;

/// A `vectors.json` document (SCHEMA.md, "`vectors.json` contract"). The file, its clock, each
/// scenario and each expectation are closed: a field the contract does not name is refused when
/// the file is read, so a misspelt or retired field cannot be silently ignored.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
struct Clock {
    step_s: f64,
    horizon_s: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
    inputs: serde_json::Map<String, serde_json::Value>,
    expect: Vec<Expect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    output: String,
    from_s: f64,
    to_s: f64,
    equals: serde_json::Value,
}

impl Expect {
    /// Whether the tick at `t` is inside the window, which includes both of its ends.
    fn holds(&self, t: f64) -> bool {
        self.from_s <= t && t <= self.to_s
    }
}

/// One expectation of a scenario, as a refusal names it.
#[derive(Debug, Clone, PartialEq)]
struct Window {
    /// Position in the scenario's `expect` list, counted from one.
    expectation: usize,
    output: String,
    from_s: f64,
    to_s: f64,
}

impl std::fmt::Display for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "expectation {} on `{}` has the window {}..{}s",
            self.expectation, self.output, self.from_s, self.to_s
        )
    }
}

/// Why a vectors file, or one scenario in it, is refused before any tick runs: replaying it
/// would compare nothing where it claims to, its clock describes no schedule or one too long to
/// run, or its report could not tell two scenarios apart. A refused scenario is a failed
/// scenario; the verifier never reports it as passed.
#[derive(Debug, Clone, PartialEq)]
enum Vacuous {
    /// `clock.step_s` is not a finite number greater than zero.
    StepNotPositive { step_s: f64 },
    /// `clock.horizon_s` is not a finite number greater than zero.
    HorizonNotPositive { horizon_s: f64 },
    /// The clock schedules more than [`MAX_TICKS`] ticks: `floor(horizon_s / step_s)` is
    /// [`MAX_TICKS`] or more.
    TooManyTicks { step_s: f64, horizon_s: f64 },
    /// The vectors file holds no scenario.
    NoScenarios,
    /// Two scenarios of the vectors file have this name, so a report line that carries it would
    /// not say which of them passed or failed.
    DuplicateScenarioName { name: String },
    /// The scenario's `expect` list is empty.
    NoExpectations,
    /// The window ends before it starts, so no time is inside it.
    InvertedWindow(Window),
    /// The window ends before the first tick, at 0 s.
    WindowBeforeFirstTick(Window),
    /// The window starts after the last tick the clock schedules.
    WindowAfterLastTick { window: Window, last_tick_s: f64 },
    /// The window lies inside the schedule and between two neighbouring ticks.
    WindowBetweenTicks { window: Window, step_s: f64 },
}

impl std::fmt::Display for Vacuous {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StepNotPositive { step_s } => write!(
                f,
                "`clock.step_s` is {step_s}: it must be a finite number greater than zero"
            ),
            Self::HorizonNotPositive { horizon_s } => write!(
                f,
                "`clock.horizon_s` is {horizon_s}: it must be a finite number greater than zero"
            ),
            // `{:?}` writes a very large or very small number with an exponent, where `{}` would
            // write out hundreds of digits.
            Self::TooManyTicks { step_s, horizon_s } => write!(
                f,
                "`clock` schedules more than 1,000,000 ticks (`step_s` {step_s:?}, `horizon_s` {horizon_s:?}): `floor(horizon_s / step_s)` must be below 1,000,000"
            ),
            Self::NoScenarios => {
                write!(f, "`scenarios` is empty: the vectors file compares nothing")
            }
            Self::DuplicateScenarioName { name } => write!(
                f,
                "`scenarios` holds the name `{name}` more than once: a scenario name must be unique in its vectors file"
            ),
            Self::NoExpectations => {
                write!(f, "`expect` is empty: the scenario compares nothing")
            }
            Self::InvertedWindow(window) => {
                write!(
                    f,
                    "{window}, which ends before it starts: no tick is inside it"
                )
            }
            Self::WindowBeforeFirstTick(window) => write!(
                f,
                "{window}, which ends before the first tick at 0s: no tick is inside it"
            ),
            Self::WindowAfterLastTick {
                window,
                last_tick_s,
            } => write!(
                f,
                "{window}, which starts after the last tick at {last_tick_s}s: no tick is inside it"
            ),
            Self::WindowBetweenTicks { window, step_s } => write!(
                f,
                "{window}, which falls between two ticks (one every {step_s}s from 0s): no tick is inside it"
            ),
        }
    }
}

/// Why a scenario did not pass.
#[derive(Debug, PartialEq)]
enum ScenarioError {
    /// Refused before any tick ran.
    Refused(Vacuous),
    /// Replayed, or prepared for replay, and stopped: a name did not resolve on the boundary, a
    /// frame was refused, or a committed output was not the expected value.
    Failed(String),
}

impl From<Vacuous> for ScenarioError {
    fn from(reason: Vacuous) -> Self {
        Self::Refused(reason)
    }
}

impl From<String> for ScenarioError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// The message of a refusal: the bundle, the scenario when the refusal is one scenario's, and the
/// reason.
fn refusal(bundle: &Path, scenario: Option<&str>, reason: &Vacuous) -> String {
    match scenario {
        Some(name) => format!(
            "refused: bundle `{}`, scenario `{name}`: {reason}",
            bundle.display()
        ),
        None => format!("refused: bundle `{}`: {reason}", bundle.display()),
    }
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

/// Whether a committed output is the expected value: a Boolean or an Integer exactly, a Real
/// within [`REAL_TOLERANCE`]. A value of another type than the expected one never matches, and
/// neither does a Real that is not a number.
fn values_match(got: &Value, expected: &serde_json::Value) -> bool {
    match (got, expected) {
        (Value::Boolean(g), serde_json::Value::Bool(e)) => g == e,
        (Value::Real(g), serde_json::Value::Number(n)) => {
            n.as_f64().is_some_and(|e| (g - e).abs() <= REAL_TOLERANCE)
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

/// Index of the last tick, `floor(horizon_s / step_s)`.
fn last_tick_index(clock: &Clock) -> u64 {
    (clock.horizon_s / clock.step_s).floor() as u64
}

/// Tick times `k * step_s` for `k = 0 ..= floor(horizon_s / step_s)`.
fn tick_times(clock: &Clock) -> impl Iterator<Item = f64> {
    let step_s = clock.step_s;
    (0..=last_tick_index(clock)).map(move |k| k as f64 * step_s)
}

/// A replay clock must describe a schedule the verifier will run to its end: a finite step and a
/// finite horizon, both greater than zero, and at most [`MAX_TICKS`] ticks. A step of zero or
/// less schedules no second tick, and a zero step never reaches the horizon. A step far smaller
/// than its horizon schedules ticks without end in practice: the quotient of two finite positive
/// numbers can be as large as infinity, which is refused like any other count over the limit.
fn check_clock(clock: &Clock) -> Result<(), Vacuous> {
    if !(clock.step_s.is_finite() && clock.step_s > 0.0) {
        return Err(Vacuous::StepNotPositive {
            step_s: clock.step_s,
        });
    }
    if !(clock.horizon_s.is_finite() && clock.horizon_s > 0.0) {
        return Err(Vacuous::HorizonNotPositive {
            horizon_s: clock.horizon_s,
        });
    }
    if (clock.horizon_s / clock.step_s).floor() >= MAX_TICKS {
        return Err(Vacuous::TooManyTicks {
            step_s: clock.step_s,
            horizon_s: clock.horizon_s,
        });
    }
    Ok(())
}

/// The first scenario name that an earlier scenario in the list already has.
fn duplicate_scenario_name(scenarios: &[Scenario]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    scenarios
        .iter()
        .map(|scenario| scenario.name.as_str())
        .find(|name| !seen.insert(*name))
}

/// What must hold of a vectors file before any of its scenarios is replayed for a verdict: a
/// clock that describes a schedule of at most [`MAX_TICKS`] ticks, at least one scenario, and no
/// name shared by two scenarios.
fn check_vectors(vectors: &Vectors) -> Result<(), Vacuous> {
    check_clock(&vectors.clock)?;
    if vectors.scenarios.is_empty() {
        return Err(Vacuous::NoScenarios);
    }
    if let Some(name) = duplicate_scenario_name(&vectors.scenarios) {
        return Err(Vacuous::DuplicateScenarioName {
            name: name.to_string(),
        });
    }
    Ok(())
}

/// What must hold of a scenario before it is replayed for a verdict: it has at least one
/// expectation, and every expectation's window holds at least one tick of `clock`, so that a pass
/// means every expectation was compared with a committed output at least once.
fn check_scenario(clock: &Clock, scenario: &Scenario) -> Result<(), Vacuous> {
    check_clock(clock)?;
    if scenario.expect.is_empty() {
        return Err(Vacuous::NoExpectations);
    }
    for (index, expect) in scenario.expect.iter().enumerate() {
        if tick_times(clock).any(|t| expect.holds(t)) {
            continue;
        }
        let window = Window {
            expectation: index + 1,
            output: expect.output.clone(),
            from_s: expect.from_s,
            to_s: expect.to_s,
        };
        let last_tick_s = last_tick_index(clock) as f64 * clock.step_s;
        return Err(if expect.from_s > expect.to_s {
            Vacuous::InvertedWindow(window)
        } else if expect.to_s < 0.0 {
            Vacuous::WindowBeforeFirstTick(window)
        } else if expect.from_s > last_tick_s {
            Vacuous::WindowAfterLastTick {
                window,
                last_tick_s,
            }
        } else {
            Vacuous::WindowBetweenTicks {
                window,
                step_s: clock.step_s,
            }
        });
    }
    Ok(())
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
                    if let Some(unknown) = step.as_object().and_then(|fields| {
                        fields
                            .keys()
                            .find(|field| !matches!(field.as_str(), "t" | "value"))
                    }) {
                        return Err(format!(
                            "input `{name}`: step has the unknown field `{unknown}`"
                        ));
                    }
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

/// Replay one scenario for a verdict. `Ok` means every expectation was compared with the
/// committed output at one tick or more and matched at each: a scenario that would compare
/// nothing somewhere is refused before the engine is loaded.
fn run_scenario(
    rule_bytes: &[u8],
    clock: &Clock,
    scenario: &Scenario,
) -> Result<(), ScenarioError> {
    check_scenario(clock, scenario)?;
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
            if !exp.holds(t) {
                continue;
            }
            let got = committed_output(&frame, path)?;
            if !values_match(got, &exp.equals) {
                return Err(ScenarioError::Failed(format!(
                    "t={t}s: `{}` = {} but expected {} (window {}..{}s)",
                    exp.output,
                    fmt_value(got),
                    exp.equals,
                    exp.from_s,
                    exp.to_s
                )));
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
    if !n_ticks.is_finite() || n_ticks >= MAX_TICKS {
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
    if vectors.scenarios.is_empty() {
        return Err(format!(
            "{}: {}",
            vectors_path.display(),
            Vacuous::NoScenarios
        ));
    }
    if let Some(name) = duplicate_scenario_name(&vectors.scenarios) {
        return Err(format!(
            "{}: {}",
            vectors_path.display(),
            Vacuous::DuplicateScenarioName {
                name: name.to_string()
            }
        ));
    }
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
    check_vectors(&vectors).map_err(|reason| refusal(dir, None, &reason))?;

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
            Err(ScenarioError::Refused(reason)) => {
                all_pass = false;
                println!(
                    "  FAIL  {} — {}",
                    scenario.name,
                    refusal(dir, Some(&scenario.name), &reason)
                );
            }
            Err(ScenarioError::Failed(msg)) => {
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
        Clock, InputEvent, InputSchedule, Scenario, ScenarioError, Vacuous, Vectors, Window,
        check_scenario, check_vectors, g36_declaration_root, refusal, run_scenario, tick_times,
        trace_scenario, validate_generated_registry, validate_trace_clock, values_match,
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

    /// The message of a scenario that was replayed, or prepared for replay, and failed: `why`
    /// says what is wrong with it. A scenario refused before any tick is a different error.
    fn failed(verdict: Result<(), ScenarioError>, why: &str) -> String {
        match verdict {
            Err(ScenarioError::Failed(message)) => message,
            other => panic!("expected a failed replay ({why}), got {other:?}"),
        }
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
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &partial),
            "incomplete frame",
        );
        assert!(
            error.contains("both_above.b` has no value at t=0s"),
            "{error}"
        );

        let late = scenario(
            serde_json::json!({"a": 45.0, "b": [{"t": 60, "value": 0.0}]}),
            y_stays_false,
        );
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &late),
            "no value at the first tick",
        );
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
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &flipped),
            "y is true, the expectation says false",
        );
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
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &one_wide_window),
            "y is true at the 60 s tick",
        );
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
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &last_tick_wrong),
            "y is false again at the 120 s tick",
        );
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
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &misspelt),
            "the graph has no output `yy`",
        );
        assert!(
            error.contains("no boundary output matching `yy`"),
            "{error}"
        );

        // An internal connector is not a boundary output, whatever its value.
        let internal = scenario(
            serde_json::json!({"a": 45.0, "b": 45.0}),
            serde_json::json!([{"output": "u1", "from_s": 0, "to_s": 120, "equals": true}]),
        );
        let error = failed(
            run_scenario(BOTH_ABOVE, &THREE_TICKS, &internal),
            "`both.u1` is internal",
        );
        assert!(
            error.contains("no boundary output matching `u1`"),
            "{error}"
        );
    }

    /// The verdict on a scenario with these expectations over [`THREE_TICKS`], with both inputs
    /// above their thresholds so that `y` is true at every tick. The expectations refused below
    /// assert `false`: a single comparison would fail them, so `Ok` for one of them can only mean
    /// that nothing was compared.
    fn verdict_over_three_ticks(expect: serde_json::Value) -> Result<(), ScenarioError> {
        run_scenario(
            BOTH_ABOVE,
            &THREE_TICKS,
            &scenario(serde_json::json!({"a": 45.0, "b": 45.0}), expect),
        )
    }

    fn y_is(equals: bool, from_s: f64, to_s: f64) -> serde_json::Value {
        serde_json::json!({"output": "y", "from_s": from_s, "to_s": to_s, "equals": equals})
    }

    fn refused(reason: Vacuous) -> Result<(), ScenarioError> {
        Err(ScenarioError::Refused(reason))
    }

    /// The expectation at position `expectation` (from one) on `y`.
    fn window(expectation: usize, from_s: f64, to_s: f64) -> Window {
        Window {
            expectation,
            output: "y".to_string(),
            from_s,
            to_s,
        }
    }

    #[test]
    fn scenario_with_an_empty_expect_list_is_refused() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([])),
            refused(Vacuous::NoExpectations)
        );
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(true, 0.0, 120.0)])),
            Ok(())
        );
    }

    #[test]
    fn window_after_the_last_tick_is_refused() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, 180.0, 240.0)])),
            refused(Vacuous::WindowAfterLastTick {
                window: window(1, 180.0, 240.0),
                last_tick_s: 120.0,
            })
        );
        // A window that starts on the last tick and runs past the horizon holds that tick.
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(true, 120.0, 240.0)])),
            Ok(())
        );

        // The last tick is the last whole step, not the horizon: with ticks at 0, 60 and 120 s
        // and a horizon of 150 s, a window inside the horizon can still be past the last tick.
        let horizon_past_the_last_tick = Clock {
            step_s: 60.0,
            horizon_s: 150.0,
        };
        let inside_the_horizon = scenario(
            serde_json::json!({"a": 45.0, "b": 45.0}),
            serde_json::json!([y_is(false, 130.0, 150.0)]),
        );
        assert_eq!(
            run_scenario(BOTH_ABOVE, &horizon_past_the_last_tick, &inside_the_horizon),
            refused(Vacuous::WindowAfterLastTick {
                window: window(1, 130.0, 150.0),
                last_tick_s: 120.0,
            })
        );
    }

    #[test]
    fn window_between_two_ticks_is_refused() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, 61.0, 119.0)])),
            refused(Vacuous::WindowBetweenTicks {
                window: window(1, 61.0, 119.0),
                step_s: 60.0,
            })
        );
        // A window of no width between two ticks holds none either.
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, 90.0, 90.0)])),
            refused(Vacuous::WindowBetweenTicks {
                window: window(1, 90.0, 90.0),
                step_s: 60.0,
            })
        );
        // Both ends are inside the window: one that only touches a tick holds it.
        for (from_s, to_s) in [(60.0, 60.0), (61.0, 120.0), (0.0, 59.0)] {
            assert_eq!(
                verdict_over_three_ticks(serde_json::json!([y_is(true, from_s, to_s)])),
                Ok(()),
                "{from_s}..{to_s}s"
            );
        }
    }

    #[test]
    fn window_before_the_first_tick_is_refused() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, -30.0, -1.0)])),
            refused(Vacuous::WindowBeforeFirstTick(window(1, -30.0, -1.0)))
        );
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(true, -30.0, 0.0)])),
            Ok(())
        );
    }

    #[test]
    fn inverted_window_is_refused() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, 120.0, 0.0)])),
            refused(Vacuous::InvertedWindow(window(1, 120.0, 0.0)))
        );
        // Inverted is the reason wherever the window lies, past the last tick included.
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([y_is(false, 240.0, 180.0)])),
            refused(Vacuous::InvertedWindow(window(1, 240.0, 180.0)))
        );
    }

    /// One expectation that is compared does not carry another that is not.
    #[test]
    fn every_expectation_must_hold_a_tick_not_only_the_first() {
        assert_eq!(
            verdict_over_three_ticks(serde_json::json!([
                y_is(true, 0.0, 120.0),
                y_is(false, 180.0, 240.0)
            ])),
            refused(Vacuous::WindowAfterLastTick {
                window: window(2, 180.0, 240.0),
                last_tick_s: 120.0,
            })
        );
    }

    /// The check a scenario goes through before it is replayed, for one whose window holds every
    /// tick there could be, on the clock `step_s`, `horizon_s`. The check executes no frame, but
    /// it does walk the clock's tick times until one falls inside the window: the clock is
    /// checked first, so that walk is over at most a million ticks, and this window ends it at
    /// the first.
    fn check_on_clock(step_s: f64, horizon_s: f64) -> Result<(), Vacuous> {
        check_scenario(
            &Clock { step_s, horizon_s },
            &scenario(
                serde_json::json!({"a": 45.0, "b": 45.0}),
                serde_json::json!([y_is(true, f64::MIN, f64::MAX)]),
            ),
        )
    }

    #[test]
    fn clock_without_a_positive_finite_step_is_refused() {
        assert_eq!(check_on_clock(60.0, 120.0), Ok(()));
        // A negative step used to run a single tick; a zero step scheduled ticks without end.
        for step_s in [-60.0, 0.0, -0.0, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                check_on_clock(step_s, 120.0),
                Err(Vacuous::StepNotPositive { step_s }),
                "step_s = {step_s}"
            );
        }
        assert!(matches!(
            check_on_clock(f64::NAN, 120.0),
            Err(Vacuous::StepNotPositive { step_s }) if step_s.is_nan()
        ));
        // The scenario is refused for its clock when it is replayed, too.
        assert_eq!(
            run_scenario(
                BOTH_ABOVE,
                &Clock {
                    step_s: -60.0,
                    horizon_s: 120.0,
                },
                &scenario(
                    serde_json::json!({"a": 45.0, "b": 45.0}),
                    serde_json::json!([y_is(true, -60.0, 120.0)]),
                ),
            ),
            refused(Vacuous::StepNotPositive { step_s: -60.0 })
        );
    }

    #[test]
    fn clock_without_a_positive_finite_horizon_is_refused() {
        // A horizon shorter than one step is positive: it schedules the tick at 0 s.
        assert_eq!(check_on_clock(60.0, 30.0), Ok(()));
        for horizon_s in [-120.0, 0.0, -0.0, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                check_on_clock(60.0, horizon_s),
                Err(Vacuous::HorizonNotPositive { horizon_s }),
                "horizon_s = {horizon_s}"
            );
        }
        assert!(matches!(
            check_on_clock(60.0, f64::NAN),
            Err(Vacuous::HorizonNotPositive { horizon_s }) if horizon_s.is_nan()
        ));
    }

    /// A clock whose step is far smaller than its horizon is refused before the tick schedule
    /// is walked: the walk, and the replay after it, would not end.
    #[test]
    fn clock_that_schedules_more_than_a_million_ticks_is_refused() {
        let too_many = |step_s, horizon_s| Err(Vacuous::TooManyTicks { step_s, horizon_s });
        // The limit is on the index of the last tick, `floor(horizon_s / step_s)`.
        assert_eq!(check_on_clock(1.0, 999_999.0), Ok(()));
        assert_eq!(check_on_clock(1.0, 999_999.5), Ok(()));
        assert_eq!(check_on_clock(1.0, 1_000_000.0), too_many(1.0, 1_000_000.0));
        assert_eq!(check_on_clock(60.0, 59_999_999.0), Ok(()));
        assert_eq!(
            check_on_clock(60.0, 60_000_000.0),
            too_many(60.0, 60_000_000.0)
        );
        // The two shapes that ran without end: a step too small and a horizon too long.
        assert_eq!(check_on_clock(1e-300, 1.0), too_many(1e-300, 1.0));
        assert_eq!(check_on_clock(60.0, 1e300), too_many(60.0, 1e300));
        // Two finite numbers whose quotient is not: still a count over the limit.
        assert_eq!(check_on_clock(1e-300, 1e300), too_many(1e-300, 1e300));
        assert_eq!(
            check_on_clock(f64::MIN_POSITIVE, f64::MAX),
            too_many(f64::MIN_POSITIVE, f64::MAX)
        );

        // A window no tick is inside does not get the schedule walked for it first: the clock
        // is the reason, for the scenario and for the file.
        let unbounded = Clock {
            step_s: 1e-300,
            horizon_s: 1.0,
        };
        let past_the_horizon = scenario(
            serde_json::json!({"a": 45.0, "b": 45.0}),
            serde_json::json!([y_is(false, 2.0, 3.0)]),
        );
        assert_eq!(
            run_scenario(BOTH_ABOVE, &unbounded, &past_the_horizon),
            refused(Vacuous::TooManyTicks {
                step_s: 1e-300,
                horizon_s: 1.0,
            })
        );
        let one_scenario = serde_json::json!([{"name": "probe", "inputs": {}, "expect": []}]);
        assert_eq!(
            check_vectors(&vectors(
                serde_json::json!({"step_s": 60, "horizon_s": 1e300}),
                one_scenario
            )),
            too_many(60.0, 1e300)
        );
    }

    fn vectors(clock: serde_json::Value, scenarios: serde_json::Value) -> Vectors {
        serde_json::from_value(serde_json::json!({
            "schema": "cxf-library/vectors/v1",
            "clock": clock,
            "scenarios": scenarios
        }))
        .expect("vectors deserialize")
    }

    #[test]
    fn vectors_file_with_no_scenarios_is_refused() {
        let three_ticks = serde_json::json!({"step_s": 60, "horizon_s": 120});
        assert_eq!(
            check_vectors(&vectors(three_ticks.clone(), serde_json::json!([]))),
            Err(Vacuous::NoScenarios)
        );
        let one_scenario = serde_json::json!([{"name": "probe", "inputs": {}, "expect": []}]);
        assert_eq!(
            check_vectors(&vectors(three_ticks, one_scenario.clone())),
            Ok(())
        );
        // The clock is the file's too, and is refused before any scenario is looked at.
        assert_eq!(
            check_vectors(&vectors(
                serde_json::json!({"step_s": -60, "horizon_s": 120}),
                one_scenario.clone()
            )),
            Err(Vacuous::StepNotPositive { step_s: -60.0 })
        );
        assert_eq!(
            check_vectors(&vectors(
                serde_json::json!({"step_s": 60, "horizon_s": 0}),
                one_scenario
            )),
            Err(Vacuous::HorizonNotPositive { horizon_s: 0.0 })
        );
    }

    #[test]
    fn vectors_file_with_two_scenarios_of_one_name_is_refused() {
        let three_ticks = serde_json::json!({"step_s": 60, "horizon_s": 120});
        let named = |names: &[&str]| {
            let scenarios: Vec<serde_json::Value> = names
                .iter()
                .map(|name| serde_json::json!({"name": name, "inputs": {}, "expect": []}))
                .collect();
            vectors(three_ticks.clone(), serde_json::Value::Array(scenarios))
        };
        assert_eq!(check_vectors(&named(&["first", "second", "third"])), Ok(()));
        // Names are compared as written: a different case is a different name.
        assert_eq!(check_vectors(&named(&["probe", "Probe"])), Ok(()));
        assert_eq!(
            check_vectors(&named(&["probe", "probe"])),
            Err(Vacuous::DuplicateScenarioName {
                name: "probe".to_string()
            })
        );
        // The repeat need not be a neighbour, and the first name repeated is the one reported.
        assert_eq!(
            check_vectors(&named(&["first", "second", "third", "second", "first"])),
            Err(Vacuous::DuplicateScenarioName {
                name: "second".to_string()
            })
        );
    }

    /// The message serde gives for `document` when it does not deserialize as a vectors file.
    fn unreadable(document: serde_json::Value) -> String {
        match serde_json::from_value::<Vectors>(document) {
            Ok(_) => panic!("the document deserialized as a vectors file"),
            Err(error) => error.to_string(),
        }
    }

    /// A vectors file with `extra` merged into the object at `pointer` (a JSON pointer; the empty
    /// pointer is the file itself). Without `extra` it is a complete file that deserializes.
    fn vectors_document_with(pointer: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut document = serde_json::json!({
            "schema": "cxf-library/vectors/v1",
            "clock": {"step_s": 60, "horizon_s": 120},
            "scenarios": [{
                "name": "probe",
                "description": "both inputs above their thresholds",
                "inputs": {"a": 45.0, "b": 45.0},
                "expect": [{"output": "y", "from_s": 0, "to_s": 120, "equals": true}]
            }]
        });
        let target = document
            .pointer_mut(pointer)
            .and_then(serde_json::Value::as_object_mut)
            .expect("the pointer names an object of the document");
        for (field, value) in extra.as_object().expect("extra fields are an object") {
            target.insert(field.clone(), value.clone());
        }
        document
    }

    #[test]
    fn field_the_contract_does_not_name_is_refused_at_every_level_of_the_file() {
        serde_json::from_value::<Vectors>(vectors_document_with("", serde_json::json!({})))
            .expect("the document deserializes when nothing is added to it");
        for (pointer, level) in [
            ("", "the file"),
            ("/clock", "the clock"),
            ("/scenarios/0", "a scenario"),
            ("/scenarios/0/expect/0", "an expectation"),
        ] {
            let error = unreadable(vectors_document_with(
                pointer,
                serde_json::json!({"note": "not in the contract"}),
            ));
            assert!(error.contains("unknown field `note`"), "{level}: {error}");
        }
    }

    /// `tolerance` was an optional field of an expectation. No value of it is accepted now,
    /// neither the old default nor one so wide that the comparison could not fail.
    #[test]
    fn expectation_that_carries_a_tolerance_is_refused_as_an_unknown_field() {
        for tolerance in [1e-9, 0.0, 1e6, 1e308, -1.0] {
            let error = unreadable(vectors_document_with(
                "/scenarios/0/expect/0",
                serde_json::json!({"tolerance": tolerance}),
            ));
            assert!(
                error.contains("unknown field `tolerance`"),
                "tolerance {tolerance}: {error}"
            );
        }
    }

    #[test]
    fn input_step_with_an_unknown_field_fails_its_scenario_and_names_the_field() {
        let y_stays_true =
            serde_json::json!([{"output": "y", "from_s": 0, "to_s": 120, "equals": true}]);
        let plain = scenario(
            serde_json::json!({"a": 45.0, "b": [{"t": 0, "value": 45.0}]}),
            y_stays_true.clone(),
        );
        run_scenario(BOTH_ABOVE, &THREE_TICKS, &plain).expect("a step of `t` and `value` replays");

        let extra = scenario(
            serde_json::json!({"a": 45.0, "b": [{"t": 0, "value": 45.0, "hold": "linear"}]}),
            y_stays_true,
        );
        assert_eq!(
            failed(
                run_scenario(BOTH_ABOVE, &THREE_TICKS, &extra),
                "a step has only `t` and `value`",
            ),
            "input `b`: step has the unknown field `hold`"
        );
    }

    #[test]
    fn refusal_names_the_bundle_the_scenario_and_the_reason() {
        let bundle = Path::new("faults/ahu/AHU-0001");
        assert_eq!(
            refusal(bundle, Some("probe"), &Vacuous::NoExpectations),
            "refused: bundle `faults/ahu/AHU-0001`, scenario `probe`: `expect` is empty: the scenario compares nothing"
        );
        assert_eq!(
            refusal(bundle, None, &Vacuous::NoScenarios),
            "refused: bundle `faults/ahu/AHU-0001`: `scenarios` is empty: the vectors file compares nothing"
        );
    }

    #[test]
    fn each_reason_states_the_value_or_the_window_that_is_wrong() {
        let reasons = [
            (
                Vacuous::StepNotPositive { step_s: -60.0 },
                "`clock.step_s` is -60: it must be a finite number greater than zero",
            ),
            (
                Vacuous::HorizonNotPositive { horizon_s: 0.0 },
                "`clock.horizon_s` is 0: it must be a finite number greater than zero",
            ),
            (
                Vacuous::TooManyTicks {
                    step_s: 1e-300,
                    horizon_s: 1.0,
                },
                "`clock` schedules more than 1,000,000 ticks (`step_s` 1e-300, `horizon_s` 1.0): `floor(horizon_s / step_s)` must be below 1,000,000",
            ),
            (
                Vacuous::TooManyTicks {
                    step_s: 60.0,
                    horizon_s: 1e300,
                },
                "`clock` schedules more than 1,000,000 ticks (`step_s` 60.0, `horizon_s` 1e300): `floor(horizon_s / step_s)` must be below 1,000,000",
            ),
            (
                Vacuous::DuplicateScenarioName {
                    name: "probe".to_string(),
                },
                "`scenarios` holds the name `probe` more than once: a scenario name must be unique in its vectors file",
            ),
            (
                Vacuous::InvertedWindow(window(1, 120.0, 0.0)),
                "expectation 1 on `y` has the window 120..0s, which ends before it starts: no tick is inside it",
            ),
            (
                Vacuous::WindowBeforeFirstTick(window(2, -30.0, -1.0)),
                "expectation 2 on `y` has the window -30..-1s, which ends before the first tick at 0s: no tick is inside it",
            ),
            (
                Vacuous::WindowAfterLastTick {
                    window: window(3, 180.0, 240.0),
                    last_tick_s: 120.0,
                },
                "expectation 3 on `y` has the window 180..240s, which starts after the last tick at 120s: no tick is inside it",
            ),
            (
                Vacuous::WindowBetweenTicks {
                    window: window(4, 61.0, 119.0),
                    step_s: 60.0,
                },
                "expectation 4 on `y` has the window 61..119s, which falls between two ticks (one every 60s from 0s): no tick is inside it",
            ),
        ];
        for (reason, message) in reasons {
            assert_eq!(reason.to_string(), message);
        }
    }

    #[test]
    fn expected_value_matches_only_its_own_type_and_reals_only_within_the_fixed_tolerance() {
        use serde_json::json;
        assert!(values_match(&Value::Boolean(true), &json!(true)));
        assert!(!values_match(&Value::Boolean(true), &json!(false)));
        // A Real matches within 1e-9 of the expected value, on either side, and no further.
        assert!(values_match(&Value::Real(0.6), &json!(0.6)));
        assert!(values_match(&Value::Real(0.6), &json!(0.6 + 5e-10)));
        assert!(values_match(&Value::Real(0.6), &json!(0.6 - 5e-10)));
        assert!(!values_match(&Value::Real(0.6), &json!(0.6 + 2e-9)));
        assert!(!values_match(&Value::Real(0.6), &json!(0.6 - 2e-9)));
        assert!(!values_match(&Value::Real(0.6), &json!(1e6)));
        assert!(!values_match(&Value::Real(f64::NAN), &json!(1.0)));
        assert!(!values_match(&Value::Real(f64::INFINITY), &json!(1.0)));
        assert!(values_match(&Value::Integer(3), &json!(3)));
        assert!(!values_match(&Value::Integer(3), &json!(4)));
        assert!(!values_match(&Value::Boolean(true), &json!(1)));
        assert!(!values_match(&Value::Real(1.0), &json!(true)));
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

    /// Parsing only: a file of `schema`, `clock` and `scenarios`, with no routine identity beside
    /// them, deserializes. The replay path goes on to refuse this one, which has a zero horizon
    /// and no scenario; `--trace-json` refuses it for the missing scenario.
    #[test]
    fn vectors_file_of_schema_clock_and_scenarios_alone_deserializes() {
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
