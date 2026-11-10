//! The verifier binary on vectors that compare nothing somewhere.
//!
//! Each test writes a bundle (the test-local fixture graph and one `vectors.json`) under the
//! build's temporary directory and runs the real `cxf-verify` on it. A bundle that makes no
//! comparison, or whose clock runs no real schedule, must end with a non-zero exit status, no
//! `PASS` line for the scenario concerned and a message naming the bundle, the scenario and the
//! reason. A control beside the refusals shows that the same bundle shape passes when it does
//! compare.
//!
//! The fixture graph has Real inputs `a` and `b` and the Boolean output
//! `y = (a > 5) and (b > 5)`. Every scenario here holds both inputs at 45, so `y` is true at every
//! tick. The vacuous expectations assert `false`: one comparison would be enough to fail them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BOTH_ABOVE: &[u8] = include_bytes!("fixtures/both_above_threshold.cxf.jsonld");

/// A run that has not ended by now is a failure: a zero `step_s` used to schedule ticks without
/// end.
const DEADLINE: Duration = Duration::from_secs(60);

/// What one run of the verifier left behind.
struct Run {
    /// The bundle directory as it was passed to the verifier, which is how the verifier names it.
    bundle: String,
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Run {
    /// The one output line that reports `marker` (`FAIL` or `ERROR`).
    fn line(&self, marker: &str) -> &str {
        let lines: Vec<&str> = self
            .stdout
            .lines()
            .filter(|line| line.trim_start().starts_with(marker))
            .collect();
        match lines.as_slice() {
            [one] => one,
            other => panic!(
                "expected one `{marker}` line, found {}:\n{}",
                other.len(),
                self.stdout
            ),
        }
    }

    fn passed_scenarios(&self) -> Vec<&str> {
        self.stdout
            .lines()
            .filter_map(|line| line.strip_prefix("  PASS  "))
            .collect()
    }

    /// The run failed, reported no scenario as passed and did not claim overall success.
    fn assert_nothing_passed(&self) {
        assert_eq!(self.status, Some(1), "{}{}", self.stdout, self.stderr);
        assert_eq!(self.passed_scenarios(), [""; 0], "{}", self.stdout);
        assert!(
            !self.stdout.contains("all scenarios passed"),
            "{}",
            self.stdout
        );
    }

    /// The scenario `probe` was refused for `reason`, in a line that names the bundle too.
    fn assert_scenario_refused(&self, reason: &str) {
        self.assert_nothing_passed();
        let line = self.line("FAIL");
        for part in [
            "refused:",
            &format!("bundle `{}`", self.bundle),
            "scenario `probe`",
            reason,
        ] {
            assert!(line.contains(part), "`{part}` is not in: {line}");
        }
    }

    /// The whole vectors file was refused for `reason`, in a line that names the bundle.
    fn assert_bundle_refused(&self, reason: &str) {
        self.assert_nothing_passed();
        let line = self.line("ERROR");
        for part in ["refused:", &format!("bundle `{}`", self.bundle), reason] {
            assert!(line.contains(part), "`{part}` is not in: {line}");
        }
    }
}

/// Write the fixture graph and `vectors` as the bundle `case`.
fn write_bundle(case: &str, vectors: &Value) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("refused_vectors")
        .join(case);
    std::fs::create_dir_all(&dir).expect("create the bundle directory");
    std::fs::write(dir.join("rule.cxf.jsonld"), BOTH_ABOVE).expect("write the rule");
    std::fs::write(
        dir.join("vectors.json"),
        serde_json::to_vec_pretty(vectors).expect("vectors serialize"),
    )
    .expect("write the vectors");
    dir
}

