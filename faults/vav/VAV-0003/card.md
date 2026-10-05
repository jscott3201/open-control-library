---
schema: cxf-library/fault-card/v1
id: VAV-0003
name: Reheat valve open with zone satisfied
equipment: vav
status: verified
phase: 2
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §10, VAV-0003"
  - "PNNL retuning"
  - "PNNL-25985 EEM-15/38"
g36: null
clusters: [CLU-05]
suppresses: []
suppressed_by: []
related: [AHU-0016, VAV-0001, VAV-0006]
playbooks: [vav-min-flow-reheat]
operating_states: "deadband / satisfied (host-gated)"
preconditions: "The AHU serving this VAV box is running — with no supply air moving, a reheat valve position means nothing thermally and the zone temperature is drifting on envelope loads rather than on anything the box is doing. The host must bind the ACTIVE occupied-mode heating and cooling setpoints the zone loop is tracking, not schedule defaults: during setback the effective band is wider than the occupied one and reheat inside the occupied band can be legitimate. The zone temperature sensor must be trustworthy — VAV-0010 territory — since a sensor reading high while the space is genuinely cold produces exactly this signature with nothing wrong."
points:
  - rht_vlv_cmd
  - zone_temp
  - zone_temp_sp_htg
  - zone_temp_sp_clg
outputs:
  - name: yFault
    description: True while the reheat valve is open past its threshold with the zone inside the satisfied band, continuously for at least alarm_delay
params:
  reheat_open_threshold:
    default: 15.0
    unit: "%"
    description: Reheat valve command above which the coil counts as open
    cxf: rhtOn.t
  zone_deadband:
    default: 0.5
    unit: °C
    description: Tolerance widening the satisfied band past each setpoint; binds both the lower and the upper bound
    cxf: [aboveHtg.t, belowClg.t]
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: VAV zone reheat — pure waste while the zone is satisfied
  savings_range: 5-15% of zone thermal energy
  climate_sensitivity: heating-dominant
  runtime_estimation: "waste_kw = rht_vlv_cmd/100 × vav_rht_capacity_kw — with the zone inside its deadband there is no useful heating output to net out, so the whole coil load is waste"
emissions:
  scope: "1"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:ea366afd07e1888e9fd147a855249bcc"
  date: 2026-10-05
---

## Description

The zone is comfortable — sitting between its heating and cooling setpoints,
asking for nothing — and the reheat coil is running anyway. Unlike an oversized
minimum flow, where the reheat at least offsets air the ventilation code
required, every kilowatt through this coil is waste with no offsetting benefit,
which is why the category is CRITICAL_WASTE and the runtime estimate has no
efficiency term: `waste_kw = rht_vlv_cmd/100 × vav_rht_capacity_kw`, the whole
coil load. The fault hides well, because a satisfied zone with extra heat in it
drifts up until the cooling loop opens the damper and takes the heat back out —
the box masks its own symptom, and the only trace is two subsystems working
against each other at a scale too small to see on a meter until you multiply by
the number of boxes in the building.

## Detection Logic

