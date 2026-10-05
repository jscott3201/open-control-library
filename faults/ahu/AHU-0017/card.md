---
schema: cxf-library/fault-card/v1
id: AHU-0017
name: Economizer not operational when favorable
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0017"
  - "PNNL-27338 §3"
  - "PNNL EEM-06"
  - "Cowan 2004 (54% of RTUs)"
g36: null
clusters: [CLU-03]
suppresses: []
suppressed_by: []
related: [AHU-0009, AHU-0011, AHU-0034]
playbooks: [economizer-failure]
operating_states: "OS 4 (mechanical cooling)"
preconditions: "Supply fan running. The outdoor/return comparison must be evaluable: |oat - rat| >= TMIN (PNNL-27338 uses 5 °F for its outdoor-air-fraction work), since two sensors reading within their combined error of each other cannot establish which air is cooler. Hosts also gate on OAT sensor quality — a sensor reading high produces this fault's signature with the economizer control working correctly (diagnosis 4). When either gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - oat
  - rat
  - clg_vlv_cmd
  - oa_dmpr_cmd
outputs:
  - name: yFault
    description: True while outdoor conditions have favored economizing, mechanical cooling has run, and the OA damper has stayed below econ_damper_threshold, all continuously for at least alarm_delay
params:
  econ_type_is_ddb:
    default: true
    unit: bool
    description: Economizer changeover type — true = differential dry-bulb (compare oat to rat), false = fixed high-limit dry-bulb (compare oat to econ_hl_temp)
    cxf: isDDB.k
  econ_hl_temp:
    default: 21.0
    unit: "°C"
    description: Fixed high-limit changeover temperature, used only when econ_type_is_ddb is false
    cxf: hlConst.k
  temp_deadband:
    default: 1.0
    unit: "°C"
    description: Margin the favorable comparison must clear before economizing counts as worthwhile; binds both changeover branches
    cxf: [ddbFav.t, hlFav.t]
  cooling_enabled_threshold:
    default: 10.0
    unit: "%"
    description: Cooling valve command above which mechanical cooling counts as active
    cxf: clgOn.t
  econ_damper_threshold:
    default: 25.0
    unit: "%"
    description: OA damper command below which the damper counts as parked at minimum position
    cxf: dmprLow.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU mechanical cooling energy
  savings_range: 5-20% of cooling energy (PNNL-27338)
  climate_sensitivity: cooling-dominant
  runtime_estimation: "waste_kw = clg_vlv_cmd/100 × ahu_clg_capacity_kw (the mechanical cooling free cooling would have displaced)"
emissions:
  scope: "2"
  method: DIRECT_EMISSIONS
validation:
  - kind: simulation_tpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "TPR physics-level: FaultModel:TemperatureSensorOffset:OutdoorAir +/-4 degC patched into the epJSON (controller acts on the biased sensor; FDD replays true node values), B2B OfficeMedium-4004 July week, 3 loops; failures = missed detections"
    scenarios: 3
    failures: 0
    notes: "+4 degC controller offset (locks out economizing early): detected 3/3 loops, ~3-4 h on two; correctly silent at -4. Envelope rules stayed silent — physically consistent sensors — the exact complement of the input-bias campaign"
  - kind: simulation_tpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "TPR: +/-3 degC OAT bias injected into replay inputs (faulted-sensor-as-seen-by-FDD), B2B OfficeMedium-4004 July week, 3 VAV loops; failures = missed detections; baseline-confounded rules excluded from attribution"
    scenarios: 3
    failures: 0
    notes: "-3 degC direction: detected 3/3 loops within ~40 min of the first gated window; correctly silent at +3 (direction mirror of AHU-0034)"
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-18
    fleet: "B2B OfficeMedium x8 ASHRAE climate zones (1-8), one July + one January week, 3 VAV loops each, host-gated (fan + OS)"
    scenarios: 32
    failures: 0
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:6bd93b0a587698d63e6263031a029829"
  date: 2026-10-05
---

## Description

Outdoor air is cool enough to do the cooling for free, but the outdoor-air
damper sits at or near its minimum position while the cooling coil runs. Every
kilowatt the compressor or chiller spends in that state buys cooling the
economizer was standing by to provide at fan power alone. Common and cheap to
fix: Cowan's 2004 field survey found 54% of RTU economizers carrying at least
one fault, most often a disconnected damper linkage. Trigger rule for CLU-03
(Economizer Failure) — fix it first, since a damper pinned at minimum also
fails the mixed-air and OA-fraction tests in AHU-0009 and AHU-0011.

