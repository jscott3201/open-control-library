---
schema: cxf-library/fault-card/v1
id: HW-0010
name: Hot-water supply temperature tracking failure
equipment: hw
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "NIST, Automatically Detecting Faulty Regulation in HVAC Controls (2013), pp. 412 and 416-419 — regulated-variable allowance bands, transient exclusions, and field-tuned alarm parameters"
  - "LBNL Simulated Boiler Plant dataset inventory, PDF pp.4-8 — hot-water loop supply temperature/setpoint/status channels and separate sensor-bias, fouling, and poor-PI fault cases; the dataset is a future replay target, not a completed validation claim"
  - "Library precedent CHW-0007 — verified strict mirrored hydronic tracking-error topology and continuous 900 s persistence"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0002, HW-0004, HW-0007, HW-0008, HW-0009, HW-0011, FPB-0006]
playbooks: [hot-water-plant-faults]
operating_states: "normal automatic hot-water operation after startup, with at least one boiler proven firing and distribution circulation established"
preconditions: "hws_temp and hws_temp_sp must describe the same controlled outlet or common header and final active target. A common header plus an OR of firing statuses is valid only when all points belong to the same configured plant; it is not an individual-boiler outlet comparison. Exclude warm-up and setback recovery, setpoint/reset ramps, stage or pump changes, minimum-flow transitions, tuning tests, and intentional demand, high-limit, fuel, flame-safeguard, freeze, emissions, or other capacity limits until the plant has settled. boiler_status must represent firing rather than enable, and hw_pump_status must establish distribution circulation. Temperature, status, and setpoint signals must be fresh, aligned, calibrated, and in the declared units. When any obligation is unmet the verdict is NO_EVAL, not healthy."
points:
  - hws_temp
  - hws_temp_sp
  - boiler_status
  - hw_pump_status
outputs:
  - name: yFault
    description: True after the active plant remains more than tracking_error above or below its final HWS target continuously for sustained_duration
  - name: yTooCold
    description: Immediate direction flag; true while the active plant is more than tracking_error below setpoint
  - name: yTooHot
    description: Immediate direction flag; true while the active plant is more than tracking_error above setpoint
params:
  tracking_error:
    default: 2.0
    unit: K
    description: "ADOPTED_TUNABLE symmetric settled tracking allowance. Commission above combined measurement error, setpoint resolution, and the controller's normal deadband; exact equality is clear."
    cxf: [tooHot.t, tooCold.t]
  sustained_duration:
    default: 900.0
    unit: s
    description: "LIBRARY_PRECEDENT from CHW-0007's identical hydronic tracking form. Confirm it exceeds ordinary plant response after every excluded transition."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Boiler plant, hot-water distribution, and served heating loads
  savings_range: "Site-dependent; a cold direction can shift load or miss comfort, while a hot direction can increase distribution loss and reduce condensing efficiency"
  climate_sensitivity: heating-dominant
  runtime_estimation: "QUALITATIVE_ONLY. Error-hours do not determine excess fuel without flow, delivered load, temperature lift, plant efficiency, and the active limiting state."
emissions:
  scope: "1"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, January 6-12, one target-loop hot-water boiler and pump, plant mode at 60 s"
    scenarios: 1
    failures: 0
    notes: "single RunPeriod and 10,080 chronology-validated samples; exact HeatSys1 boiler/pump membership was traversed from PlantLoop supply branches, positive boiler PLR was the disclosed firing proxy, positive same-loop pump mass flow was the circulation proxy, and common loop outlet temperature was compared with its own node setpoint. Ten setpoint-stable windows began after an 1800 s active/settling lead, totaling 3,897 evaluated ticks (64.95 h), with zero false positives. This ideal LeavingSetpointModulated model is healthy FPR evidence, not sensor-noise robustness or TPR."
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:d680c1142ee5617cde72f0dbcc028046"
  date: 2026-10-05
---

## Description

This rule reports a firing, circulating hot-water plant that cannot hold the
same target its supply-temperature point is meant to control. The direction is
diagnostic, not causal: cold water may reflect capacity, flow, fouling, staging,
or an intentional limit the host failed to exclude; hot water may reflect
overshoot, aggressive staging, a bad target, sensor bias, or the wrong header.

## Detection Logic

```text
error       = hws_temp - hws_temp_sp
too_hot     = error > tracking_error
too_cold    = -error > tracking_error
plant_active = boiler_status AND hw_pump_status

yTooHot  = plant_active AND too_hot
yTooCold = plant_active AND too_cold
yFault   = TrueDelay(yTooHot OR yTooCold, sustained_duration)
```

Block graph (`rule.cxf.jsonld`):

![HW-0010 block graph](diagram.svg)

Both comparisons are strict, so exactly +/-2 K is clear. The single delay is
after their OR: a directly sampled hot-to-cold jump without an in-band tick
preserves persistence, while an in-band, boiler-off, or pump-off tick resets it.
`delayOnInit=true` requires the complete interval after evaluator startup.

## Possible Diagnoses

1. Boiler capacity, fuel input, heat exchanger, or minimum-flow limitation.
2. Poor temperature-loop tuning, excessive integral action, or plant delay.
3. Stage command, firing proof, distribution pump, or control-valve problem.
4. Active setpoint not reaching the local boiler or mixing controller.
5. Temperature sensor bias, poor placement, stale delivery, or wrong header.
6. A real demand, reset, safety, emissions, or high-limit condition omitted
   from the host gate.

## Energy Impact

The finding is qualitative. Sustained over-temperature can increase pipe loss
and keep a condensing plant above its efficient return-temperature region;
under-temperature can increase terminal/pump effort or shift load to other heat.
The graph has no fuel or delivered-load model, so it does not invent savings.

## Emissions Impact

Any scope-1 effect follows the change in boiler fuel use and cannot be inferred
from error direction alone. Quantification requires measured fuel or a validated
load-and-efficiency model; this rule reports no generic emissions reduction.

## Deviations

- This is a library-authored application of NIST regulation concepts and the
  CHW-0007 graph, not a source-transcribed boiler rule.
- The shipped 2 K allowance is adopted and tunable; neither NIST nor LBNL
  publishes it as a portable boiler threshold.
- The LBNL dataset is cited for point/fault coverage only. No LBNL replay was
  available locally for this slice, so no LBNL-derived TPR or FPR claim is
  recorded; the frontmatter separately records limited healthy EnergyPlus FPR
  evidence.
- Confidence is MEDIUM rather than the brief's proposed HIGH because common-
  header topology, sensor bias, and intentional plant limits remain material
  confounders even with the stated host gates.
- Direction flags include the active-plant gate. When the plant stops, they and
  `yFault` clear immediately; the host must report NO_EVAL outside the stated
  operating state.

## Notes

Read HW-0009 when command and proof disagree, HW-0008 when the target itself is
not resetting, and HW-0002/HW-0004 when temperature tracking coexists with
efficiency or low-delta-T evidence. These rules may co-occur and do not suppress
one another.
