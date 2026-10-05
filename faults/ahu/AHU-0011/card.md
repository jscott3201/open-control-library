---
schema: cxf-library/fault-card/v1
id: AHU-0011
name: OAT too low for mechanical cooling
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §5.8.1 (index; card abbreviated)"
  - "G36 §5.16.14 FC#11 (text per Addendum u public review)"
  - "NISTIR 7365 (defaults provenance)"
g36: "§5.16.14 FC#11"
clusters: [CLU-03]
suppresses: []
suppressed_by: []
related: [AHU-0017, AHU-0009]
playbooks: [economizer-failure]
operating_states: "OS#3 (mechanical + economizer cooling) — host-gated"
preconditions: "Evaluate only in G36 OS#3, which Table 5.16.14.2 defines by actuator signature: heating coil = 0, cooling coil > 0, OA damper = 100%. Per §5.16.14.11 the host suspends evaluation while the AHU is not operating and for ModeDelay (30 min) after a mode change in any zone group the AHU serves; per §5.16.14.12 fault conditions not applicable to the current OS are not evaluated at all. Verdicts outside OS#3, and verdicts inside a transition window, are NO_EVAL — never healthy. `sat_sp` must be the setpoint the economizer is controlling to (G36's SATsp for heating-coil and economizer control), not the cooling-coil setpoint SATsp-C. The 3.0 °C default threshold assumes a local OAT sensor at the unit; a site on a global OAT sensor must retune it before trusting the verdict."
points:
  - oat
  - sat_sp
outputs:
  - name: yFault
    description: True while outdoor air has stayed more than oat_deficit_threshold below the supply air setpoint continuously for at least alarm_delay