## Detection Logic

```
econ_favorable = (rat - oat)          > temp_deadband   when econ_type_is_ddb
               = (econ_hl_temp - oat) > temp_deadband   otherwise

yFault = econ_favorable
     AND clg_vlv_cmd > cooling_enabled_threshold
     AND oa_dmpr_cmd < econ_damper_threshold
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0017 block graph](diagram.svg)

Both changeover branches compute on every tick and `favSel` (`Logical.Switch`,
`y = u2 ? u1 : u3`) picks one: `isDDB` true (the default) selects the
differential branch (`rat - oat`), false the fixed high-limit branch
(`econ_hl_temp - oat`). Thresholding the difference rather than the raw
temperatures is what lets one `temp_deadband` serve both branches. All three
comparisons are strict, so a damper parked at exactly 25%, a cooling valve at
exactly 10%, or a gap of exactly 1.0 °C does not trip the rule. `persist`
requires 30 minutes of continuous violation — long enough to ride out damper
strokes and changeover transitions — and any interruption restarts the timer;
`delayOnInit = true` makes a violation already present at engine start wait out
the full window.

## Possible Diagnoses

1. Economizer control sequence disabled or misconfigured in the BAS
2. OA damper stuck at minimum position (linkage disconnected, blades bound)
3. Damper actuator failure — no power, no air, or a burnt-out motor
4. OAT sensor error, reading higher than actual, which locks out changeover
5. Economizer lockout active when it should not be (seasonal or manual)

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT. The waste is the
mechanical cooling that free cooling would have displaced, readable straight
off the valve command: `waste_kw = clg_vlv_cmd/100 × ahu_clg_capacity_kw`.
Correcting economizer operation saves 5–20% of cooling energy (PNNL-27338 §3;
PNNL EEM-06, OA damper faults and controls). Cooling-dominant, and worth most
in mild shoulder-season weather — exactly when the damper should be modulating.

## Emissions Impact

Scope 2, DIRECT_EMISSIONS, HIGH confidence; typical 1,500–8,000 kg CO₂e/yr. The
displaced energy is electric compressor or chiller work, so the entire impact
lands in purchased electricity. Free-cooling hours cluster in mild daytime and
overnight weather, when the marginal generator differs sharply from the annual
average — use the marginal operating emissions rate (MOER), not an average
grid factor.

## Deviations

- The reference's `econ_type` enum (`DDB` | `HL_DB`, default `DDB`) is carried
  here as the boolean `econ_type_is_ddb` driving a `Logical.Switch`. Two values
  do not earn an enum, and a boolean is retunable through `set_param` on a
  deployed rule; enthalpy changeover needs its own rule, not a third value.
- The evaluability gate `|oat - rat| >= TMIN` and the OS-4 (mechanical cooling)
  operating-state restriction are declared as preconditions for host
  enforcement rather than encoded in the block graph — gating and data quality
  stay out of the rule, which computes the fault condition given valid data.
- All three comparisons are strict (`>`, `>`, `<`); the reference does not
  specify boundary behavior, so the library's strict convention applies.
- `temp_deadband` is one card parameter bound to two CXF paths (`ddbFav.t`,
  `hlFav.t`), matching the reference's single deadband. Hosts must set both
  paths together, or flipping `econ_type_is_ddb` silently changes the deadband.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a violation already present at load waits out the full 30
  minutes instead of alarming on the first tick after a controller restart.

## Notes

With default parameters the shipped vectors exercise only the DDB branch —
`vectors/v1` stages inputs, not parameters, so `hlConst`, `hlGap`, and `hlFav`
are structurally verified but never reach `yFault` through `u3`. A host that
sets `econ_type_is_ddb = false` should commission that path itself.

`econ_hl_temp` defaults to 21 °C, near ASHRAE 90.1's 70 °F high limit for
climate zones 4A–5A. Zones 1A–3A allow 75 °F (23.9 °C) and zones 5B–8 use
65 °F (18.3 °C) — retune per the playbook rather than accepting the default in
a climate it does not fit.
