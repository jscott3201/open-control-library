---
schema: cxf-library/fault-card/v1
id: AHU-0004
name: Excessive operating state changes per hour
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0004"
  - "G36 §5.16.14 FC#4"
g36: "§5.16.14 FC#4"
clusters: [CLU-01]
suppresses: []
suppressed_by: []
related: [AHU-0016, AHU-0022]
playbooks: [simultaneous-hc]
operating_states: "OS 1–5 (all)"
preconditions: "The host must report NO_EVAL for the first count_window (1 h) after engine start: while the moving average's window fills, its divisor is elapsed time, so the output is an extrapolated rate rather than a completed-hour count. `delayOnInit = true` on persist already blocks any assertion before 3600 s, so the two windows coincide. The operating_state encoding must be stable for the life of the deployment — re-mapping the enum mid-stream registers as a transition on every point that moved. Operator activity is not a fault: periods of commissioning, manual mode forcing, or scheduled occupancy testing must be excluded host-side, since every deliberate mode change counts the same as an oscillation. When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - operating_state
outputs:
  - name: yFault
    description: True while the number of operating-state transitions in the trailing count_window has stayed above os_max for at least alarm_delay
params:
  os_max:
    default: 7.0
    unit: "1/h"
    description: Transitions per hour above which the sequence counts as unstable rather than load-following
    cxf: cntHigh.t
  count_window:
    default: 3600.0
    unit: s
    description: Trailing window the transitions are counted over (1 h). It also fixes the units of os_max; a host that shortens it must retune count_scale with it and read os_max as transitions per window rather than per hour
    cxf: rate.delta
  count_scale:
    default: 12.0
    unit: "1"
    description: "Rescales the moving average of the one-tick pulse train back into a transition count: k = count_window / host tick interval in seconds. The default 12.0 is 3600/300, correct only at a 300 s tick; a host on a different tick MUST retune this or every count is wrong by the ratio of the two intervals"
    cxf: count.k
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous fault persistence required before the alarm asserts (60 min — the reference ch.9 card's own AlarmDelay for FC#4; G36-2018 Table 5.16.14.5 applies a uniform 30 min AlarmDelay to all fifteen FCs)"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU sequencing / actuator wear
  savings_range: 1-3% of AHU energy from valve and damper wear plus the losses of transitions that undo each other
  climate_sensitivity: neutral
  runtime_estimation: "none in-rule — QUALITATIVE_ONLY; size the opportunity per Energy Impact Reference §4.4 from unstable hours × AHU coil and fan power, as for AHU-0022"
emissions:
  scope: "1+2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:e8de53811e08e20ebd38bc83fa7a602f"
  date: 2026-10-05
---

## Description

The sequence cannot decide what it is doing. An AHU walking between heating,
cooling, economizer, and off more than a few times an hour is not following
load — load does not move that fast — it is chasing a changeover threshold with
nothing to hold it on one side. Valves and dampers stroke, control loops
restart from a new setpoint and overshoot, and air conditioned one way is
conditioned the other way minutes later. The cause is nearly always a missing
or undersized deadband, in the changeover logic or in the zone demand
aggregation feeding it; a close second is upstream sensor noise crossing a
threshold every few minutes with the sequencing logic working as written. A
member fault of CLU-01 (Simultaneous Heating & Cooling).

## Detection Logic

```
pulse  = (operating_state ≠ previous tick's operating_state)   one tick wide
count  = MovingAverage(pulse, count_window) × count_scale       transitions in the trailing hour
yFault = count > os_max, sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0004 block graph](diagram.svg)

`Reals.MovingAverage` is a continuous-time integral mean: it accumulates `u·dt`
and divides by the window, so a one-tick pulse of height 1.0 encloses one tick
interval of area and `n` transitions in the trailing hour give
`rate = n · dt / count_window`. `count_scale = count_window / dt` = 3600/300 =
12 recovers `n`, which couples the parameter to the host's tick interval — the
one thing about this rule that can be got wrong silently (see Deviations).
Only whether the state moved is consumed; `chg.up` and `chg.down` are declared
and left unconnected, since the destination state says nothing about
oscillation. The threshold is strict, so exactly seven transitions an hour
reads clear and eight alarms; the arithmetic is exact at that boundary in
IEEE-754, not approximately exact. `persist` then requires the count to stay
above `os_max` for a full hour, so roughly two hours of genuine thrashing
elapse before anything is reported, and any interruption restarts the timer.

## Possible Diagnoses

1. Deadband between modes too narrow — the sequence flips back as soon as it
   has finished acting, because the condition that ended the last mode is the
   condition that starts the next one
2. Fluctuating zone demands near a changeover threshold — the aggregated demand
   signal sits on the boundary and the unit follows every wobble in it
3. Sensor noise causing mode oscillation — one intermittent or poorly located
   sensor crosses the threshold repeatedly and the sequencing logic faithfully
   obeys

## Energy Impact

COMFORT_ENERGY, LOW confidence, QUALITATIVE_ONLY. The rule sees a state index
and nothing else, so it cannot say what any transition cost. The reference puts
the loss at 1–3% of AHU energy, split between actuator wear and the transitions
themselves, where a coil is charged and then abandoned before the air stream
has settled. Confidence is LOW because no controlled study isolates cycling
losses from the deadband change that fixes them and no PNNL measure covers
sequencing stability. Climate-neutral. Size the opportunity per Energy Impact
Reference §4.4 (unstable hours × AHU coil and fan power); this rule contributes
the hours.

## Emissions Impact

Scope 1 + 2, QUALITATIVE_EMISSIONS, LOW confidence; on the order of
5–15 kg CO₂e/yr from cycling losses. Both scopes appear because the transitions
being counted cross between them — gas at the boiler and electricity at the
chiller for the same hour of indecision. The magnitude is small enough that the
number is an order of magnitude, not an estimate. Avoided-emissions basis: N/A.

## Deviations

- **The reference card names no points; `operating_state` is our choice.** Its
  chapter 9 card has no Required Points table, only the logic and tunables.
  Only transitions are consumed and no value is ever interpreted, so any stable
  enumeration binds; the dictionary recommends the G36 §5.16.14 OS#1–OS#5 index
  and requires only that the encoding not change under the rule's feet.
- **Rolling count built from a moving average, because the block set has no
  windowed counter.** `Integers.OnCounter` counts monotonically from a reset
  and has no window, so it would need a host-driven hourly reset — turning the
  rolling count into a tumbling one whose verdict depends on where the hour
  boundary fell.
- **`count_scale` is coupled to the host's tick interval.**
  `k = count_window / dt`; the default 12.0 is correct only at a 300 s tick. A
  host ticking every 60 s must set 60.0, and leaving 12.0 reports a fifth of
  the true count so the rule never fires. Same family of deployment constraint
  as AHU-0022's minimum sample interval, but this one fails quietly: a
  mis-set scale produces a plausible-looking number.
- **Minimum tick interval, from the same block.** Each `MovingAverage` keeps a
  fixed 64-checkpoint ring and silently drops the oldest in-window sample past
  that. A window spanning n ticks retains n + 1 checkpoints, so it may span at
  most 63 ticks: `dt ≥ count_window/63` = **57.15 s**, and a legal deployment
  has `count_scale = 3600/dt ≤ 63`. At the default 300 s tick the window holds
  12 samples.
- **Startup artifact (a): the first-tick pulse is inert.** `Integers.Change`
  compares against `pre_u_start` on the first tick, so a unit loading in OS#3
  registers a change at t = 0. It encloses no area because `dt` is zero there,
  so it never reaches the count. `chg.pre_u_start` is written explicitly as 0
  rather than left to the engine default (also 0) and is not exposed as a card
  parameter, since nothing it can do survives tick 0.
- **Startup artifact (b): the first hour reads as a rate, not a count.** While
  `t < count_window` the moving average divides by elapsed time, so two changes
  in the first ten minutes read as 12/hr — the pace, extrapolated. Defensible,
  but not the reference's completed-hour count. `delayOnInit = true` blocks any
  assertion before 3600 s and the frontmatter precondition requires the host to
  report NO_EVAL over the same hour, so the artifact cannot reach a verdict.
- **The counting window is half-open.** `rate` compares the accumulated
  integral now against its value one `count_window` ago, so a transition
  exactly `count_window` old has just left the window. The reference is silent;
  it matters only right at the threshold, and it errs toward silence.
- **`alarm_delay` equals `count_window`.** Both come from the reference (60 min
  AlarmDelay, per-hour count), but the interaction is worth stating: the count
  must hold above `os_max` for a full hour after first crossing it, so a burst
  that ends inside that hour never reports.
- **`related` adds AHU-0022.** The reference lists AHU-0016 only;
  AHU-0022 (SAT hunting) already names this fault from its side, so the link
  is made reciprocal.
- Severity 3 (warning) and method `rule`, per the reference's chapter 9 card;
  its §5.8.1 index carries no severity column. Operating states OS 1–5 are
  declared, not gated — the reference marks the fault applicable in every state.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a count already above `os_max` at load waits out the full
  hour instead of alarming on the first tick after a controller restart.

## Notes

The fix is a deadband, and it is remote and free. Step 2.1 of the
[simultaneous-hc](../../../playbooks/simultaneous-hc.md) playbook gives the
numbers for the heating/cooling case — G36 §5.16's 2.8 °C (5 °F) minimum — and
the same reasoning applies to whatever pair of states this unit oscillates
between: the condition that leaves a mode must not be the condition that
re-enters it. Confirm first that the demand signal is not itself the problem
(diagnosis 3). This rule and AHU-0022 are the same pathology at two altitudes,
and both tripping together is the strongest evidence for that diagnosis.
