---
schema: cxf-library/fault-card/v1
id: AHU-0007
name: SAT too low at full heating
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "HVAC FDD Reference v1.0 §5.8.1 (index; card abbreviated)"
  - "G36 §5.16.14 FC#7 (text per Addendum u public review)"
  - "NISTIR 7365 (defaults provenance)"
g36: "§5.16.14 FC#7"
clusters: []
suppresses: []
suppressed_by: []
related: [AHU-0005, AHU-0013, AHU-0023, AHU-0032, AHU-0033]
playbooks: []
operating_states: "OS#1 (heating) — host-gated"
preconditions: "The unit must have a heating coil; G36 omits FC#7 where there is none. Evaluate only in OS#1, whose actuator signature per G36 Table 5.16.14.2 is heating coil > 0, cooling coil = 0, OA damper at minimum position — in any cooling state SAT is meant to sit below its heating setpoint. Supply fan running, since SAT means nothing in still air. Suspend evaluation for ModeDelay (30 min) after any operating-state change, while the coil is still stroking and SAT still reflects the previous state. sat_sp must be the setpoint the unit is actively controlling to: a host that binds a design constant while the sequence follows a reset schedule is comparing SAT against a number nothing is chasing. When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - sat
  - sat_sp
  - htg_vlv_cmd
outputs:
  - name: yFault
    description: True while SAT has stayed more than sat_error_threshold below setpoint with the heating coil commanded above hc_full_threshold, for at least alarm_delay
