---
schema: cxf-library/fault-card/v1
id: HW-0006
name: HW loop DP reset not functioning
equipment: hw
status: verified
phase: 2
method: statistical
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "PNNL-27338 §4.3.2 (no DP reset; daily MAX−MIN of the loop DP setpoint against a 2.5 psi range), pp. 4.10-4.11"
  - "PNNL-27338 §4.2.2 (high loop DP), pp. 4.7-4.8 — establishes pump VFD speed as the HW loop's load proxy"
  - "PNNL-27338 (Katipamula et al. 2018) — adapted via an internal paraphrased deep-read digest, not distributed (rule candidate 5)"
  - "Sibling-rule precedent: CHW-0003 (window, alarm delay, half-range tolerance), CHW-0002 (sampler+dwell activity conjunct and its NO_EVAL output), AHU-0024"
  - "Library extension: HVAC FDD Reference v1.0 ch.14 specifies HW-0001..052 only — index framing in faults/hw/README.md"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0008, CHW-0003, AHU-0024]
playbooks: [missing-reset, hot-water-plant-faults]
operating_states: "Heating season, HW distribution running on variable-speed pumps for the bulk of the evaluation window"
preconditions: "hw_dp_sp must be the setpoint the pumps actually control to — the active value in the pump controller, not a design figure in a schedule table — and hw_pump_vfd_speed must be the speed of a pump on that same loop. On a lead/lag set bind the lead pump: a lag pump's speed is pinned by staging logic rather than by the building, and it reads flat while the loop swings. On primary/secondary plants bind the secondary (distribution) pumps; a constant-speed primary has no reset to fail. A loop whose pumps have no drive at all must not be bound: there is no DP reset to detect and the speed input has nothing to say. The plant must be running for most of the window — a loop shut down for the summer holds both signals still, and only yPumpSpeedVaried stands between that and a false alarm; evaluability is signalled in-rule by that output, and when it is false the verdict is NO_EVAL, not healthy. Finally, confirm the setpoint point is actually written: a BAS that trends the reset output only while the reset is enabled shows a flat last-known value that no pump is following."
points:
  - hw_dp_sp
  - hw_pump_vfd_speed
outputs:
  - name: yFault
    description: True while the HW loop differential-pressure setpoint has stayed flat over the evaluation window despite sufficient pump-speed variation, for at least alarm_delay
  - name: yPumpSpeedVaried
    description: Evaluability signal — true when HW pump speed has varied enough within the evaluation window for a flat setpoint to mean anything; false means NO_EVAL and the host must ignore yFault
params:
  evaluation_window:
    default: 259200.0
    unit: s
    description: Window over which setpoint flatness and pump-speed variation are assessed (3 days); drives both baseline sample periods and both dwell timers
    cxf: [spRef.samplePeriod, pumpRef.samplePeriod, spFlatHeld.delayTime, pumpFlatHeld.delayTime]
  sp_flat_tolerance:
    default: 8.625
    unit: kPa
    description: Max deviation of hw_dp_sp from its sampled baseline to count as flat — half of PNNL-27338 §4.3.2's 2.5 psi (17.24 kPa) minimum expected setpoint range
    cxf: spFlat.t
  pump_variation_tolerance:
    default: 10.0
    unit: "%"
    description: "Max deviation of hw_pump_vfd_speed from its sampled baseline to still count as flat (half an adopted 20-point minimum speed range; PNNL-27338 states no activity gate for this check — see Deviations)"
    cxf: pumpFlat.t
  alarm_delay:
    default: 86400.0
    unit: s
    description: Fault persistence before alarm (24 h)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: HW distribution pump energy (cubic pump law)
  savings_range: "1-3% site energy for HW supply-temperature reset and DP reset together (playbooks/hot-water-plant-faults.md, per PNNL-27338); no split between the two measures is published"
  climate_sensitivity: heating-dominant
  runtime_estimation: "pump_waste_kw = hw_pump_kw × [1 − (1 − DP_reduction/100)³] — CHW-0003's formula applied to the heating loop, since the cubic pump law does not care which fluid is in the pipe. Both inputs come from the host: measured pump input power, and the setpoint reduction a commissioned reset would have achieved on this loop. The graph supplies the trigger only"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:754dbe3f355e9ba37c4c95e68c10df7b"
  date: 2026-10-05
---

## Description

The hot water loop holds one differential-pressure setpoint all winter while the
pumps modulate underneath it. Whatever the design-day pressure was is what the
loop gets in November and in March, and the coil valves throttle away the
surplus. Pump power follows the cube of pressure, so a fifth off the setpoint is
roughly half the pump energy, and what buys it is a sequence, not a pump.

This rule is a **library extension** — the reference's ch.14 specifies three hot
water rules and no reset checks. The detection is grounded in PNNL-27338 §4.3.2,
which flags a loop whose DP setpoint moves less than 2.5 psi across a day; the
window, the activity conjunct and every block in the graph are this library's.
The pump-speed conjunct is what makes a flat setpoint mean something: a loop
that is off, or whose pumps sit at a fixed speed, holds its setpoint flat for
reasons that have nothing to do with a missing reset.

