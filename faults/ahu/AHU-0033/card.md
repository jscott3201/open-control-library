---
schema: cxf-library/fault-card/v1
id: AHU-0033
name: Supply air temperature tracking error
equipment: ahu
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3 report (Air Handling Unit and VAV Box Diagnostics), §4.2 Table 1 rule 25 — the rule expression"
  - "Same report §4.2.3 — εt = 1.7 °C (3 °F), the flat threshold APAR applies to every temperature-comparison rule"
  - "Same report §4.1 and §4.2 — Modes 1-5 derived from coil-valve and damper signals; Table 2 places rule 25 in the comfort-requirements group and scopes it to Modes 1-4"
  - "Adapted from an internal paraphrased digest of that report; the report itself is not redistributed with this library"
  - "Sibling precedent: AHU-0007 and AHU-0013 (the valve-gated SAT misses this rule deliberately drops the gate from), AHU-0010 (same four-block graph shape)"
  - "Library extension: the HVAC FDD Reference v1.0 §5.8.1 indexes 31 AHU codes ending at AHU-0031 — see faults/ahu/README.md"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [AHU-0007, AHU-0013, AHU-0023]
playbooks: []
operating_states: "Occupied, in one of the four defined AHU modes — heating, cooling with outdoor air, mechanical cooling on 100% outdoor air, mechanical cooling at minimum outdoor air (APAR Modes 1-4, this library's OS#1-#4) — host-gated. The unknown-mode case is excluded; see Deviations."
preconditions: "Supply fan running and the unit occupied — a supply temperature describes nothing in still air, and an unoccupied unit is not chasing a setpoint. The unit must be in one of the four defined occupied modes, which APAR derives from the coil-valve and damper commands exactly the way this library derives its operating states, and the host must suspend evaluation for the usual ModeDelay (30 min) after any mode or operating-state change, plus through morning warmup and cooldown. That gate matters more here than on AHU-0007 or AHU-0013: this rule carries no actuator conjunct, so nothing inside it distinguishes a unit still stroking toward a new mode from a unit that cannot hold setpoint. `sat_sp` must be the setpoint the sequence is actively holding, reset included; bind a design constant against a unit that follows a reset schedule and the rule reports a fault every hour of every day. SAT sensor integrity is a precondition, not a conclusion — a sensor reading 2 K off produces a permanent tracking error at a perfectly tuned loop, and this rule reads no second signal that could tell the two apart, so clear whatever sensor-health rule the host runs on `sat` before believing the verdict. The rule discards the sign of the miss; a host that wants the direction reads `sat` and `sat_sp` alongside the verdict. When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - sat
  - sat_sp
outputs:
  - name: yFault
    description: True while |sat − sat_sp| has stayed above sat_error_threshold for at least alarm_delay. The only output — nothing in this rule is unevaluable from its own two inputs, so there is no evaluability flag and a host must not read one into it
params:
  sat_error_threshold:
    default: 1.7
    unit: "°C"
    description: "Two-sided band around the active setpoint that SAT may stray within before the miss counts. Default 1.7 °C (3 °F) is APAR's εt at §4.2.3, the single flat threshold that report applies to every one of its temperature-comparison rules. It is deliberately not G36's eSAT = 1.0 °C, which AHU-0007 and AHU-0013 carry under this same parameter name: eSAT is a supply-air sensor accuracy allowance, while εt is a heuristic band the source states covers measurement error generally. A site with a calibrated SAT sensor and a loop it trusts may lower it toward 1.0 for coherence with those two cards; raising it hides the mistuning this rule exists to find."
    cxf: gapBig.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous tracking error required before the alarm asserts (60 min). LIBRARY-CHOSEN — APAR specifies no per-rule persistence, and the implementation described in §4.3 evaluated its rules on hourly data, which is the nearest thing the source offers to a time constant. An hour is double the 30 min AHU-0007 and AHU-0013 use, because those two are protected by a saturated-valve conjunct that is itself rare and this rule has persistence and nothing else standing between it and every setpoint step, pulldown and load change. A site that wants the three SAT cards to alarm on the same clock retunes this to 1800."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU coil energy, and the downstream compensation a supply temperature off setpoint forces — terminal reheat, VAV airflow, and zone-level heat
  savings_range: "no published figure — APAR publishes no energy estimates for any of its 28 rules. The reference's §5.8.1 index gives the valve-gated siblings AHU-0007 and AHU-0013 2-5% of AHU energy; that range is carried here only as an order-of-magnitude anchor and is argued down in Energy Impact"
  climate_sensitivity: neutral
  runtime_estimation: "imbalance_kw = supply_airflow_m3s × 1.2 kg/m³ × 1.005 kJ/kg·K × |sat − sat_sp| — the conditioning the unit is delivering in excess of, or short of, what the sequence asked for. Airflow is neither a point of this rule nor available to it, so the host supplies it, which is what keeps the estimate a proxy. The sign decides what the number means and the rule does not carry it: SAT below setpoint in a cooling mode is over-cooling paid for at the coil and often again at terminal reheat, SAT above setpoint in a cooling mode is conditioning not delivered and costs airflow downstream instead, and both invert in heating."
