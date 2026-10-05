---
schema: cxf-library/fault-card/v1
id: AHU-0012
name: SAT too high vs MAT in cooling
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
  - "G36 §5.16.14 FC#12 (text per Addendum u public review)"
  - "NISTIR 7365 (defaults provenance)"
g36: "§5.16.14 FC#12"
clusters: [CLU-01]
suppresses: []
suppressed_by: [AHU-0028]
related: [AHU-0016, AHU-0005, AHU-0013, AHU-0028, AHU-0032]
playbooks: [simultaneous-hc]
operating_states: "OS#2-#4 (any cooling-side state) — host-gated"
preconditions: "Supply fan running, and the unit in one of the cooling-side operating states G36 defines by actuator signature: OS#2 (HC = 0, CC = 0, minimum < OA damper < 100%), OS#3 (HC = 0, CC > 0, OA damper = 100%), or OS#4 (HC = 0, CC > 0, OA damper at minimum). Omit the rule on a unit with no MAT sensor — G36 marks FC#12 `omit if no MAT sensor`, and a MAT inferred from a mixing model rather than measured makes the comparison circular. Suspend evaluation for ModeDelay (30 min) after any mode or operating-state change in a zone group the AHU serves, while actuators are still stroking to their new positions. Silence the rule while AHU-0028 is active: a MAT outside the OAT/RAT envelope is not a number to compare anything against. When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - sat
  - mat
outputs:
  - name: yFault
    description: True while SAT has stayed more than sat_mat_gap_threshold above MAT for at least alarm_delay
