---
schema: cxf-library/fault-card/v1
id: AHU-0028
name: Mixing box damper fault — MAT outside the OAT/RAT envelope
equipment: ahu
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0028"
  - "Schein et al. 2006 (APAR Rule 1)"
  - "Bushby et al. 2001 (NIST/CEC PIER Project 2.3, APAR rules 26/27 — earliest form of this envelope test)"
  - "Torabi 2022"
  - "Gunay 2023"
g36: null
clusters: [CLU-09]
suppresses: [AHU-0002, AHU-0003, AHU-0005, AHU-0006, AHU-0008, AHU-0010, AHU-0012, AHU-0014, AHU-0015, AHU-0021, AHU-0030, RTU-0005, RTU-0006]
suppressed_by: []
related: [AHU-0002, AHU-0003]
playbooks: [sensor-drift]
operating_states: "all (supply fan running)"
preconditions: "Supply fan running and the dampers commanded to a mixing state. The host must not evaluate during coil freeze-protection or within 2 min of an economizer mode transition, when MAT lags the mixture it is supposed to report. When the fan is off or damper state is unknown, the verdict is NO_EVAL, not healthy."
points:
  - oat
  - rat
  - mat
outputs:
  - name: yFault
    description: True while MAT has stayed outside the min(oat, rat)–max(oat, rat) envelope by more than sensor_tolerance for at least alarm_delay