emissions:
  scope: "1|2"
  method: PROXY_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 48
    failures: 8
    notes: "winter-only: fleet publishes one constant cooling-oriented SAT setpoint, so heating-season tracking error is real per the rule but the setpoint no longer means the active mode's target; per-mode setpoint binding required at deployment"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:3aeb1453a7bb1cd2a02efab2f51a4722"
  date: 2026-10-05
---

## Description

The supply air is not at its setpoint, and no actuator is at its stop. That
second half is what makes this rule worth having separately. A loop whose valve
has saturated is the easy case — the controller has asked for everything it has
and the air is still wrong — and AHU-0007 and AHU-0013 state it, one per
coil. A loop sitting 3 K off setpoint with its valve modulating around 60% is
the harder case: the controller is not out of capacity, it is out of tune, or
its valve has no authority left, or its actuator does not move until the ask
gets large. Both sibling cards test the valve command first, so neither can ever
report it. APAR states this as rule 25 and groups it with rules 3, 13 and 19 as
comfort sacrificed, only the other three additionally establishing that the loop
has run out of control authority. The fault is quiet by nature: zones
compensate, boxes open, reheat picks up, and nobody files a ticket about a
supply temperature.

## Detection Logic

```
APAR rule 25, applicable in every defined occupied mode:

    | Tsa − Tsa,s | > εt          εt = 1.7 °C (3 °F)

as implemented:

    sp_gap = sat − sat_sp
    yFault = (|sp_gap| > sat_error_threshold), sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0033 block graph](diagram.svg)

Four blocks, the same shape AHU-0010 uses for its equality test: `spGap`
subtracts the setpoint from the measurement, `absGap` folds the two signs
together, `gapBig` compares the magnitude against `sat_error_threshold`, and
`persist` requires 60 continuous minutes before reporting. There is no fifth
block, and the absence is the whole point of the card: every other SAT rule in
this library carries a second conjunct — a saturated valve on AHU-0007 and
AHU-0013, a reheat fraction on AHU-0019, a baseline on AHU-0022 — and each
conjunct is what makes its rule specific and also what makes it blind. Rule 25
buys generality by spending specificity: it says the unit is not delivering what
it was asked for and nothing at all about why. Persistence and the host's mode
gate are the only things keeping that statement from being noise, which is why
both are set conservatively; `delayOnInit = true` holds the hour across a
controller restart. `gapBig` is strict, as the source's rule 25 is, so a miss
sitting exactly on 1.7 K reads healthy — though exact equality is not reachable
in doubles from a realistic temperature pair (see Deviations).

## Possible Diagnoses

APAR names no per-rule causes; §4.2.2's fault classes read through rule 25 and
ordered by what an ungated tracking test finds first:

1. Control-loop tuning — a wide proportional band, a slow integral term, or a
   loop detuned to stop it hunting and left parked off setpoint since
2. Valve or damper authority: a valve sized to pass design flow at 20% open has
   no resolution left around setpoint
3. Actuator stiction, hysteresis, or a slipping linkage — the actuator does
   move, eventually, and never quite enough (AHU-0020 catches the frank case)
4. Coil or plant capacity short of saturation: degraded enough to miss setpoint,
   not enough to drive the valve to its stop, so the gated cards stay silent
5. Sequencing logic errors — a reset stepping faster than the unit can follow,
   two sequences writing the same coil output, a changeover leaving the unit on
   the previous mode's setpoint
6. SAT sensor error: the air is at setpoint and the reading is not. Cheapest to
   rule out, and the reason sensor health is a precondition here
7. Operator intervention — a valve in hand, a coil output overridden
   (AHU-0027 reports the override directly)
8. A coil fighting the other coil: simultaneous heating and cooling holds SAT
   off setpoint at part-open commands, which AHU-0016 names

## Energy Impact

COMFORT_ENERGY, MEDIUM confidence, PROXY_ESTIMATION. The category follows the
source: rule 25 is in APAR's comfort-requirements group, and what it establishes
first is that the building is not getting the air it asked for. The runtime
estimate is therefore an imbalance rather than a waste — `imbalance_kw =
supply_airflow_m3s × 1.2 × 1.005 × |sat − sat_sp|`, host-supplied airflow — and
the sign decides whether it is money: air colder than a cooling setpoint is
over-cooling paid once at the coil and often again at terminal reheat, air
warmer is under-delivery whose cost migrates downstream to boxes at maximum
flow. Both invert in heating. No savings range is published; the reference's
2–5% belongs to the saturated-coil siblings, so treat it as a ceiling. MEDIUM
because the cause — which decides the cost and even its sign — is not in the
rule's two inputs. Climate-neutral.

## Emissions Impact

PROXY_EMISSIONS, MEDIUM confidence. Scope is `1|2` because the rule spans every
occupied mode and cannot tell which coil is involved: a heating-side miss made
up by a gas boiler or steam coil is scope 1, while a cooling-side miss, electric
heating, and the fan energy moving make-up air are scope 2. Basis: static
combustion factor for the fuel half, MOER for the electric half. As on
AHU-0013, fixing an under-delivery instance can raise site emissions — a loop
that finally holds setpoint delivers conditioning the building was going
without — so the claim that survives is the over-conditioning branch plus
whatever downstream compensation stops.

## Deviations

- This card is a library extension, not a transcription: the reference's §5.8.1
  index ends at AHU-0031. The detection logic is APAR rule 25 (§4.2 Table 1)
  with its threshold from §4.2.3; severity, phase, energy and emissions grades,
  persistence, the diagnosis list and the prose are authored here. The source
  report is personally licensed and not redistributed with this library.
- The valve-position gate is absent on purpose, and that absence is the card.
  APAR pairs a SAT miss with a saturated coil in rules 3, 13 and 19 and states
  rule 25 with the temperature term alone. AHU-0007 and AHU-0013 argue that
  a SAT miss at a part-open valve is a loop working through a load change —
  right for a rule that fires in half an hour, wrong as a general claim, since
  a loop 30 minutes or six months off setpoint at 60% command is working
  through nothing. This rule takes the other half of the trade and pays with a
  longer persistence and a stricter host gate.
- The overlap with AHU-0007 and AHU-0013 is real and not suppressed: a
  saturated valve missing setpoint trips this rule too, an hour later. The two
  findings are different statements and the gated one is more informative when
  both are true, so a host wanting one alarm should rank rather than silence.
- εt = 1.7 °C ships flat as the source states it, not composed. The
  G36-lineage cards here do compose their bands (AHU-0010 in quadrature,
  AHU-0005 linearly), so this is a departure from local practice in favour of
  source fidelity — and the honest reading, since the band must absorb sensor
  error *and* the tracking error a healthy proportional loop shows at partial
  load, of which only the first half has a published budget.
- `sat_error_threshold` shares its name with AHU-0007 and AHU-0013 but not
  its value or meaning: 1.0 °C there (G36's eSAT, one-sided), 1.7 °C here
  (APAR's εt, two-sided). The shared name keeps one vocabulary for "how far SAT
  may stray"; the parameter description states the difference so nobody copies
  a value across.
- `alarm_delay = 3600 s` is library-chosen; APAR states no persistence. Its
  §4.3 implementation evaluated rules on hourly data, and an hour matches this
  chapter's other chronic conditions. It doubles the siblings' 30 minutes
  because their second conjunct is rare enough that most transients never reach
  their timer, whereas here the timer is the only defence.
- No boundary rewrite: rule 25 is already strict, unlike the G36 `≥` the sibling
  cards convert. Exact equality is also unreachable — the double nearest 1.7
  needs mantissa bits down to 2⁻⁵² and a difference of two temperatures in the
  8–16 binade is a multiple of 2⁻⁴⁹ — so the vectors bracket the line instead
  of landing on it. Same class of finding as HW-0004's 5.55 K trip line.
- Mode scope follows the source's Table 2 (Modes 1–4), not Table 1's "all
  occupied modes" heading, which would include APAR's unknown mode. That is
  where the report puts mode transitions and simultaneous heating and cooling,
  and a SAT miss there is already reported with its cause attached by
  AHU-0016 and AHU-0029. A host can widen the gate.
- Mode gating is host-side, and the source agrees: APAR classifies its five
  modes from coil-valve and damper signals alone, then evaluates only the
  applicable rules — this library's `operating_states` plus `preconditions`
  convention, reached independently two decades earlier. A verdict produced
  outside the four modes, in a transition window, or with the fan off is
  NO_EVAL and never healthy.
- Instantaneous samples against an hourly source. An hourly average tolerates a
  signal that keeps crossing back while its mean stays outside the band;
  persistence does not, so SAT oscillating across the band never alarms here.
  That case is AHU-0022's to report, which is the reason to deploy both.
- The sign of the miss is computed and then discarded, because the source's
  expression is a magnitude. Exposing it would add a block and an output to a
  rule whose value is its bluntness, and a host holding `sat` and `sat_sp` has
  the sign for free. Same treatment as AHU-0010.
- `outputs` carries `yFault` alone: every evaluability question this rule has —
  occupancy, mode, fan status, whether the setpoint is live, whether the SAT
  sensor is trustworthy — needs a signal the rule does not bind, so all of them
  are preconditions and none qualifies for a `y…Ok` output.
- Severity 3, phase 2 and the energy block are library-assigned; no reference
  row exists to copy. Severity 3 matches every comparison rule in this chapter,
  and `savings_range` declines to invent a number.
- APAR publishes an expression and a threshold, not test cases, so every
  scenario in `vectors.json` is authored.
- `persist.delayOnInit = true` (the CDL default is `false`), the library's
  standing choice: a miss already present at load waits out the full hour
  rather than alarming on the first tick after a controller restart.
- `playbooks`, `clusters` and both suppression lists are empty. `playbooks/` has
  no loop-tuning or coil-capacity playbook, which is what the first four
  diagnoses dispatch, and `sensor-drift` and `missing-reset` each cover one
  diagnosis apiece — listing them would over-claim. Cluster membership is
  arguable but is the index owner's edit, not this card's.

## Notes

Read this card as the complement to AHU-0007 and AHU-0013, not a
replacement. The three partition the SAT-miss space by what the actuator is
doing: those two cover the saturated end, where the diagnosis list is short and
the fix is usually mechanical, this one everything below saturation, where the
list is long and the fix is usually at a keyboard. Tripping alone, it is a
tuning, authority, or sequence problem until proven otherwise; tripping with one
of the gated pair, it is repeating that card's fault an hour later.

Check the setpoint before the loop: a unit holding a design setpoint through a
mild afternoon can miss it for reasons that have nothing to do with the loop
(AHU-0023). This rule is only as meaningful as the setpoint it is handed —
which is also the caution for any host binding a design constant to `sat_sp`.
Then read the sign, which the rule computes and does not report: consistently
warm in a cooling mode points at capacity, authority, or a coil fighting another
coil, consistently cold at over-cooling (check AHU-0019), and a miss that
changes sign through the day at tuning or hunting (AHU-0022).
