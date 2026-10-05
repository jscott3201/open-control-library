---
schema: cxf-library/fault-card/v1
id: AHU-0019
name: Supply air temperature setpoint too low (over-cooling)
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0019"
  - "PNNL-27338"
  - "PNNL-25985 EEM-05/EEM-15"
g36: null
clusters: [CLU-02]
suppresses: []
suppressed_by: []
related: [VAV-0001, AHU-0022, AHU-0023]
playbooks: [missing-reset]
operating_states: "Occupied (OS 2, 3, 4)"
preconditions: "AHU in occupied mode and serving multiple zones; zone reheat data available and aggregated by the host into zone_reheat_fraction. When the zone data is missing, stale, or covers too few zones for the fraction to mean anything, the verdict is NO_EVAL, not healthy."
points:
  - sat_sp
  - zone_reheat_fraction
outputs:
  - name: yFault
    description: True while the SAT setpoint has stayed below sat_sp_low_limit with more than reheat_fraction_threshold of zones reheating, for at least alarm_delay
params:
  sat_sp_low_limit:
    default: 12.0
    unit: "°C"
    description: Minimum recommended SAT setpoint; below this the air is colder than any zone needs
    cxf: spLow.t
  reheat_fraction_threshold:
    default: 0.5
    unit: "1"
    description: Fraction of served zones reheating (0-1) above which the cold air is demonstrably being reheated
    cxf: rhtHigh.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (60 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU cooling + downstream VAV reheat
  savings_range: 5-15% of AHU cooling plus reheat energy (PNNL-25985 EEM-05 SAT reset + EEM-15 VAV minimum flow, combined)
  climate_sensitivity: heating-dominant
  runtime_estimation: "excess_reheat_kw = Σ over reheating zones of (rht_vlv_cmd_i/100 × vav_rht_capacity_kw_i)"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:07fe9d3b43f17351d191782e29b6577c"
  date: 2026-10-05
---

## Description

The supply air temperature setpoint sits below the minimum recommended value
while a large share of the zones served are running reheat. Air colder than any
zone asked for costs chiller energy to make and then boiler or electric reheat
energy to undo, so every degree of over-cooling is paid for twice — and the
zones stay comfortable throughout, which is why a setpoint parked at 10 °C
survives for years. SAT reset is absent in 74% of buildings (PNNL 151-building
study); this is the CLU-02 member fault that shows the missing reset actually
costing money.

## Detection Logic

```
yFault = sat_sp < sat_sp_low_limit
     AND zone_reheat_fraction > reheat_fraction_threshold
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0019 block graph](diagram.svg)

Two threshold tests feed one conjunction and one timer. `spLow` watches the
setpoint, not the measured SAT: this rule is about what the sequence asked for,
while a unit that cannot hold its setpoint (AHU-0007, AHU-0013) or hunts
around it (AHU-0022) is a separate finding. `rhtHigh` supplies the
corroboration that turns "cold setpoint" into "waste" — without a reheat
majority, a 10 °C setpoint may simply be serving a high-load hour. Both
comparisons are strict, so a setpoint parked exactly on the 12 °C limit, or
exactly half the zones reheating, does not trip the rule. `persist` requires 60
minutes of continuous violation — enough to ride out morning cool-down and the
reheat spike after an occupied-mode transition — and `delayOnInit = true`
holds that window across a controller restart.

## Possible Diagnoses

1. SAT setpoint too aggressive — set low at commissioning and never reset since
2. SAT reset logic disabled or misconfigured (the CLU-02 root cause; confirm
   with AHU-0023)
3. A single rogue zone dragging the AHU setpoint down through the
   trim-and-respond request path — one starved or mis-sensored box can hold the
   whole system at its minimum setpoint

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, PROXY_ESTIMATION. The waste is a reheat
integral: `excess_reheat_kw = Σ over reheating zones of (rht_vlv_cmd_i/100 ×
vav_rht_capacity_kw_i)`, plus the chiller energy spent making air nobody
wanted. Raising the setpoint into the reset band saves 5–15% of AHU cooling
plus reheat energy (PNNL-25985 EEM-05 SAT reset and EEM-15 VAV minimum flow
reduction, combined — the two interact, since a high minimum flow forces reheat
no setpoint change can eliminate). Heating-dominant: the reheat half of the
bill grows with hours spent below balance point.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, HIGH confidence; typical 800–5,000 kg CO₂e/yr (excess
cooling plus reheat). Sites with gas or steam reheat move that half of the
inventory into scope 1; the card reports scope 2 because electric reheat and
electric chilling are the common case. Avoided-emissions basis: marginal
operating emissions rate (MOER).

## Deviations

- The reference's single 50% zone-count fraction diverges from its own PNNL
  source: PNNL-27338 §2.2.2–2.2.3 tests two quantities — zones with reheat
  valves open (>10%) exceeding 25%, AND fleet-average reheat command above 50%.
  This card follows the reference. Sites wanting the PNNL-literal form retune
  `reheat_fraction_threshold` to 0.25; the magnitude conjunct is not
  expressible without a host-derived mean reheat command, which the point
  dictionary does not yet carry.
- The reference counts reheating zones across a per-zone valve command array.
  Library v1 avoids array boundary points, so the host does the counting and
  feeds the scalar `zone_reheat_fraction` (flagged `derived` in the dictionary,
  same pattern as `zone_dmpr_pos_max` in AHU-0024). The reheat-active
  counting threshold is host configuration, not a rule parameter.
- `reheat_fraction_threshold` is a fraction 0–1, not a percent: the reference
  states 50%, but the point compared against carries unit `1`, so the parameter
  is 0.5. Hosts feeding a 0–100 percentage fire this rule on nearly every tick.
- Both comparisons are strict (`<`, `>`); the reference does not specify
  boundary behavior, so the library's strict convention applies.
- The reference tags this fault for both AHU and RTU. This card is the
  AHU-family instance; an RTU-0004 would restate it against the RTU's
  discharge setpoint and its zone group.
- Operating-state gating (OS 2, 3, 4 — occupied) and the multi-zone
  precondition are declared in frontmatter for host enforcement rather than
  encoded in the block graph, per the library's design stance.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a violation already present at load waits out the full
  60 minutes instead of alarming on the first tick after a controller restart.

## Notes

AHU-0023 and this rule are the two halves of the same CLU-02 story. FC-057 is
the statistical trigger — it proves the setpoint never moves. This rule is the
harm case: the setpoint is parked low *and* the zones downstream are burning
fuel to undo it. Fix order is FC-057's fix — program the reset per G36 §5.16.2
(playbook `missing-reset`, step 2.2) — after which this rule should clear
within one occupied day. Before raising the setpoint, check the zone minimum
flows: a box with a 40% minimum will reheat at any SAT and will keep this rule
firing after the reset is programmed.