/// Run the verifier with `args` on the bundle at `dir`.
fn run(dir: &Path, args: &[&Path]) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cxf-verify"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the verifier");
    let started = Instant::now();
    while child.try_wait().expect("poll the verifier").is_none() {
        if started.elapsed() > DEADLINE {
            child.kill().expect("stop the verifier");
            child.wait().expect("reap the verifier");
            panic!(
                "the verifier was still running after {DEADLINE:?} on {}",
                dir.display()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().expect("collect the verifier");
    Run {
        bundle: dir.display().to_string(),
        status: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// Replay the bundle `case` as the gate replays a fault bundle, without the card lint (a
/// test-local bundle has no card).
fn replay(case: &str, vectors: &Value) -> Run {
    let dir = write_bundle(case, vectors);
    run(&dir, &[Path::new("--replay-only"), &dir])
}

/// Trace the bundle `case`, as the routine reference replay and the dataset harness do.
fn trace(case: &str, vectors: &Value) -> Run {
    let dir = write_bundle(case, vectors);
    run(
        &dir,
        &[Path::new("--trace-json"), &dir, &dir.join("vectors.json")],
    )
}

/// Ticks at 0, 60 and 120 s.
fn three_ticks() -> Value {
    json!({"step_s": 60, "horizon_s": 120})
}

fn vectors(clock: Value, scenarios: Value) -> Value {
    json!({"schema": "cxf-library/vectors/v1", "clock": clock, "scenarios": scenarios})
}

/// A scenario in which `y` is true at every tick.
fn scenario(name: &str, expect: Value) -> Value {
    json!({"name": name, "inputs": {"a": 45.0, "b": 45.0}, "expect": expect})
}

/// One scenario, `probe`, on the three-tick clock.
fn probe(expect: Value) -> Value {
    vectors(three_ticks(), json!([scenario("probe", expect)]))
}

fn y_is(equals: bool, from_s: f64, to_s: f64) -> Value {
    json!({"output": "y", "from_s": from_s, "to_s": to_s, "equals": equals})
}

#[test]
fn scenario_that_compares_at_every_tick_passes() {
    let run = replay(
        "compares_at_every_tick",
        &probe(json!([y_is(true, 0.0, 120.0)])),
    );
    assert_eq!(run.status, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.passed_scenarios(), ["probe"]);
    assert!(
        run.stdout.contains("all scenarios passed"),
        "{}",
        run.stdout
    );
}

#[test]
fn scenario_with_an_empty_expect_list_is_refused() {
    replay("empty_expect", &probe(json!([]))).assert_scenario_refused("`expect` is empty");
}

#[test]
fn window_past_the_last_tick_is_refused() {
    replay(
        "window_past_the_last_tick",
        &probe(json!([y_is(false, 180.0, 240.0)])),
    )
    .assert_scenario_refused(
        "expectation 1 on `y` has the window 180..240s, which starts after the last tick at 120s",
    );
}

#[test]
fn window_between_two_ticks_is_refused() {
    replay(
        "window_between_two_ticks",
        &probe(json!([y_is(false, 61.0, 119.0)])),
    )
    .assert_scenario_refused(
        "expectation 1 on `y` has the window 61..119s, which falls between two ticks",
    );
}

#[test]
fn window_before_the_first_tick_is_refused() {
    replay(
        "window_before_the_first_tick",
        &probe(json!([y_is(false, -30.0, -1.0)])),
    )
    .assert_scenario_refused(
        "expectation 1 on `y` has the window -30..-1s, which ends before the first tick at 0s",
    );
}

#[test]
fn inverted_window_is_refused() {
    replay("inverted_window", &probe(json!([y_is(false, 120.0, 0.0)]))).assert_scenario_refused(
        "expectation 1 on `y` has the window 120..0s, which ends before it starts",
    );
}

/// One expectation that compares does not carry another that cannot.
#[test]
fn uncovered_window_beside_a_covered_one_is_refused() {
    replay(
        "uncovered_beside_covered",
        &probe(json!([y_is(true, 0.0, 120.0), y_is(false, 180.0, 240.0)])),
    )
    .assert_scenario_refused("expectation 2 on `y` has the window 180..240s");
}

/// The refusal is the scenario's own: its neighbour still replays and passes, and the bundle
/// fails.
#[test]
fn refused_scenario_fails_the_bundle_beside_a_passing_one() {
    let run = replay(
        "refused_beside_passing",
        &vectors(
            three_ticks(),
            json!([
                scenario("compares", json!([y_is(true, 0.0, 120.0)])),
                scenario("probe", json!([])),
            ]),
        ),
    );
    assert_eq!(run.status, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.passed_scenarios(), ["compares"]);
    assert!(
        !run.stdout.contains("all scenarios passed"),
        "{}",
        run.stdout
    );
    let line = run.line("FAIL");
    assert!(line.contains("scenario `probe`"), "{line}");
    assert!(line.contains("`expect` is empty"), "{line}");
}

#[test]
fn vectors_file_with_no_scenarios_is_refused() {
    replay("no_scenarios", &vectors(three_ticks(), json!([])))
        .assert_bundle_refused("`scenarios` is empty");
}

#[test]
fn clock_with_a_negative_step_is_refused() {
    replay(
        "negative_step",
        &vectors(
            json!({"step_s": -60, "horizon_s": 120}),
            json!([scenario("probe", json!([y_is(true, 0.0, 120.0)]))]),
        ),
    )
    .assert_bundle_refused("`clock.step_s` is -60");
}

#[test]
fn clock_with_a_zero_step_is_refused() {
    replay(
        "zero_step",
        &vectors(
            json!({"step_s": 0, "horizon_s": 120}),
            json!([scenario("probe", json!([y_is(true, 0.0, 120.0)]))]),
        ),
    )
    .assert_bundle_refused("`clock.step_s` is 0");
}

#[test]
fn clock_with_a_negative_horizon_is_refused() {
    replay(
        "negative_horizon",
        &vectors(
            json!({"step_s": 60, "horizon_s": -120}),
            json!([scenario("probe", json!([y_is(true, -120.0, 0.0)]))]),
        ),
    )
    .assert_bundle_refused("`clock.horizon_s` is -120");
}

#[test]
fn clock_with_a_zero_horizon_is_refused() {
    replay(
        "zero_horizon",
        &vectors(
            json!({"step_s": 60, "horizon_s": 0}),
            json!([scenario("probe", json!([y_is(true, 0.0, 0.0)]))]),
        ),
    )
    .assert_bundle_refused("`clock.horizon_s` is 0");
}

#[test]
fn trace_of_a_vectors_file_with_no_scenarios_is_refused() {
    let run = trace("trace_no_scenarios", &vectors(three_ticks(), json!([])));
    assert_eq!(run.status, Some(2), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, "", "a refused trace prints no trace document");
    assert!(
        run.stderr.contains("`scenarios` is empty"),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("vectors.json"), "{}", run.stderr);
}

/// A trace compares nothing itself: its caller holds the expected values. The dataset harness
/// sends scenarios with an empty `expect`, and the routine matrix a single tick at `t = 0`.
#[test]
fn trace_still_takes_an_empty_expect_list_and_a_single_tick() {
    let run = trace(
        "trace_single_tick",
        &vectors(
            json!({"step_s": 1, "horizon_s": 0}),
            json!([scenario("probe", json!([]))]),
        ),
    );
    assert_eq!(run.status, Some(0), "{}{}", run.stdout, run.stderr);
    let document: Value = serde_json::from_str(&run.stdout).expect("a trace document");
    assert_eq!(
        document["scenarios"],
        json!([{"name": "probe", "samples": [{"t": 0.0, "outputs": {"y": true}}]}])
    );
}
