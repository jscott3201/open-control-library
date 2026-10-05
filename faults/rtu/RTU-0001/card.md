---
schema: cxf-library/fault-card/v1
id: RTU-0001
name: Compressor short-cycling
equipment: rtu
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §11, RTU-0001"
  - "Albayati et al. 2023"
  - "Ebrahimifakhar et al. 2020"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [RTU-0002, RTU-0010, RTU-0011, HP-0007]
playbooks: [rtu-compressor-refrigerant]
operating_states: "all active modes (any cooling or heating call)"
preconditions: "The host must report NO_EVAL for the first count_window (1 h) after engine start: while the moving average's window fills, its divisor is elapsed time, so the output is an extrapolated rate rather than a completed-hour count. Unlike AHU-0004 this gate is load-bearing — alarm_delay (15 min) is shorter than count_window, so delayOnInit does not cover the warm-up window and two starts inside the first few minutes can reach a verdict (`warmup_rate_asserts` pins it). The unit must be enabled and calling for cooling or heating: a compressor idle because there is no load produces zero starts, and reporting that as healthy cycling is the opposite of information. comp_status must be bound per compressor — on a two-compressor unit the OR of both statuses hides every start that happens while the other circuit is already running, and undercounts the cycling of each. Host tick interval must lie in [57.2 s, 300 s) with count_scale set to match (see Deviations). When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - comp_status
outputs:
  - name: yFault
    description: True while the number of compressor starts in the trailing count_window has stayed above max_starts_per_hour for at least alarm_delay
