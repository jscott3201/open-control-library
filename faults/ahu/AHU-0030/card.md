---
schema: cxf-library/fault-card/v1
id: AHU-0030
name: Excess outdoor air during heating mode
equipment: ahu
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0030"
  - "Schein et al. 2006 (APAR)"
  - "PNNL-27338"
  - "Gunay 2023"
g36: null
clusters: []
suppresses: []
suppressed_by: [AHU-0028]
related: [AHU-0028, AHU-0021, AHU-0006]
playbooks: [economizer-failure]
operating_states: "heating (htg_vlv_cmd is tested in-rule; deeper mode gating stays host-side)"
preconditions: "Supply fan running. MAT must pass its integrity gate (AHU-0028, see suppressed_by) — the reference names that rule as this one's prerequisite, because the outdoor air fraction is a ratio of temperature differences and a biased mixed-air reading moves it directly. The temperature-difference gate is signalled in-rule by yTempDeltaOk; when it is false the verdict is NO_EVAL, not healthy."
points:
  - oat
  - rat
  - mat
  - htg_vlv_cmd
outputs:
  - name: yFault
    description: True while the heating coil has been open and the outdoor air fraction more than oa_excess_margin above design, for at least alarm_delay, with the temperature difference large enough to evaluate
  - name: yTempDeltaOk
    description: Evaluability signal — true when |oat − rat| exceeds min_delta; false means NO_EVAL and the host must ignore yFault
params:
  design_min_oa_fraction:
    default: 0.15
    unit: "1"
    description: Design minimum outdoor air fraction the unit should hold while heating (0–1)
    cxf: designConst.k
  oa_excess_margin:
    default: 0.15
    unit: "1"
    description: Tolerance above the design minimum fraction before the excess counts as a fault
    cxf: marginHigh.t
  min_delta:
    default: 6.0
    unit: "°C"
    description: Minimum |oat − rat| for the fraction to be meaningful; below it the rule is not evaluable
    cxf: deltaOk.t
  valve_open_threshold:
    default: 5.0
    unit: "%"
    description: Heating valve command above which the coil counts as heating
    cxf: htgOn.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU heating energy from excess ventilation
  savings_range: 3-15% of heating energy; worst in cold climates
  climate_sensitivity: heating-dominant
  runtime_estimation: "excess_htg_kw = (actual_oaf − design_min_oa_fraction) × airflow × cp × (rat − oat)"
emissions:
  scope: "1+2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:a1a61e83113bf6d634eb7702f954f32f"
  date: 2026-10-05
---

## Description

The heating coil is running and the unit is drawing well over its design minimum
outdoor air. This is AHU-0021's measurement narrowed to the operating state
where excess ventilation costs the most: every extra cubic metre arrives at
outdoor temperature and has to be lifted to supply temperature by the coil the
rule is watching — in a −5 °C hour against 22 °C return air, 27 °C of lift on
that share of the airflow. The classic cause is an economizer that never handed
back: dampers open for free cooling in a mild afternoon, outdoor air turns cold
overnight, and the sequence (or a stuck actuator, or a leaking blade seal)
leaves them there while the heating coil compensates. Nothing about it is
uncomfortable, so it survives until someone reads the fuel bill. Present in
roughly 15% of buildings.

## Detection Logic