```
rht_on    = rht_vlv_cmd > reheat_open_threshold
satisfied = zone_temp > (zone_temp_sp_htg − zone_deadband)
        AND zone_temp < (zone_temp_sp_clg + zone_deadband)

yFault    = (rht_on AND satisfied) sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![VAV-0003 block graph](diagram.svg)

Both band bounds are computed as a positive gap against the deadband — `gapH` is
`zone_temp_sp_htg − zone_temp`, `gapC` is `zone_temp − zone_temp_sp_clg` — which
keeps `zone_deadband` a single positive parameter instead of a value added on
one side and subtracted on the other. Both bounds read that one value, so a host
retuning it must set both CXF paths together. At the shipped defaults and the
reference's 21/23 °C setpoints the satisfied band runs 20.5 to 23.5 °C,
exclusive at both ends (see Deviations): below it the zone is genuinely calling
for heat and the coil is doing its job, above it the zone is in cooling
territory and reheat there belongs to VAV-0006. The 30-minute `persist` delay
covers ordinary overshoot — a heating loop that carries the zone a little past
setpoint before releasing the coil shows this signature for a few minutes on
every recovery cycle, and that is control, not a fault. `delayOnInit = true`
holds the window across a restart.

## Possible Diagnoses

1. Reheat valve stuck or leaking — the actuator is not where the command says,
   or the seat passes flow at closed (hands off to the `stuck-actuator` playbook)
2. Control sequence not releasing reheat when the zone is satisfied: the loop or
   the mode logic keeps a heating output alive past its drop-out point
3. Incorrect valve stroke calibration — the actuator honors the command but the
   linkage maps 0% to a partly open plug

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT. The reference gives 5–15%
of zone thermal energy, under PNNL-25985's EEM-15 and EEM-38 (minimum flow
reduction and eliminating simultaneous heating and cooling). Estimation is
DIRECT because the valve command is the waste: with the zone inside its deadband
there is no useful heating output to subtract, so coil load scaled by command
position is the whole figure and the only inference left is the coil's rated
capacity. Heating-dominant, though the waste itself does not care about the
weather and in cooling season it compounds by loading the chiller as well.

## Emissions Impact

Scope 1, DIRECT_EMISSIONS, HIGH confidence; typically 300–2,000 kg CO₂e/yr per
zone. Scope 1 covers the usual case of a hydronic coil on a gas boiler. Sites
with electric reheat or a heat-pump-fed hot water loop should read this as scope
2 and rescale — the kilowatts are the same but the inventory is not.
Avoided-emissions basis: marginal operating emissions rate (MOER).

## Deviations

- **The reference's `>=` and `<=` band bounds become strict.** CDL `Reals` has
  no `GreaterEqual` or `LessEqual`, so both bounds are `LessThreshold` on a gap
  and a zone sitting exactly on either edge reads as outside the satisfied band
  where the reference reads it inside. The disagreement has measure zero on a
  real temperature signal and errs toward silence. A host binding coarsely
  quantized zone temperatures — integer °C, or a BAS that rounds to 0.5 — should
  widen `zone_deadband` slightly rather than rely on the signal landing off the
  boundary.
- `zone_deadband` is one card parameter bound to two CXF paths (`aboveHtg.t`,
  `belowClg.t`), matching the reference's single tunable. Hosts must set both
  together; a site wanting an asymmetric band retunes the paths individually and
  notes the divergence. Precedent: AHU-0025's `valve_open_threshold`.
- **Band bounds are expressed as gaps rather than shifted setpoints.**
  Implementing `zone_temp >= zone_temp_sp_htg − zone_deadband` literally would
  need a constant subtracted from the setpoint and a two-input comparison per
  bound; the gap form gets the same answer with one `Subtract` and one threshold
  per bound and keeps `zone_deadband` a single positive `set_param` path.
  Algebraically identical apart from the strictness above.
- `AlarmDelay = 30 min` becomes `persist.delayTime = 1800 s` with
  `delayOnInit = true` (Modelica/CDL default is `false`), the library's standing
  choice against alarming on the first tick after a controller restart.
- The rule does not test whether the AHU is running, whether the zone is
  occupied, or whether the zone temperature sensor is trustworthy; all three are
  frontmatter preconditions for host enforcement. The sensor one bites hardest —
  a zone sensor reading 2 °C high puts a genuinely cold space inside the
  satisfied band with the coil correctly responding, and this rule calls that a
  fault.
- `g36: null`, and no G36 clause appears in `source`. G36 sequences terminal-unit
  reheat, but the reference derives this logic from the PNNL retuning work, and
  SCHEMA.md reserves the `g36` field for the 001–049 range regardless.
- Severity 3 (warning) is the reference's chapter 10 value, kept despite the
  CRITICAL_WASTE category: the category describes the character of the waste, the
  severity the response urgency, and a leaking reheat valve on one box is not an
  emergency. The reference's §5.8.2 index carries no severity column.

## Notes

The remote discriminator is the [vav-min-flow-reheat](../../../playbooks/vav-min-flow-reheat.md)
playbook's step 2.2: command the valve to 0% and watch the zone. Temperature
still climbing means the valve is not where it says it is (diagnosis 1, hand off
to `stuck-actuator`); temperature responding and reheat returning once the
override is released means the sequence is the problem (diagnosis 2, a
programming fix); a valve that responds but never fully closes is diagnosis 3.

Within CLU-05 this rule is a member and VAV-0006 is the trigger. Where both
fire on one box, fix the trigger first — reheat during cooling season usually
traces to an oversized minimum or a too-low supply air temperature, and
correcting either can quiet this rule without touching the valve. All three
siblings watch the same valve: VAV-0001 asks whether the box is configured to
force reheat, VAV-0006 whether reheat runs while the building is trying to
cool, and this rule whether reheat runs when the zone wanted nothing at all.
