---
schema: cxf-library/fault-card/v1
id: FCU-0007
name: Simultaneous heating and cooling commands
equipment: fcu
status: verified
phase: 2
method: rule
severity: 2
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 ch.12 and library AHU-0016 — ordinary simultaneous heating/cooling command overlap is an actionable control-waste signature"
  - "Library-authored FCU command-level adaptation; no cited source publishes the shipped 10% thresholds or 300 s duration as portable values"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FCU-0004, FCU-0005]
playbooks: [simultaneous-hc, fcu-faults]
operating_states: "normal automatic FCU heating or cooling when simultaneous cooling-plus-reheat is not an intended humidity-control sequence"
preconditions: "htg_vlv_cmd and clg_vlv_cmd must be physical 0-100% valve commands for heating and cooling coils on the same FCU, with known scaling and fail positions. Exclude valve exercise, freeze protection, commissioning tests, and intentional dehumidification/cooling-plus-reheat. If simultaneous conditioning is a designed mode, provide a host mode gate or do not instantiate this rule. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points:
  - htg_vlv_cmd
  - clg_vlv_cmd
outputs:
  - name: yFault
    description: "True after both same-FCU valve commands remain strictly above their active thresholds for sustained_duration"
  - name: yHeatingActive
    description: "Diagnostic sub-condition flag; true while the heating command is strictly above threshold. False never means NO_EVAL"
  - name: yCoolingActive
    description: "Diagnostic sub-condition flag; true while the cooling command is strictly above threshold. False never means NO_EVAL"
params:
  heating_active_threshold:
    default: 10.0
    unit: "%"
    description: "ADOPTED_TUNABLE material heating-command threshold; equality is clear. Commission above leakage, actuator minimum, and command noise."
    cxf: heatingActive.t
  cooling_active_threshold:
    default: 10.0
    unit: "%"
    description: "ADOPTED_TUNABLE material cooling-command threshold; equality is clear. Commission above leakage, actuator minimum, and command noise."
    cxf: coolingActive.t
  sustained_duration:
    default: 300.0
    unit: s
    description: "LIBRARY_PRECEDENT continuous overlap duration; verify against normal FCU mode transitions and sampling."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: FCU heating coil, cooling coil, serving plants, and fan
  savings_range: "Potentially material thermal waste while both commands overlap; not quantified from command alone"
  climate_sensitivity: both
  runtime_estimation: "QUALITATIVE_ONLY. Command overlap hours do not establish flow, delivered heat, or avoided plant energy; use valve feedback, coil temperatures, flow, and plant efficiency for savings."
emissions:
  scope: "1/2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:4b59bb37f9d6537178e60d22896cd2d6"
  date: 2026-10-05
---

## Description

This rule detects a same-FCU heating-valve command and cooling-valve command
materially open together. It reports a command conflict, not physical valve
position or coil heat transfer. FCU-0004/0005 remain useful for distinguishing
unintended thermal effect with a nominally closed command.

## Detection Logic

```
heating_active = htg_vlv_cmd > heating_active_threshold
cooling_active = clg_vlv_cmd > cooling_active_threshold

yHeatingActive = heating_active
yCoolingActive = cooling_active
yFault = (heating_active AND cooling_active) sustained for sustained_duration
```

![FCU-0007 block graph](diagram.svg)

Both comparisons are strict. `TrueDelay(delayOnInit=true)` starts only while
both subconditions are true; either command clearing resets the timer and a
mature alarm clears immediately.

## Possible Diagnoses

1. Heating and cooling PID loops overlap or lack an interlock
2. Occupancy, mode, or setpoint transition leaves both outputs active
3. BAS priority-array override holds one command open
4. Wrong point scaling or commands bound from different FCUs
5. Intentional dehumidification/reheat, freeze, or exercise mode not host-gated

## Energy Impact

Simultaneous commands can add and remove heat at the same terminal, increasing
plant and fan energy. Commands alone do not prove valve position, water flow,
or thermal transfer, so this card makes no portable savings claim.

## Emissions Impact

Scope 1 and/or 2 effects depend on the serving heating and cooling plants.
Quantify only after measuring delivered heat and avoided plant input energy.

## Deviations

- **Thresholds and duration are library defaults.** The source supports the
  signature, not universal 10% or 300 s values; commission all three.
- **Intentional cooling-plus-reheat is host-excluded.** The graph has no mode
  input and intentionally alarms on that raw overlap when the host gate is absent.
- **CLU-01 is unchanged.** Its AHU-0016 trigger cannot causally clear a local
  FCU command conflict under the cluster contract; shared workflow is represented
  by the simultaneous-hc playbook instead.
- **Leak rules are not suppressed.** A command conflict and physical heat transfer
  with a closed command are different evidence and can co-occur.
- **No empirical FPR or TPR is claimed.** No current harness exposes both physical
  same-FCU valve commands without inference; validation is synthetic only.