params:
  sat_mat_gap_threshold:
    default: 5.0
    unit: "°C"
    description: "How far SAT may exceed MAT before the air is warming for a reason no cooling-side state explains. Default 5.0 °C is G36's composition eSAT + eMAT + dTSF = 1 + 3 + 1: the supply-air and mixed-air sensor allowances plus the temperature rise across the supply fan. A site retuning a sensor allowance recomputes that sum — a local calibrated MAT sensor lowers eMAT, and a measured fan rise replaces dTSF"
    cxf: gapBig.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (G36 AlarmDelay, 30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU cooling coil and the heat source running against it
  savings_range: "2-5% of AHU energy (HVAC FDD Reference §5.8.1 index row; no PNNL EEM mapped)"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "waste_kw = supply_airflow_m3s × 1.2 kg/m³ × 1.005 kJ/kg·K × (sat − (mat + dTSF)) — the thermal power being added to air the cooling coil is then paid to remove again. The term sizes the heat added; the cooling that cancels it is a second, roughly equal charge at the plant's efficiency. Design airflow stands in for a measured one, which is what keeps this a proxy rather than a direct measurement; when the cause turns out to be a sensor there is nothing to count"
emissions:
  scope: "1+2"
  method: PROXY_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 48
    failures: 0
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:d771011dc1df5622a20da08a347948cc"
  date: 2026-10-05
---

## Description

Air crossing an air handler in a cooling-side state can gain a little heat from
the supply fan and nothing else. When SAT reads more than the combined sensor
and fan-heat allowance above MAT, the stream is picking up heat from a source
that should not be running: a gas or electric stage stuck on, a heating valve
leaking through, or a heating coil that never closed when the unit left OS#1.
The alternative is that one of the two sensors is lying.

This is G36 §5.16.14 FC#12, AHU-0005 read in the mirror; both are statements
about the sign of the temperature change across the unit, each evaluated only
in the states where its sign is the expected one. A heating source active while
the unit is cooling is simultaneous heating and cooling under another name,
which puts this rule in CLU-01 behind AHU-0016. The difference is what each
can see: AHU-0016 needs both valve commands and catches the conflict at the
command layer, while this rule reads two temperatures and catches it at the air
stream — including the case AHU-0016 misses, where the heating command reads
zero and heat arrives anyway.

## Detection Logic

```
gap    = sat − mat
yFault = gap > sat_mat_gap_threshold,
         sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0012 block graph](diagram.svg)

G36 writes the test as `SAT_AVG − eSAT − ΔTSF ≥ MAT_AVG + eMAT`; moving the
constants to one side gives `SAT − MAT ≥ eSAT + eMAT + ΔTSF`, one subtraction
against the composed 5.0 °C threshold. The comparison is strict, so a gap
sitting exactly on 5.0 °C reads healthy where G36 would report the fault.
`persist` requires 30 minutes of continuous violation and any interruption
restarts the timer, which separates a stuck heat source from a heating stage
finishing its off-cycle purge; recovery is immediate on the tick the gap falls
back inside the allowance.

Nothing below the threshold can trip this rule, including a gap of −20 °C. A
cooling coil that has stopped cooling shows up here only once the air is
actively being heated; the case where SAT merely fails to reach setpoint with
the valve wide open is AHU-0013's.

## Possible Diagnoses

Transcribed from G36 §5.16.14 FC#12:

1. SAT sensor error
2. MAT sensor error
3. Cooling coil valve stuck closed or actuator failure
4. Fouled or undersized cooling coil
5. CHW temperature too high or CHW unavailable
6. DX cooling unavailable
7. Gas or electric heat stuck on
8. Heating coil valve leaking or stuck open

Diagnoses 7 and 8 are what make this a waste fault rather than a capacity
fault: they are the only entries that put energy *into* the stream. Air merely
failing to be cooled lands about one fan-heat rise above MAT, not five degrees,
so the cooling-side entries usually reach this rule in combination — a leaking
heating coil that a working chilled-water coil had been masking becomes visible
the moment the cooling capacity goes away.

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, PROXY_ESTIMATION, savings 2–5% of AHU
energy — the §5.8.1 index row, the only energy statement the reference makes
here (no EEM mapped). The waste has two halves, the heat someone paid to add
and the cooling paid to remove it again:

```
waste_kw = supply_airflow_m3s × 1.2 × 1.005 × (sat − (mat + dTSF))
```

That sizes the first half; the second is a further charge of roughly the same
magnitude at the plant's efficiency. Design airflow substituting for a measured
one keeps the term a proxy, and MEDIUM rather than HIGH because the rule cannot
separate its waste diagnoses from its sensor diagnoses — a mis-calibrated SAT
sensor draws the identical trace and wastes nothing. Cooling-dominant, since
the rule is evaluated only in cooling-side states.

## Emissions Impact

PROXY_EMISSIONS, MEDIUM confidence. Scope `1+2`, following AHU-0016: when the
fault is real both inventories are paid into at the same moment — gas heat
adding energy (Scope 1) and purchased electricity driving the chiller that
removes it (Scope 2). The `1` is contingent on the heat being combustion; on an
all-electric unit the exchange collapses to Scope 2. The heating-side mirror
AHU-0005 records the same exchange as `1|2` for that contingency, so the pair
differs on notation rather than physics. When the cause is a sensor there is
nothing to attribute. Avoided-emissions basis: marginal operating emissions
rate (MOER) for the electric half, static combustion factor for the fuel half.

## Deviations

- **The reference card is abbreviated; G36 is the normative text.** The HVAC
  FDD Reference carries AHU-0012 only as a §5.8.1 index row — no equation,
  internal variables, vectors, severity, diagnoses, or preconditions. Detection
  logic and the diagnosis list are transcribed from ASHRAE Guideline 36
  §5.16.14 FC#12 as it appears in Addendum u to Guideline 36-2018 (First Public
  Review, 2021).
- **Combined threshold instead of three separate allowances.** eSAT = 1 °C,
  eMAT = 3 °C, and ΔTSF = 1 °C (NISTIR 7365 defaults the addendum notes are
  "intentionally biased toward minimizing false alarms") compose into one
  positive 5.0 °C threshold on one CXF path, so a host retunes one number
  instead of three. Recompute as `eSAT + eMAT + ΔTSF`: a local calibrated MAT
  sensor at eMAT = 1 °C gives 3.0, a measured 2 °C fan rise gives 6.0. Same
  rearrangement as AHU-0028 and AHU-0001.
- **G36's `≥` becomes a strict `>`.** CDL `Reals` offers only strict
  comparisons, so a gap of exactly 5.000 °C reads healthy where G36 reports the
  fault. Measure zero on a real temperature signal, and it errs toward silence.
  A host binding coarsely quantized temperatures (integer °C, or a BAS that
  rounds to 0.5) should retune `sat_mat_gap_threshold` to 4.9.
- **Instantaneous samples instead of 5-minute rolling averages.** G36 computes
  every §5.16.14 signal as a 5-minute rolling average with 1-minute sampling;
  this library consumes instantaneous points and lets the 30-minute AlarmDelay
  stand in. Not equivalent — persistence resets on every compliant tick, so an
  oscillating gap can hide indefinitely, while the steady offset of a stuck heat
  source or a drifted sensor reads the same either way. (Honesty note from
  AHU-0002.)
- **Operating states and ModeDelay are host-side preconditions.** G36 scopes
  FC#12 to OS#2–#4, suspends evaluation for ModeDelay (30 min) after a mode
  change in a served zone group, and suspends it entirely while the AHU is off.
  None of it is in the graph, per the library's stance (precedent AHU-0029).
  G36's `omit if no MAT sensor` qualifier is a deployment decision of the same
  kind and lives in `preconditions` too.
- **OS#2 is included on the addendum's own authority.** The published FC#12 row
  applies to OS#3–#4; Addendum u shows the applicability edited to OS#2–#4,
  which is what is transcribed here — free cooling with a modulating damper is a
  cooling-side state like the other two. A host running the unedited 2018 text
  gates OS#3–#4 only; the graph is identical either way.
- **Severity 3 is the library's.** No chapter card states one and the §5.8.1
  index carries no severity column. G36's Level 3 alarm grading is a reporting
  priority rather than this library's 1–4 scale, so it corroborates without
  supplying.
- **The energy profile is the index row's; the runtime formula and scope are
  the library's.** `category`, `confidence`, `estimation_method`, and
  `savings_range` are copied from §5.8.1. The proxy formula is mirrored from
  AHU-0005 with the sign flipped — that rule counts heat removed from air the
  heating coil paid to warm, this one heat added to air the cooling coil pays
  to remove.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a violation already present at load waits out the full 30
  minutes instead of alarming on the first tick after a controller restart.

## Notes

The threshold asymmetry with AHU-0005 is worth understanding before either
number is retuned. The heating-side rule tests `MAT − SAT` against
eSAT + eMAT − ΔTSF = 3.0 °C; this one tests `SAT − MAT` against
eSAT + eMAT + ΔTSF = 5.0 °C. The sensor allowances are identical and the fan
heat is what differs: expected warming excuses SAT running warm and indicts SAT
running cold. Start with the two sensor diagnoses — cheapest to eliminate and
most likely to be right — and note that an active AHU-0028 should already be
suppressing this rule. If the sensors check out, the
[simultaneous-hc](../../../playbooks/simultaneous-hc.md) playbook covers the
rest: the heat source is on, and what remains is whether the command, the
valve, or the sequence is responsible.
