---
schema: cxf-library/fault-card/v1
id: AHU-0021
name: Excess outdoor air during occupied hours
equipment: ahu
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0021"
  - "PNNL-27338 §3"
  - "PNNL EEM-17 (demand control ventilation)"
g36: null
clusters: []
suppresses: []
suppressed_by: [AHU-0028]
related: [AHU-0006, AHU-0017, AHU-0030]
playbooks: [economizer-failure]
operating_states: "occupied, non-economizer operation (host-gated); reference OS 1, OS 4"
preconditions: "Supply fan running. The host must not evaluate during economizer operation — bringing in more than the design minimum is the point of economizing, and this rule cannot tell that apart from a stuck damper. MAT must pass its integrity gate (AHU-0028, see suppressed_by): the fraction is a ratio of temperature differences, so a biased mixed-air reading moves it directly. The temperature-difference gate is signalled in-rule by yTempDeltaOk; when it is false the verdict is NO_EVAL, not healthy. Additionally suspend evaluation (NO_EVAL) while demand-controlled ventilation or a ventilation-demand override holds outdoor-air flow above the minimum-OA state — at VAV turndown a constant ventilation flow becomes a large OA fraction and this rule fires on healthy operation (fleet-validated FP mechanism; see the validation block)."
points:
  - mat
  - rat
  - oat
outputs:
  - name: yFault
    description: True while the outdoor air fraction has stayed more than oaf_threshold above desired_oaf for at least alarm_delay, with the temperature difference large enough to evaluate
  - name: yTempDeltaOk
    description: Evaluability signal — true when |oat − rat| exceeds oaf_temp_threshold; false means NO_EVAL and the host must ignore yFault
params:
  desired_oaf:
    default: 0.15
    unit: "1"
    description: Design outdoor air fraction the unit should hold at minimum ventilation (0–1)
    cxf: designConst.k
  oaf_threshold:
    default: 0.10
    unit: "1"
    description: Tolerance above the design fraction before the excess counts as a fault
    cxf: marginHigh.t
  oaf_temp_threshold:
    default: 6.0
    unit: "°C"
    description: Minimum |oat − rat| for the fraction to be meaningful; below it the rule is not evaluable
    cxf: deltaOk.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU ventilation thermal energy
  savings_range: 2-10% of AHU thermal energy (PNNL-27338)
  climate_sensitivity: heating-dominant
  runtime_estimation: "excess_oa_kw = (actual_oaf − desired_oaf) × airflow × cp × |oat − rat|"
emissions:
  scope: "1+2"
  method: DIRECT_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 47
    failures: 31
    notes: "same DCV excess-OA events as AHU-0006 (see its note); winter-dominant"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:a9c58cb4c0e46fe131faed48a0c8efc0"
  date: 2026-10-05
---

## Description

The unit is pulling in more outdoor air than its design minimum ventilation
requires, and it is not economizing — every extra cubic metre has to be heated
or cooled to supply temperature for no ventilation benefit. Unlike a failed
economizer, this fault is invisible from the zone: the space stays comfortable,
the coils simply work harder to keep it that way, through every occupied hour.
The outdoor air fraction is inferred from the mixing-box energy balance rather
than measured, which makes the diagnostic cheap — three temperatures, no airflow
station — and conditional, since the inference only holds when outdoor and
return air differ enough to locate the fraction; hence the explicit evaluability
output. AHU-0030 is the same measurement narrowed to heating operation, where
the excess is most expensive. Present in roughly 15% of buildings.

## Detection Logic