## Detection Logic

```
baseline(x)      = x sampled and held every evaluation_window (3 days)
sp_flat          = |hw_dp_sp − baseline(hw_dp_sp)| < sp_flat_tolerance,
                   continuously for evaluation_window
pump_flat        = |hw_pump_vfd_speed − baseline(hw_pump_vfd_speed)|
                   < pump_variation_tolerance,
                   continuously for evaluation_window

yPumpSpeedVaried = NOT pump_flat     (false ⇒ host reports NO_EVAL)
yFault           = sp_flat AND yPumpSpeedVaried, sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HW-0006 block graph](diagram.svg)

This is CHW-0002's detector with the heating loop's points bound to it. Two
symmetric chains compare each signal against a `Discrete.Sampler` hold refreshed
once per window; the sampler emits the live input on its first tick, so there is
no startup artifact. `spFlatHeld` asserts only after the setpoint has stayed
within tolerance continuously for a full window, and any reset activity of
8.625 kPa or more restarts it. `pumpFlatHeld` does the same for speed, and its
negation is `yPumpSpeedVaried` — "not varied" means a full window of continuous
flatness, so the signal is optimistically true during the first window after
startup, which is harmless because `yFault` needs that same window.

Both dwell timers fire on the same tick when the loop is flat in both signals,
so the fault conjunction is false by construction on that tick and there is no
boundary race. `persist` (24 h) filters the remainder, and every `TrueDelay`
carries `delayOnInit = true`. Worst-case time to alarm from cold start is
`evaluation_window + alarm_delay` — 4 days; a loop going flat mid-run alarms
4 days after the setpoint settles. Comparisons are strict, so a signal sitting
exactly on a tolerance falls on the *not-flat* side.

## Possible Diagnoses

The reference algorithm publishes a detection test and no diagnosis list; these
are this library's, in the order a technician should work them.

1. Loop DP reset never programmed — the common case, and a desk fix
2. Reset programmed but disabled, or the setpoint overridden to a fixed value
   and the override never released
3. Valve-position or zone requests never reach the pump controller, so a
   correctly written reset has nothing to respond to
4. DP sensor mounted at the pump discharge rather than at the hydraulically most
   remote coil — a pipe-fitting job, and it makes a correct reset impossible to
   commission
5. Reset running but writing to a different point than the one trended — a
   monitoring artifact and the cheapest thing on the list to rule out

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, PROXY_ESTIMATION. The savings figure is the
hot water playbook's: HW supply-temperature reset and DP reset together are
worth 1–3% of site energy, with no published split. The mechanism on this half
is the cubic pump law —
`pump_waste_kw = hw_pump_kw × [1 − (1 − DP_reduction/100)³]`, CHW-0003's
formula — and it is heating-dominant because that is where the loop runs for
months. Confidence is HIGH for the detection, not for the dollar figure: a flat
setpoint on a modulating loop is about as direct an inference as this library
makes, while the saving depends on surplus pressure these two points cannot
measure.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, HIGH confidence. The waste is pump electricity, so it
prices at the marginal operating emissions rate and shrinks as the grid does —
unlike HW-0008's fuel-side twin, which a site owns however clean its
electricity gets. Typical range 300–3,000 kg CO₂e/yr by analogy with
CHW-0003's chilled water case; no HW-specific figure is published.

## Deviations

- **This rule extends the reference rather than transcribing it.** Ch.14 covers
  HW-0001, 051 and 052 only, so there is no reference card behind this one —
  no published description, operating states, diagnosis list, tunables line or
  test vectors. The detection is paraphrased from PNNL-27338 §4.3.2 (Katipamula
  et al. 2018); `name`, `severity` and `method` come from `faults/hw/README.md`.
- **Daily MAX−MIN → rolling sampled baseline plus dwell.** PNNL's reset checks
  run once a day at midnight over the prior day's array; this engine has no
  windowed min/max block and no batch clock, a question the library settled at
  AHU-0023/AHU-0024 and again at CHW-0002/CHW-0003. Detection is equivalent for a
  setpoint that moves and returns, and slightly conservative for monotonic drift
  inside one window, where a range test would still call the day flat.
- **2.5 psi becomes an 8.625 kPa half-range.** The point dictionary carries
  `hw_dp_sp` in kPa and rules do no unit conversion, so it happens here once:
  2.5 psi = 17.24 kPa, half-range 8.62, shipped as 8.625 (a 17.25 kPa range).
  The half-range is the library's convention, because a signal swinging ±t about
  a baseline spans 2t. The 0.08% difference from an exact conversion is far
  below any BAS's setpoint resolution, and 8.625 is exactly representable in
  binary, which is what lets the boundary be pinned to the bit.
- **The 3-day window is adopted from CHW-0003, not from PNNL.** PNNL's window
  is one calendar day, which works there because the daily batch discards its
  evidence at every midnight; a rolling dwell does not, and a one-day dwell plus
  a 24 h alarm delay would fire on any quiet weekend. Three days spans a weekend
  plus a working day. `evaluation_window` drives all four timing parameters
  together and hosts must set them as a group.
- **Pump-speed range is the activity conjunct, and it is not CHW-0003's valve
  test.** The hot water dictionary has no valve aggregate to bind, so the honest
  analog is the load proxy PNNL's own HW algorithms use (§4.2.2 and §4.4.2 both
  test `avg_pump_vfd`). What it buys is weaker: pump-speed range answers "is
  this loop alive and modulating", not "is any coil starving". A loop pinned at
  its DP limit by a starving coil usually runs its pumps flat out, which
  collapses the speed range and correctly yields NO_EVAL — but a loop that still
  modulates below full speed against a maxed-out setpoint reads as a missing
  reset here. HW-0005 (high loop DP) separates those two on the same loop.
- **The 20-point minimum speed range is adopted, and argued rather than cited.**
  §4.3.2 has no activity gate at all, so there is no figure to transcribe;
  `pump_variation_tolerance = 10 %` (half-range) follows CHW-0002's adopted
  `min_load_range` in shape and intent — a low bar a live loop clears easily,
  not a discriminating threshold. A variable-speed loop that never swings 20
  points across three days is riding its minimum-speed floor, running against a
  fixed bypass, or reporting a dead feedback, and NO_EVAL is right in all three.
  Lowering it makes the rule fire more often, not less.
- **NO_EVAL is surfaced as `yPumpSpeedVaried`, where CHW-0003 ships no
  evaluability output at all.** That rule's valve test is a conjunct of the
  fault condition — a starving coil makes the flat setpoint legitimate, not
  unmeasurable. The pump-speed test here is an evaluability question, which is
  CHW-0002's `yLoadVaried` semantics exactly, and it earns its place as an
  output rather than an echo by being a stateful window test over the signal's
  own baseline. Boolean logic has no tri-state, so the host must read it first:
  false means NO_EVAL, never healthy.
- **`Reals.MovingAverage` rejected, and the tick band that follows.** The engine
  implements it with a fixed 64-checkpoint ring, so a three-day window would
  need `dt ≥ 4,114 s` before it stops silently dropping its oldest samples. No
  BAS ticks that slowly; AHU-0023 found this and every reset rule since has
  inherited it. The sampler-and-dwell chain has no lower bound on tick period,
  and its upper bound is the one to watch — a reset excursion shorter than one
  tick is invisible to the flatness test, so trend at 5–15 min.
- **Strict comparisons on both tolerances.** `Reals.LessThreshold` is `u < t`,
  so a setpoint deviating exactly 8.625 kPa clears the flatness dwell and a pump
  speed deviating exactly 10 points counts as varied. Equality is measure-zero
  in continuous data and perfectly reachable in a BAS that scales setpoints to
  fixed increments, so both boundaries are pinned from both sides.
- **`alarm_delay` = 24 h implemented as `TrueDelay` on the fault conjunction**;
  the evaluation window itself is enforced by the two flatness dwells.
  `delayOnInit = true` on every `TrueDelay` (startup conservatism per
  AHU-0016), so a rule loaded onto an already-faulted loop still waits the
  full window plus delay. 24 h is the value every reset rule in this library
  carries; PNNL's daily batch has no analog, its evaluation being one shot per
  midnight.
- **Two playbooks, where the house habit is one.** `missing-reset` owns the
  reset family's verification step and its Applies-To already reaches past the
  AHU rules to the CHW pair; `hot-water-plant-faults` owns the remedy in its
  step 3 (reset the loop DP from the most-open valve position). Neither covers
  the fault alone and both indexes belong to other writers, so this card lists
  both rather than stretching one.
- **Blind spots.** The rule reads the setpoint, never the pressure: a loop whose
  setpoint resets correctly while the pumps fail to track it is a different
  fault. Diagnoses 1–3 produce one signature and cannot be separated here.
  Diagnosis 4 is worse than invisible — it produces a plausible flat setpoint
  this rule reports as a missing reset, which is why the playbook's first step
  is to find the sensor. A loop cycling on and off around a fixed speed can
  present a varying speed and a legitimately flat setpoint. And a reset written
  backwards is a working reset as far as a range test is concerned.

## Notes

Fix path is the [missing-reset](../../../playbooks/missing-reset.md) playbook
for the verification step and
[hot-water-plant-faults](../../../playbooks/hot-water-plant-faults.md) for the
remedy: plot `hw_dp_sp` against `hw_pump_vfd_speed` over the window, confirm the
DP sensor sits at the hydraulically most remote coil rather than at the pump,
then program the reset from valve position or zone requests.

`clusters` is deliberately empty. CLU-02 ("Missing Reset Strategy") is an
AHU-scoped cluster triggered by AHU-0023, and membership is
`clusters/clusters.json`'s to declare — the same call CHW-0002 and CHW-0003
made one system upstream. A plant failing both HW-0006 and HW-0008 has one
root cause, which is that nobody commissioned the hot water resets, and it
should be dispatched as one visit.