```
oaf          = (mat − rat) / (oat − rat)
yTempDeltaOk = |oat − rat| > min_delta                (false ⇒ host reports NO_EVAL)
yFault       = (oaf − design_min_oa_fraction > oa_excess_margin)
               AND (htg_vlv_cmd > valve_open_threshold)
               AND yTempDeltaOk,
               sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0030 block graph](diagram.svg)

The fraction chain is AHU-0021's, unchanged: `matRat` and `oatRat` form the
two differences, `oaf` divides them, and `margin` subtracts the design fraction
so `marginHigh` tests the excess against a single positive threshold. `htgOn`
adds the heating condition and `and1` conjoins it with the excess; `and2` then
gates the whole finding on `deltaOk`, whose output is also the boundary output
`yTempDeltaOk`. That gate is what makes the unguarded division safe: CDL
`Divide` follows IEEE-754, so `oat = rat` yields ±∞ or NaN rather than an error,
and a near-zero denominator turns ordinary sensor noise into a fraction of any
magnitude. NaN compares false everywhere, but ±∞ and a noise-inflated finite
fraction can both raise `marginHigh`, and `and2` stops them. All three
comparisons are strict: a valve parked at exactly 5%, a fraction sitting exactly
at `design_min_oa_fraction + oa_excess_margin`, and a temperature difference of
exactly `min_delta` all read as no-fault. `persist` requires 30 continuous
minutes, riding out damper strokes and the mixing transient after a mode change;
`delayOnInit = true` holds that window across a controller restart.

## Possible Diagnoses

1. OA damper stuck partially open
2. OA damper minimum setpoint configured too high
3. Economizer override not releasing after the transition out of free cooling
4. Leaking OA damper seals

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, DIRECT_MEASUREMENT. The waste is
computable from live data: `excess_htg_kw = (actual_oaf −
design_min_oa_fraction) × airflow × cp × (rat − oat)`, with the excess fraction
already on the wire as `oaf − designConst.k`. Correcting it saves 3–15% of
heating energy (PNNL EEM-06, OA damper faults; PNNL-27338), the top of that
range in cold climates where `(rat − oat)` stays large for months. This is the
defect AHU-0021 finds year-round, priced at its worst hour, which is why the
two share a playbook.

## Emissions Impact

Scope 1 + 2, DIRECT_EMISSIONS, HIGH confidence; typical 300–2,500 kg CO₂e/yr
for the excess ventilation heating load. Most of it is scope 1 fuel at the
boiler or furnace; the scope 2 share is whatever the site's heating comes from
electrically — heat pumps, electric resistance coils, and the extra fan energy
of moving the air. Avoided-emissions basis: marginal operating emissions rate
(MOER).

## Deviations

- `min_delta` is adopted, not transcribed: the reference states the fraction is
  computed only when `|OAT − RAT| > min_delta` but omits the parameter from its
  tunables table. 6.0 °C matches AHU-0021's `oaf_temp_threshold` so the two
  rules agree on when the shared measurement is meaningful (PNNL-27338 uses
  5 °F for the same computation); retune one and retune the other.
- `valve_open_threshold` is likewise absent from the reference's tunables
  table. 5% is what chapter 9 uses everywhere else a valve counts as open
  (AHU-0016, AHU-0025), so "heating" means the same thing chapter-wide.
- The reference writes the test as `oaf > (design_min_oa_fraction +
  oa_excess_margin)`, which would force the two tunables into one summed
  threshold. Feeding the design fraction as `Reals.Sources.Constant.k` and
  comparing the remaining margin against `oa_excess_margin` is algebraically
  identical and keeps both retunable alone.
- Evaluability is an output, not just a precondition: the `|oat − rat|` test is
  computable from this rule's own inputs, so SCHEMA.md requires exposing it as
  `yTempDeltaOk`. A false `yFault` under a false `yTempDeltaOk` means
  "unknown", not "healthy".
- Heating is in-graph because `htg_vlv_cmd` is a measured point; everything
  beyond it — occupancy, unit mode, morning warmup — stays host-side per this
  library's design stance, as in AHU-0017 and AHU-0021.
- All three comparisons are strict (`>`); the reference does not specify
  boundary behavior, so the library's strict convention applies.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the
  library's standing choice: an excess already present at load waits out the
  full 30 minutes instead of alarming on the first tick after a controller
  restart.

## Notes

This rule and AHU-0021 share the fraction core and differ in three ways, all
from their respective reference cards: the `htgOn` term is in-graph here where
FC-055 leaves its scope to the host, the excess margin is 0.15 against FC-055's
0.10, and the energy term uses the signed `(rat − oat)` because in heating the
sign is known. Deploying both is not redundant — this one alarms earlier in
winter with a sharper cost estimate, FC-055 watches the rest of the year.

Verify AHU-0028 is clear before acting: a MAT sensor reading 3 °C low in
−5 °C weather manufactures this fault out of nothing, which is why the
reference names it a prerequisite. If the fraction is genuinely high, command
the OA damper to minimum and watch MAT. It should climb toward return
temperature within minutes; if it does not, the problem is mechanical and the
[economizer-failure](../../../playbooks/economizer-failure.md) playbook's
on-site steps apply. If it does, the sequence never commanded minimum position
and the fix is at a desk.