```
oaf          = (mat − rat) / (oat − rat)
yTempDeltaOk = |oat − rat| > oaf_temp_threshold     (false ⇒ host reports NO_EVAL)
yFault       = (oaf − desired_oaf > oaf_threshold) AND yTempDeltaOk,
               sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0021 block graph](diagram.svg)

`matRat` and `oatRat` form the two differences, `oaf` divides them, and
`margin` subtracts the design fraction so that `marginHigh` tests the excess
against a single positive threshold. `oatRat` fans out a second time into
`absDelta` and `deltaOk`, whose output is both the boundary output
`yTempDeltaOk` and the second input of `gate` — so `yFault` is held down over
exactly the interval the host is told to disregard it. That matters because the
division is unguarded: CDL `Divide` follows IEEE-754, so `oat = rat` yields ±∞
or NaN rather than an error, and a near-zero denominator amplifies ordinary
sensor noise into a fraction of any magnitude. NaN compares false everywhere,
but ±∞ and a noise-inflated finite fraction can both raise `marginHigh`, and
`gate` is what stops them. Both comparisons are strict: a fraction sitting
exactly at `desired_oaf + oaf_threshold` is not a fault, and a temperature
difference of exactly `oaf_temp_threshold` is not evaluable. The fraction is
signed consistently across the year — summer both differences positive, winter
both negative — so no seasonal branch is needed. `persist` requires 30
continuous minutes, riding out damper strokes and the mixing transient after a
mode change; `delayOnInit = true` holds that window across a restart.

## Possible Diagnoses

1. OA damper minimum position set too high
2. OA damper not closing to minimum — stuck, or the sequence never commands it
   back down after a purge or economizer period
3. Damper actuator issue: failed actuator, slipped linkage, or a position
   feedback that disagrees with the blade
4. Exhaust fan creating negative building pressure that pulls outdoor air in
   past the minimum position

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, DIRECT_MEASUREMENT. The waste is
computable from live data: `excess_oa_kw = (actual_oaf − desired_oaf) ×
airflow × cp × |oat − rat|`, with the excess fraction already on the wire as
`oaf − designConst.k`. Correcting minimum ventilation saves 2–10% of AHU
thermal energy (PNNL-27338), the upper half of that range in heating-dominant
climates. PNNL EEM-17 (demand control ventilation) is the related retrofit and
this rule is its screening test: a unit already over its design fraction with
the dampers at minimum will not benefit from CO₂ control until the mechanical
problem is fixed.

## Emissions Impact

Scope 1 + 2, DIRECT_EMISSIONS, HIGH confidence; typical 500–4,000 kg CO₂e/yr
for the excess ventilation thermal load. The split follows the season: excess
outdoor air in winter usually burns scope 1 fuel at the heating coil, in
summer it draws scope 2 electricity at the chiller. Avoided-emissions basis:
marginal operating emissions rate (MOER).

## Deviations

- The reference's `AND NOT econ_favorable` term is not in the block graph.
  Economizer operation is an operating state, not a measurement, and this
  library keeps operating-state gating host-side (precedent: AHU-0017's OS-4
  restriction), so the term lives in `operating_states` and `preconditions`
  instead. A host that evaluates this rule during economizing will get a fault,
  and it will be the host's bug.
- The reference writes the test as `oaf > (desired_oaf + oaf_threshold)`, which
  would force the two tunables into one summed threshold. Feeding `desired_oaf`
  as `Reals.Sources.Constant.k` and comparing the remaining margin against
  `oaf_threshold` is algebraically identical and keeps both retunable alone.
- Evaluability is an output, not just a precondition: the `|oat − rat|` test is
  computable from this rule's own inputs, so SCHEMA.md requires exposing it as
  `yTempDeltaOk` (PNNL-27338 uses 5 °F for the same computation; the
  reference's 6 °C default is adopted). A false `yFault` under a false
  `yTempDeltaOk` means "unknown", not "healthy".
- Both comparisons are strict (`>`); the reference does not specify boundary
  behavior, so the library's strict convention applies.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the
  library's standing choice: an excess already present at load waits out the
  full 30 minutes instead of alarming on the first tick after a controller
  restart.

## Notes

Check the minimum position setpoint before sending anyone to the roof — the
most common cause is a minimum dialled up during a ventilation complaint or a
commissioning shortcut, and it is a $0 desk fix. The
[economizer-failure](../../../playbooks/economizer-failure.md) playbook's damper
and linkage steps come after that.

The rule is deliberately blind to why the fraction is high. A damper stuck at
40% and a building held under negative pressure by an oversized exhaust fan
produce the same number, and the second is invisible from the AHU's own points:
if commanding the damper closed does not move the fraction, measure building
pressure before replacing the actuator.