params:
  oat_deficit_threshold:
    default: 3.0
    unit: "°C"
    description: "Amount by which oat may fall below sat_sp before mechanical cooling counts as unnecessary. The default composes the G36 §5.16.14 internal variables as eOAT + eSAT + dTSF = 1 + 1 + 1 = 3.0 °C, using the local-sensor eOAT. Retuning any one term means recomputing the sum: a global OAT sensor (eOAT = 3 °C) gives 5.0 °C; a measured 2 °C fan rise gives 4.0 °C."
    cxf: gapBig.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (G36 AlarmDelay, 30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU mechanical cooling — chiller or DX doing work free cooling was already doing
  savings_range: "Sensor-dependent (reference §5.8.1 row, EEM 06); bounded above by the cooling energy spent while the unit sits in OS#3 instead of OS#2"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "none — the rule sees two temperatures and no airflow or coil load, so no waste term is computable from its inputs"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 26
    failures: 0
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:c58377cb68e22f9e5e9d4f66cc906bba"
  date: 2026-10-05
---

## Description

The unit is running mechanical cooling on top of a fully open outdoor air
damper while outdoor air is already colder than the supply setpoint by more
than the fan will add back. In OS#3 the dampers sit at 100% and the chiller
trims what the economizer cannot deliver, which is legitimate — until OAT is
more than `dTSF` below setpoint, at which point free cooling alone would
overshoot and the unit belongs in OS#2 modulating back toward a blend.

Something is holding the machine in the wrong state: the sequence never changed
over, a sensor is lying about which side of setpoint the air is on, or a
heating coil is leaking warmth into a stream the chiller then removes — the
expensive version, two subsystems paying to cancel each other. Within CLU-03
(Economizer Failure) this is the third face of the same changeover problem:
AHU-0017 catches a damper that stays shut when outdoor air is useful,
AHU-0009 an economizer still modulating after outdoor air stopped being
useful, and this one mechanical cooling still running when the economizer alone
had it covered.

## Detection Logic

```
G36 §5.16.14 FC#11, applies to OS#3:

    OAT_AVG + eOAT < SATSP − dTSF − eSAT

rearranged to gap form:

    sat_sp − oat > eOAT + eSAT + dTSF = 1 + 1 + 1 = 3.0 °C

yFault = (sat_sp − oat > oat_deficit_threshold), sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0011 block graph](diagram.svg)

G36 spreads the two sensor error bands and the fan-heat correction across both
sides of the inequality; the rearrangement collapses them into the single
positive `gapBig.t`, so a host retunes one parameter instead of tracking which
side each term sits on. Note the operand order: `gap` is `sat_sp − oat`, the
reverse of AHU-0009's, because the two rules look at opposite ends of the
same band and each wants its own end positive.

G36's comparison is already strict, so the rearranged `GreaterThreshold`
reproduces it exactly and a gap of exactly 3.0 °C reads healthy in both.
`persist` requires 30 minutes of continuous violation and any interruption
restarts the timer; recovery is immediate on the tick the gap falls back under
the threshold. Either input can open the gap — raising the supply setpoint does
it as surely as a cold front.

## Possible Diagnoses

Per G36 §5.16.14 Table 5.16.14.8, FC#11:

1. SAT sensor error — a supply reading biased high keeps the cooling loop
   calling for capacity the air does not need
2. OAT sensor error — a sensor reading high keeps the changeover logic from
   recognizing that free cooling would now overshoot
3. Heating coil valve leaking or stuck open — warmth upstream of the cooling
   coil is heat the chiller then removes, and the reason the unit needs
   mechanical cooling on air that arrived cold. The most expensive diagnosis
   here, and the one that also lights up FC#15 where that rule is deployed
4. Leaking or stuck economizer damper or actuator — a damper reported at 100%
   but physically short of it, or a return damper that will not close, delivers
   a warmer mixture than OAT implies

## Energy Impact

COMFORT_ENERGY, LOW confidence, QUALITATIVE_ONLY — the reference's §5.8.1 index
row, which maps the fault to PNNL-25985 EEM-06 (OA damper faults). Mechanical
cooling is doing free cooling's job: every kilowatt at the chiller buys a
temperature drop the outdoor air had already made. The waste is real and
continuous but not computable here — two temperatures carry no airflow, no coil
load, and no counterfactual for the correct state. An upper bound is the
cooling energy spent over the fault's duration; a host with airflow and coil
data can substitute the direct measurement AHU-0017 uses. Cooling-dominant,
and most common in shoulder seasons when outdoor air swings across the
changeover point.

## Emissions Impact

QUALITATIVE_EMISSIONS, LOW confidence, Scope 2. The waste is chiller or DX
compressor electricity plus fan energy already being spent. Diagnosis 3 adds
on-site combustion at the boiler, which would be Scope 1 — but that heat is
inferred rather than observed, and the coil it feeds is commanded shut in OS#3,
so the emissions this rule can honestly claim are the purchased-electricity
ones. Avoided-emissions basis: N/A.

## Deviations

- **The reference card is an index row, so this card is built from G36.**
  §5.8.1 gives the code and the name and nothing else. The equation, OS#3
  applicability, four diagnoses, and internal-variable defaults are transcribed
  from ASHRAE Guideline 36 §5.16.14 as it appears in Addendum u to Guideline
  36-2018 (first public review, 2021).
- **Severity 3 is library-assigned.** The reference publishes none; the value
  matches this chapter's scaffold row and every other G36 001-range card here.
  G36's Level 3 alarm grading (§5.16.14.16) is a reporting priority rather than
  a ranking, but it points the same direction.
- **Energy profile follows the §5.8.1 index row** (COMFORT_ENERGY / LOW / QUAL,
  EEM-06, savings "sensor-dependent"), and the published row wins where one
  exists. EXCESS_CONSUMPTION is a defensible alternative — setpoint is being
  met, so the unit is comfortable and wasteful rather than uncomfortable — and
  is recorded, not adopted. The emissions block is library-assigned.
- **Combined-epsilon threshold.** G36 puts `eOAT` on the measured side and
  `dTSF` and `eSAT` on the setpoint side, which would bind one card parameter
  to three block parameters. `sat_sp − oat > eOAT + eSAT + dTSF` puts one
  positive number on one CXF path, algebraically identical, with the
  composition recorded in the parameter description. Same move as AHU-0028
  and AHU-0009.
- **The default assumes a local OAT sensor.** G36 gives eOAT as 1 °C at the
  unit and 3 °C for a global sensor. A site feeding this rule from a campus or
  weather-service OAT must set `oat_deficit_threshold` to 5.0 °C (3 + 1 + 1);
  leaving it at 3.0 makes the rule fire on sensor disagreement G36 considers
  within tolerance.
- **No boundary deviation for this fault.** FC#11's comparison is already
  strict (`<`), unlike the `≥`/`≤` forms elsewhere in Table 5.16.14.8 (FC#5,
  FC#12, FC#14, FC#15), so `GreaterThreshold` on the rearranged gap reproduces
  the source exactly.
- **Instantaneous samples instead of averaged signals.** G36 compares 5-minute
  rolling averages sampled at 1-minute intervals; this rule compares raw
  samples and leans on the 30-minute `persist` delay. Not equivalent —
  persistence resets on every compliant tick, so an oscillating OAT can hide
  indefinitely, while the steady offset this rule is for reads the same either
  way. (Honesty note carried from AHU-0002.)
- **Operating-state gating and NO_EVAL are frontmatter, not graph.** G36 scopes
  FC#11 to OS#3 (§5.16.14.9c), suspends evaluation for ModeDelay after a mode
  change, and suspends it entirely when the AHU is off (§5.16.14.11). The
  engine is status-blind and the graph computes fault-given-valid-data only
  (precedent AHU-0029). A host that evaluates in OS#2 will see this rule
  assert on every cold morning — cold outdoor air with the dampers modulating
  is free cooling working.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`): a gap already
  open at load waits out the full 30 minutes rather than alarming on the first
  tick after a controller restart. Library-wide choice, per AHU-0016.

## Notes

The instructive property of this rule is only visible next to its pair.
AHU-0009 and AHU-0011 test the same two points against the same epsilons
and both account for the same 1 °C fan rise, but FC#9 **subtracts** fan heat
(threshold 1.0 °C) while FC#11 **adds** it (3.0 °C): fan heat narrows the
usable free-cooling band from the top and widens it from the bottom. A site on a
global OAT sensor recomputes both — 5.0 °C here, 3.0 °C for AHU-0009 — and
they do not scale together. The two cannot fire at once, since one needs OS#2
and the other OS#3; a unit that alternates between them across a day is saying
the changeover point is in the wrong place or the OAT sensor is unreliable.
Check that sensor before anyone edits changeover logic — step 1.2 of the
[economizer-failure](../../../playbooks/economizer-failure.md) playbook.