params:
  max_starts_per_hour:
    default: 6.0
    unit: "1/h"
    description: Starts per hour above which cycling counts as short-cycling rather than load-following; the reference's ceiling, equivalent to a 10-minute minimum interval between starts
    cxf: cntHigh.t
  count_window:
    default: 3600.0
    unit: s
    description: Trailing window the starts are counted over (1 h). It also fixes the units of max_starts_per_hour; a host that shortens it must retune count_scale with it and read max_starts_per_hour as starts per window rather than per hour
    cxf: rate.delta
  count_scale:
    default: 60.0
    unit: "1"
    description: "Rescales the moving average of the one-tick pulse train back into a start count: k = count_window / host tick interval in seconds. The default 60.0 is 3600/60, correct only at a 60 s tick; a host on a different tick MUST retune this or every count is wrong by the ratio of the two intervals"
    cxf: count.k
  alarm_delay:
    default: 900.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (15 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: RTU compressor and refrigeration circuit
  savings_range: 3-5% efficiency loss while cycling; avoided compressor replacement of $2,000-$8,000
  climate_sensitivity: cooling-dominant
  runtime_estimation: "none in-rule — QUALITATIVE_ONLY; the rule sees a run status and cannot price a start. Size the opportunity host-side from cycling hours x rated compressor power x the 3-5% penalty, and treat the avoided replacement as the larger term"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:2a5f66097f97bf8154323abae2e36ded"
  date: 2026-10-05
---

## Description

A compressor starting ten times an hour is not following load. Every start
draws locked-rotor current through windings that have not cooled, restarts
against a head pressure that has not equalized, and pumps oil out of the sump
faster than the return line brings it back. The efficiency loss is modest — the
reference puts it at 3–5%, the cost of running the first minutes of every cycle
before the coil reaches steady state — but the mechanical damage is what costs
money: a compressor worn out early is $2,000–$8,000 plus the days the space
spends uncooled, which is why this card is PROTECTIVE rather than an efficiency
rule. Short-cycling is a symptom, not a root cause; the rule reports that the
compressor is being asked to start too often and the service call decides why.

## Detection Logic

```
start  = rising edge of comp_status                       one tick wide
count  = MovingAverage(start, count_window) × count_scale  starts in the trailing hour
yFault = count > max_starts_per_hour, sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![RTU-0001 block graph](diagram.svg)

`Logical.Edge` emits `u ∧ ¬pre(u)`, one tick wide, on every OFF→ON transition —
stops and run durations are not counted. `Reals.MovingAverage` is a
continuous-time integral mean, so a one-tick pulse of height 1.0 encloses one
tick interval of area and `n` starts inside the window give
`rate = n · dt / count_window`; multiplying by `count_scale = count_window / dt`
recovers `n`. That makes `count_scale` a function of the host's tick interval,
and this rule is tighter about the tick than the rest of the library — see the
first three Deviations before deploying. The comparison is strict, so exactly
six starts an hour reads clear and seven alarms; on an integer-valued count that
boundary is unambiguous. `persist` then requires the count to stay above the
ceiling for 15 minutes — two to three more starts on a unit already cycling hard
— which rides out a defrost sequence or a one-off pressure trip without leaving
a compressor tearing itself apart for an hour. `delayOnInit = true` holds that
window across a controller restart.

## Possible Diagnoses

1. Thermostat or controller differential set too small — the call is satisfied
   within a minute or two and restarts as soon as the space drifts back
2. Equipment oversized for the load: at part load the unit can only meet the
   call by cycling, and no setting adjusts it
3. Low refrigerant charge — suction pressure falls to the cutout every cycle and
   the low-pressure switch does the cycling
4. Defective run capacitor: the compressor stalls on start and drops out on
   thermal overload
5. Iced or fouled evaporator coil starving the suction side (RTU-0002 sees the
   same coil from the airside)
6. Control board or contactor fault chattering the compressor output

## Energy Impact

PROTECTIVE, MEDIUM confidence, QUALITATIVE_ONLY. The rule sees one boolean and
cannot price a start, so there is no waste term computable from its inputs. Size
the opportunity host-side from cycling hours × rated compressor power × the
reference's 3–5% efficiency penalty, and treat the $2,000–$8,000 avoided
compressor replacement as the larger term — that probabilistic term is why the
fault is severity 2. MEDIUM confidence: the mechanism is not in doubt and the
sources (Albayati et al. 2023; Ebrahimifakhar et al. 2020) are field studies of
packaged-unit faults, but the efficiency figure depends on cycle length and
ambient conditions, which this rule does not measure. Cooling-dominant.

## Emissions Impact

Scope 2, QUALITATIVE_EMISSIONS, MEDIUM confidence. AHU-0001's convention gives
a PROTECTIVE fault with no emitting stream scope "N/A"; this card does not
qualify, because the 3–5% efficiency loss is electricity a compressor actually
draws. The larger emissions term is indirect: a compressor replaced years early
carries the embodied carbon of a new compressor plus its refrigerant charge.
Avoided-emissions basis: N/A.

## Deviations

- **This rule needs a faster tick than the rest of the library, and the reason
  is Nyquist.** A start is visible only if the compressor is seen OFF on one
  tick and ON on the next, so the fastest observable cycling is `1800/dt` starts
  per hour; at the library's usual 300 s tick that ceiling is exactly 6/h — the
  threshold itself — and the rule could never fire. The default `count_scale` is
  therefore 60.0 (a 60 s tick) rather than AHU-0004's 12.0. Combined with the
  ring floor below, a legal deployment has `57.2 s ≤ dt < 300 s`; 60 s is the
  recommended value and the only one these vectors have exercised.
- **`count_scale` is coupled to the host's tick interval and the failure is
  silent.** `k = count_window / dt`, so a host ticking every 120 s must set
  `count_scale` to 30.0; left at 60.0 it reports double the true count and
  alarms on four starts an hour. AHU-0004's deployment constraint verbatim.
- **Minimum tick interval, from the moving average's ring.** Each
  `MovingAverage` keeps a fixed 64-checkpoint ring and drops the oldest
  in-window sample past that. The retained window holds `count_window/dt + 1`
  checkpoints, so `dt ≥ 3600/63` ≈ **57.2 s**. (AHU-0004 quotes `delta/64` =
  56.25 s, which omits the boundary checkpoint; neither card's tick is near it.)
- **`min_run_time` is not in the graph.** The reference lists it as a 5-minute
  tunable but its printed equation is `comp_starts_per_hour >
  max_starts_per_hour` and nothing else, and the starts-per-hour ceiling
  subsumes the protective intent: ten 2-minute runs is both a min-run-time
  violation and 10 starts an hour. A site wanting the stricter per-cycle test
  can add a companion rule.
- **Rolling count built from a moving average, because the block set has no
  windowed counter.** `Integers.OnCounter` counts monotonically from a reset, so
  a trailing-hour count would need a host-driven hourly reset — a tumbling
  count whose verdict depends on where the hour boundary fell. AHU-0004's
  trade, taken again.
- **Startup artifact (a): a spurious first-tick pulse, which costs nothing.**
  `Logical.Edge` compares `u` against `pre_u_start` on the first tick, so a unit
  already running at load registers a start at t = 0. It encloses no area (`dt`
  is zero on the first tick) and never reaches the count, so `pre_u_start` is
  written explicitly as `false` and not exposed as a card parameter.
- **Startup artifact (b): the first hour reads as a rate, and here it can reach
  a verdict.** While `t < count_window` the moving average divides by elapsed
  time, so two starts in the first three minutes read as 40/h — the pace,
  extrapolated. Unlike AHU-0004, `alarm_delay` (15 min) is shorter than the
  window, so that rate can assert (`warmup_rate_asserts` pins it). The host
  NO_EVAL precondition for the first `count_window` is not optional.
- **Strict `>` on a discrete count.** Exactly six starts an hour is clear and
  seven alarms, the reference's `> max_starts_per_hour` read literally. The
  boundary is exact in IEEE-754: `60.0 × (6 × 60 / 3600)` evaluates to precisely
  6.0, so the six-start case is a real pin and not a near-miss.
- **The counting window is half-open.** `rate` compares the accumulated integral
  now against its value one `count_window` ago, so a start exactly
  `count_window` old has just left the window. The reference is silent; it
  matters only on the threshold and it errs toward silence.
- **How long a crossing survives is `count_window` minus the span of the starts
  that caused it.** Seven starts packed into ten minutes hold `cntHigh` for
  nearly an hour and always alarm; seven spread across 54 minutes hold it for
  300 s and never do — same starts per hour, opposite verdicts, and the
  difference is not visible in the printed equation. What the rule reports is
  cycling sustained above the ceiling, not every excursion through it.
- **The reference tags this fault for both RTU and HP.** This card is the
  RTU-family instance (AHU-0025 precedent); the heat-pump sibling would
  restate the graph against a heat-pump compressor status and would have to say
  something about defrost cycles, which are starts that mean nothing is wrong.
- Severity 2 (high), phase 2, method `rule`, and the tunable defaults are the
  reference's chapter 11 card; its §5.8.3 index corroborates and carries no
  severity column. `g36: null` — PNNL/research-derived, not a G36 §5.16.14
  clause.
- Operating states are declared, not gated: the reference marks the fault
  applicable in every active mode, and the graph has nothing to exclude.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a count already above the ceiling at load waits out the full
  15 minutes instead of alarming on the first tick after a restart.

## Notes

Bind `comp_status` per compressor and deploy one instance per circuit, as the
RTU point dictionary requires. An OR across two circuits undercounts: a lag
start while the lead is already running never moves the signal, so a unit whose
lead runs continuously can cycle its lag circuit all afternoon and read healthy.

Remediation follows the [rtu-compressor-refrigerant](../../../playbooks/rtu-compressor-refrigerant.md)
playbook — thermostat differential, then charge, then the run capacitor —
confirming resolution at fewer than 6 starts/hr with a 5-minute minimum on-time
over 48 hours. The one remote check worth doing before the truck roll is
diagnosis 1: if the cooling call is satisfied within a minute or two of every
start, widen the differential. Firing together with RTU-0002 points at the
coil rather than the controls (diagnosis 5); firing alone points at the
differential or the charge.
