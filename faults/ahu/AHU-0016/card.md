---
schema: cxf-library/fault-card/v1
id: AHU-0016
name: Simultaneous heating and cooling
equipment: ahu
status: verified
phase: 1
method: rule
severity: 2
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0016"
  - "PNNL-27338 retuning measures (EEM-38)"
g36: null
clusters: [CLU-01]
suppresses: []
suppressed_by: []
related: [AHU-0004, AHU-0020, AHU-0025]
playbooks: [simultaneous-hc]
operating_states: all
preconditions: "Equipment is in occupied mode or supply fan is running. Host gates evaluation; when unmet, the verdict is NO_EVAL, not healthy."
points:
  - htg_vlv_cmd
  - clg_vlv_cmd
outputs:
  - name: yFault
    description: True while both valves have been open beyond their thresholds for at least alarm_delay
params:
  htg_vlv_threshold:
    default: 5.0
    unit: "%"
    description: Minimum heating valve position considered open
    cxf: htgThr.t
  clg_vlv_threshold:
    default: 5.0
    unit: "%"
    description: Minimum cooling valve position considered open
    cxf: clgThr.t
  alarm_delay:
    default: 900.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU heating + cooling energy
  savings_range: 10-30% of AHU thermal energy (PNNL-27338)
  climate_sensitivity: both
  runtime_estimation: "waste_kw = htg_vlv_cmd/100 × ahu_htg_capacity_kw + clg_vlv_cmd/100 × ahu_clg_capacity_kw"
emissions:
  scope: "1+2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:31aa24f4af35117444e0227b5a021b99"
  date: 2026-10-05
---

## Description

The heating coil valve and cooling coil valve are both commanded open beyond
their respective thresholds at the same time. The air stream is being heated
and then cooled (or vice versa) — energy is simultaneously added and removed
with no occupant benefit, so the entire overlap is waste. One of the most
common and highest-impact faults in commercial buildings; the trigger rule for
cluster CLU-01.

## Detection Logic

```
yFault = (htg_vlv_cmd > htg_vlv_threshold)
     AND (clg_vlv_cmd > clg_vlv_threshold)
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0016 block graph](diagram.svg)

`TrueDelay` implements the alarm persistence: `yFault` asserts only after the
combined condition has held continuously for `alarm_delay` seconds, and any
interruption restarts the timer. The comparison is strict (`>`), so a valve
sitting exactly at its threshold does not count as open. `persist` sets
`delayOnInit = true` so a condition already present at engine start still
waits out the full delay (engine-verified: the Modelica default of `false`
passes an initially-true input through immediately, which would fire the alarm
on the first tick after a controller restart).

## Possible Diagnoses

1. Control sequence error — heating and cooling loops fighting (missing
   interlock or overlapping deadbands)
2. Stuck heating valve (mechanically open)
3. Stuck cooling valve (mechanically open)
4. Incorrect valve wiring (normally-open vs normally-closed)
5. Poorly tuned PID loops with overlapping deadbands

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT. Waste is directly
computable from live points and design capacities:
`waste_kw = htg_vlv_cmd/100 × ahu_htg_capacity_kw + clg_vlv_cmd/100 ×
ahu_clg_capacity_kw`. Savings range 10–30% of AHU thermal energy (PNNL-27338,
EEM-38). Present in the majority of buildings; wastes energy in all seasons.

## Emissions Impact

Scope 1 + 2 (gas heating waste + electric cooling waste), DIRECT_EMISSIONS,
HIGH confidence. Typical range 2,000–15,000 kg CO₂e/yr. Avoided-emissions
basis: marginal operating emissions rate (MOER).

## Deviations

- The reference card's precondition ("equipment in occupied mode or fan
  running") is declared in frontmatter for host enforcement rather than encoded
  in the block graph. The CXF computes the pure fault condition; gating its
  evaluation (and the NO_EVAL verdict when data is bad or the unit is off) is
  the host's responsibility, matching the open-control engine's status-blind
  design.
- Threshold comparisons use zero hysteresis (`h = 0` on both
  `GreaterThreshold` blocks), like the reference. If command signals chatter
  around the threshold in practice, set `h` per site rather than raising `t`.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`). The
  reference is silent on startup behavior; we require the persistence window
  even when the condition pre-exists at load, to avoid instant alarms after
  controller restarts.

## Notes

Fix the trigger first: resolving this rule typically clears the CLU-01 member
faults (mode mismatch, lockout issues) within 24–48 hours. Remote fix succeeds
~70% of the time (interlock/deadband/override corrections) at $0 cost.