params:
  sensor_tolerance:
    default: 2.0
    unit: "°C"
    description: Combined sensor accuracy allowance applied to both envelope bounds; MAT may sit this far outside min/max before it counts as a fault
    cxf: [belowMin.t, aboveMax.t]
  alarm_delay:
    default: 900.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (15 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU mixing box — sensor integrity gate
  savings_range: sensor-dependent; primary impact is downstream rule accuracy
  climate_sensitivity: neutral
  runtime_estimation: "none — no direct waste term; the cost of a bad MAT is the economizer and mixing errors it induces, accounted for by the rules this one gates"
emissions:
  scope: "1|2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_tpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "TPR: +/-3 degC OAT bias injected into replay inputs (faulted-sensor-as-seen-by-FDD), B2B OfficeMedium-4004 July week, 3 VAV loops; failures = missed detections; baseline-confounded rules excluded from attribution"
    scenarios: 6
    failures: 1
    notes: "both directions: 3/3 at +3 degC (~4.5-28 h), 2/3 at -3 degC — empirical confirmation of the CLU-09 biased-OAT cascade"
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 48
    failures: 0
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:e2e36af9468f68ae62924385965827fb"
  date: 2026-10-05
---

## Description

Mixed air is a blend of outdoor and return air, so its temperature must lie
between the two source temperatures. When MAT falls outside the OAT–RAT
envelope by more than combined sensor accuracy, the physics has been violated
and one of three things is wrong: a temperature sensor is out of calibration,
the MAT sensor is reading a stratified or sun-struck slice of the plenum rather
than the mixture, or the mixing box itself is short-circuiting. This is APAR
Rule 1 (Schein et al. 2006), the sanity check every mixed-air diagnostic is
built on — the trigger of cluster CLU-09 (Sensor Integrity Failure) and the
library's first use of the suppression mechanism, since the MAT-consuming rules
are running on data known to be wrong while it is active. Roughly 15% of
buildings have at least one AHU with a mixed-air sensor this far out.

## Detection Logic

```
T_min  = min(oat, rat) − sensor_tolerance
T_max  = max(oat, rat) + sensor_tolerance
yFault = (mat < T_min OR mat > T_max), sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0028 block graph](diagram.svg)

The graph tests the same condition in gap form: `lowGap` computes
`min(oat, rat) − mat` and `highGap` computes `mat − max(oat, rat)`, and each
gap is compared against `sensor_tolerance` by a strict `GreaterThreshold` — a
gap of exactly 2.0 °C is not a fault, 2.1 °C is. `outOfRange` ORs the two sides
and `persist` requires the excursion to hold for the full `alarm_delay`, so a
MAT that swings out and back during a damper stroke never alarms. Recovery is
immediate; `delayOnInit = true` holds the window across a controller restart.
Which bound is which flips with the season — in cooling weather OAT is the upper
bound and RAT the lower — so `Min` and `Max` are used rather than a fixed
OAT-below-RAT assumption. During 100% outdoor-air or 100% return-air operation
the mixture legitimately pins to one bound, which is why `sensor_tolerance` is a
combined sensor-accuracy allowance and not a mixing-quality threshold.

## Possible Diagnoses

1. MAT sensor out of calibration or failed
2. OAT sensor out of calibration
3. RAT sensor out of calibration
4. MAT sensor in a poor location — plenum stratification or solar gain on the
   sensor rather than the mixed stream
5. Mixing box short-circuiting: OA bypassing the mix and striking the sensor
   directly

## Energy Impact

COMFORT_ENERGY, LOW confidence, QUALITATIVE_ONLY. A mis-read MAT costs nothing
by itself; its cost is indirect and can be large, since a biased mixed-air
reading drives the economizer to the wrong damper position and can hold a coil
open against outdoor air that would have done the job for free. PNNL EEM-01
(sensor recalibration) puts the recoverable range at 0–5% of site energy across
an entire sensor population, sensor-dependent and climate-neutral. This rule's
value is the accuracy it restores to the rules it gates, not savings of its own.

## Emissions Impact

QUALITATIVE_EMISSIONS, LOW confidence; no direct emissions — a sensor reading
neither burns fuel nor draws power. Scope is recorded as `1|2` because it
follows the subsystem the bad reading distorts: unnecessary preheat lands in
scope 1, disabled economizing or extra mechanical cooling in scope 2, and the
same drifted sensor can do both in different seasons. Avoided-emissions basis:
N/A.

## Deviations

- The reference's bound comparison is rewritten as a gap comparison. Compared
  directly against `T_min`/`T_max`, one card parameter would need two signed
  values, so a host retuning `sensor_tolerance` would have to negate one.
  Subtracting and comparing the gap against a positive threshold is
  algebraically identical (`min − mat > tol ⟺ mat < min − tol`) and leaves one
  number to retune, with both paths listed under `params.sensor_tolerance.cxf`.
- `GreaterThreshold` is `u > t`, so a gap of exactly `sensor_tolerance` reads
  healthy, matching the reference's strict `mat < T_min` / `mat > T_max`.
- Suppression is declared, not encoded. A status-blind engine of independent
  composites cannot silence sibling rules, so the relationship lives in
  `suppresses` and CLU-09 for the host to enforce. The graph is
  equipment-agnostic: the two RTU entries in `suppresses` are gated by this
  rule instantiated on the RTU's own mat/oat/rat points.
- Freeze-protect operation and the two-minute window after an economizer mode
  change — periods when MAT legitimately disagrees with the steady-state
  mixture — are host-enforced preconditions rather than in-rule gates.
- `delayOnInit = true` on `persist`: a controller restart mid-excursion still
  waits out the full 15 min before alarming (startup conservatism per
  AHU-0016).

## Notes

The suppression contract matters more than the fault itself. While `yFault` is
true the host must silence all thirteen rules in `suppresses` *and* report every
MAT-derived verdict as NO_EVAL rather than healthy — silence is not a clean bill
of health. Both halves are necessary: a rule that fires on garbage sensor data
erodes operator trust, and a rule that reports "no fault" from the same garbage
hides real problems. Hence Step 1.3 of the
[sensor-drift](../../../playbooks/sensor-drift.md) playbook — when several
faults fire on one AHU at once, suspect a shared sensor first. The fix is
on-site: recalibrate against a NIST-traceable reference, or replace ($30–$80).