params:
  sat_error_threshold:
    default: 1.0
    unit: "°C"
    description: "Shortfall below the supply air temperature setpoint that counts as a real miss rather than sensor error; the G36 §5.16.14 internal variable eSAT at its Table 5.16.14.5 default"
    cxf: spMiss.t
  hc_full_threshold:
    default: 99.0
    unit: "%"
    description: "Heating coil command above which the coil is treated as having no capacity left (G36 `HC >= 99%`). Hosts whose valve command is quantized to whole percent should retune to 98.9, since the comparison is strict"
    cxf: htgFull.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (G36 AlarmDelay, 30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU heating and the terminal heat making up its deficit
  savings_range: "2-5% of AHU energy (HVAC FDD Reference §5.8.1 index row; no PNNL EEM mapped)"
  climate_sensitivity: heating-dominant
  runtime_estimation: "Cause-dependent. Where a cooling source is running against the coil (leaking CC valve, DX stuck on), the waste is AHU-0016's simultaneous heating and cooling term. Where the coil is simply out of capacity or hot water, the AHU wastes nothing directly and the cost moves downstream: makeup_kw = supply_airflow_m3s × 1.2 kg/m³ × 1.005 kJ/kg·K × (sat_sp − sat), delivered by terminal reheat at its efficiency instead of the AHU's"
emissions:
  scope: "1|2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:c388701604d442eb0bf2425055d91402"
  date: 2026-10-05
---

## Description

The heating coil is wide open and the air is still coming out cold. Two signals
settle it: the coil command has reached its stop, so the control loop has
already asked for everything it has, and SAT is below setpoint by more than the
sensor could be wrong about. What is left is a capacity, supply, or sensing
problem — the coil cannot deliver, the heat source is not there to deliver, or
the measurement is lying about what did get delivered. None of it is a tuning
question, which is what makes the saturated-valve half of the test worth
carrying: SAT below setpoint at 60% valve command is an ordinary loop working
through a load change. This is G36 §5.16.14 FC#7, applicable in OS#1 (heating)
only, and G36 omits it entirely on units with no heating coil.

## Detection Logic

```
sp_gap = sat_sp − sat
yFault = (sp_gap       > sat_error_threshold)   SAT short of setpoint by more than sensor error
     AND (htg_vlv_cmd  > hc_full_threshold)     coil has no capacity left
         sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0007 block graph](diagram.svg)

The gap form is G36's `SAT_AVG < SATSP − eSAT` rearranged so the tolerance
stays a positive number on one CXF path (see Deviations). Both comparisons are
strict, so an error sitting exactly on 1.0 °C and a coil reported at exactly
99.0% both read healthy. `persist` requires 30 minutes of continuous violation
and any interruption restarts the timer, which separates a broken coil from a
morning warm-up, a step in the setpoint reset schedule, or the recovery after a
mode change.

The rule reads the coil *command*, not a position feedback, and that is
deliberate: a valve commanded to 100% while its actuator has failed closed is
diagnosis 3, and reading the command is what catches it. The cost is that the
rule cannot distinguish a coil genuinely wide open and starved from one that
never moved.

## Possible Diagnoses

G36 §5.16.14 FC#7, transcribed:

1. SAT sensor error
2. Cooling coil valve leaking or stuck open
3. Heating coil valve stuck closed or actuator failure
4. Fouled or undersized heating coil
5. HW temperature too low or HW unavailable
6. Gas or electric heat unavailable
7. DX cooling stuck on
8. Leaking or stuck economizer damper or actuator

The list spans three failure modes that look identical from these three points:
the heat never arrives (3–6), something removes it after it arrives (2, 7, 8),
or it arrived and the sensor did not see it (1). Trending MAT alongside SAT
splits the first two groups — that comparison is AHU-0005.

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, PROXY_ESTIMATION, all three from the
reference's §5.8.1 index row, which puts the range at 2–5% of AHU energy and
maps no PNNL measure. The waste depends on the cause and the three points
cannot tell the causes apart, which is why the estimate is a proxy. Where
something removes heat (diagnoses 2, 7, 8) the unit pays twice and AHU-0016's
simultaneous heating and cooling term sizes it. Where the coil is starved or
fouled (3–6) the AHU wastes little on its own and the deficit moves downstream:
`makeup_kw = supply_airflow_m3s × 1.2 × 1.005 × (sat_sp − sat)`, delivered by
terminal reheat at worse efficiency than the central coil, or not at all in
zones with no terminal heat. Heating-dominant, since the rule is only evaluable
in OS#1.

## Emissions Impact

PROXY_EMISSIONS, MEDIUM confidence. Scope `1|2` because the inventory follows
the heat source: hot water from a gas boiler, a gas furnace section, or a steam
coil is Scope 1, while electric resistance, a heat pump, and the terminal
reheat making up the deficit are usually Scope 2. A site with a gas boiler and
electric reheat pays into both, and the cooling-side diagnoses (2, 7) add a
Scope 2 term of their own. Avoided-emissions basis: static combustion factor
for the fuel half, marginal operating emissions rate (MOER) for the electric
half.

## Deviations

- **The reference card is abbreviated; G36 is the normative source.** HVAC FDD
  Reference v1.0 carries AHU-0007 as a §5.8.1 index row only — no equation,
  tunables, diagnoses, operating-state applicability, or severity. Detection
  Logic and Possible Diagnoses are transcribed from ASHRAE Guideline 36
  §5.16.14 FC#7 as it appears in Addendum u to Guideline 36-2018 (first public
  review, 2021), with the Table 5.16.14.5 defaults (NISTIR 7365 provenance,
  which the addendum notes are biased toward minimizing false alarms).
- **Severity 3 is the library's.** No reference card states one and the §5.8.1
  index carries no severity column; the value matches this chapter's scaffold
  row and the other G36 comparison rules here.
- **The energy profile is the index row's; the emissions split is the
  library's.** `category`, `confidence`, `estimation_method`, and
  `savings_range` are copied from §5.8.1. The index has no emissions column, so
  `scope: 1|2` follows AHU-0020's convention for a fault whose fuel depends
  on which diagnosis is true; the cause-dependent runtime formula is the
  library's, its simultaneous-heating-and-cooling branch AHU-0016's term.
- **`SAT_AVG < SATSP − eSAT` rewritten in gap form.** Subtracting first and
  testing `sat_sp − sat > eSAT` keeps the tolerance the positive number G36
  publishes, retunable at one CXF path, instead of a negative offset ahead of
  the comparison. Same rearrangement as AHU-0001 and AHU-0028, with the
  same one-ulp caveat that no temperature sensor resolves.
- **`HC >= 99%` becomes a strict `>`.** CDL `Reals` has only strict
  comparisons, so a coil command parked at exactly 99.000% reads as
  not-at-full-capacity and the rule stays silent — the same deviation
  AHU-0001 documents for fan speed. A host whose valve command is quantized
  to whole percent should retune `hc_full_threshold` to 98.9.
- **Instantaneous samples instead of averaged signals.** G36 computes every
  signal as a five-minute rolling average of one-minute samples; this rule
  compares raw samples and leans on the 30-minute `persist` delay. Not
  equivalent — persistence resets on every compliant tick, so an oscillating
  SAT can hide indefinitely, while the steady shortfall of a starved or fouled
  coil reads the same either way. Same note as AHU-0002; a SAT swinging
  across the band rather than sitting under it is AHU-0022's finding.
- **Operating-state applicability and ModeDelay are frontmatter, not graph.**
  G36 scopes FC#7 to OS#1 and suspends every fault condition for 30 minutes
  after a mode change; both are host concerns under this library's stance
  (precedent AHU-0029), and a verdict outside OS#1 or inside the transition
  window is NO_EVAL, never healthy. The graph's `htg_vlv_cmd > 99` test is not
  that gate — it is the saturation half of the fault condition, and OS#1 admits
  any heating command above zero.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a shortfall already present at load waits out the full 30
  minutes rather than alarming on the first tick after a controller restart.

## Notes

Check the setpoint before the coil. A SAT setpoint that never resets
(AHU-0023) can hold a heating call the unit was never sized to meet, and a
reset schedule stepping up faster than the coil can follow produces this
signature for a few minutes at every step. Then work the supply side: hot water
temperature and pump status cost nothing to read, and diagnoses 5 and 6 take a
whole building's heating rather than one unit's. AHU-0013 is this fault
mirrored into cooling; AHU-0005 is its companion in the same state, and
FC-007 with FC-005 points at a heat sink running against the heating call. No
playbook covers coil capacity and heat-supply work yet.
