---
schema: cxf-library/fault-card/v1
id: TOWER-0004
name: Tower fan proof-of-operation failure
equipment: tower
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "Library proof-of-operation precedents CHW-0008, PMP-0003, and HW-0009 — independent command/status mismatch lanes with initialization-safe direction-specific timers"
  - "EnergyPlus Engineering Reference, Cooling Towers and Evaporative Fluid Coolers — variable-speed tower fan operation follows leaving-water control and free-convection logic; control semantics only, no portable proof timer"
  - "Library-authored tower timing adaptation; no cited source publishes 120 s start and stop proof limits as portable values"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [TOWER-0001, TOWER-0003, TOWER-0005]
playbooks: [cooling-tower-performance]
operating_states: "all states in which the final individual fan/cell command and an independent proof of that same fan's operation are authoritative"
preconditions: "Bind tower_fan_cmd after cell staging, free-convection logic, minimum on/off timing, vibration lockout, and ordinary safeties; a plant tower-enable request is not a per-fan final command. Bind tower_fan_status to independent electrical, airflow, rotation, or auxiliary-contact proof for the same fan, never to an echoed command or an OR of the tower fleet. Evaluate one rule instance per independently commanded fan/cell. Normalize multi-speed stages to Boolean only when any-commanded-stage and any-proven-stage preserve real transitions. Inputs must be fresh and time-aligned, and both timers must exceed normal delivery latency and the real mechanical sequence. Exclude maintenance, local/manual operation, exercise tests, approved coast-down, vibration or OEM safety actions, and any state in which another controller owns the fan; otherwise report NO_EVAL."
points:
  - tower_fan_cmd
  - tower_fan_status
outputs:
  - name: yFault
    description: True while either independent command/status mismatch has matured through its own proof timer
  - name: yFailToStart
    description: True after a final per-fan start command remains unproved for start_proof_time
  - name: yUnexpectedRun
    description: True after independent fan operation continues without a final per-fan command for stop_proof_time
params:
  start_proof_time:
    default: 120.0
    unit: s
    description: "Allowed fan start and proof sequence. ADOPTED_TUNABLE: exceed normal VFD/starter acceleration, proof pickup, and telemetry latency."
    cxf: startProof.delayTime
  stop_proof_time:
    default: 120.0
    unit: s
    description: "Allowed fan ramp-down, coast-down, and proof dropout. ADOPTED_TUNABLE and independent of the start window."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: Cooling-tower fan, condenser heat rejection, and downstream chiller lift
  savings_range: "Direction-dependent: failure to start is chiefly availability/protection and can raise condenser temperature; unexpected run can waste measured fan power for the mismatch interval"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "For yUnexpectedRun only, host upper-bound proxy = same fan's measured kW × mismatch hours. Do not assign avoided fan energy to yFailToStart; any chiller-lift effect needs separate plant data."
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:8cea8bc88b1dfcc84aa0e09731947555"
  date: 2026-10-05
---

## Description

This rule checks whether one cooling-tower fan did what its final command
requested. A commanded fan without independent proof can mean a failed drive,
starter, belt, motor, interlock, or proof point. A proven fan after its command
has gone away can mean local control, a stuck output or contactor, a second
controller, or a command bound upstream of the true owner.

## Detection Logic

```
fail_to_start  = tower_fan_cmd AND NOT tower_fan_status
unexpected_run = NOT tower_fan_cmd AND tower_fan_status

yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![TOWER-0004 block graph](diagram.svg)

Each mismatch has a separate `TrueDelay(delayOnInit=true)`. A direct direction
reversal therefore clears the old diagnostic immediately and starts the other
timer from zero. Time never survives a healthy sample or transfers between
directions.

## Possible Diagnoses

`yFailToStart`:

1. VFD/starter trip, disconnect, overload, failed motor, belt, gearbox, or fan
2. Vibration, freeze, low-water, fire, or OEM safety interlock is active
3. Final cell-stage command was mapped to the wrong fan
4. Proof switch, airflow/rotation sensor, auxiliary contact, or integration is bad
5. Command point is upstream of free-convection, anti-cycle, or safety logic

`yUnexpectedRun`:

6. Local/manual mode, service override, or a second tower controller
7. Welded contactor, stuck output, or VFD run command held internally
8. Normal deceleration/coast-down exceeds the configured stop window
9. Fleet status was incorrectly bound to an individual fan instance

## Energy Impact

PROTECTIVE, direction-dependent. Failure to start threatens heat rejection and
can raise chiller lift, but its energy effect cannot be calculated from these
two Boolean points. Unexpected operation can waste the same fan's measured kW;
use that only as an upper bound because some rejected heat may still be useful.

## Emissions Impact

Scope 2, proxy-only for unexpected fan operation. Do not claim avoided energy
or emissions for a failed start without a separate condenser-plant model.

## Deviations

- **Both 120 s timers are adopted commissioning values.** No cited source
  establishes a universal tower-fan proof window. Configure them independently
  around actual acceleration, deceleration, proof pickup, and network latency.
- **The command is downstream of normal tower logic.** A plant enable or
  upstream leaving-water request is intentionally rejected because free
  convection, cell staging, minimum timers, and safeties can all keep an
  individual fan off correctly.
- **No rule-wide suppression targets TOWER-0001.** Failure to start can make
  approach-high non-evaluable, but unexpected operation can leave approach
  physically meaningful. Current metadata cannot suppress only one direction.
- **`delayOnInit=true` is explicit on both lanes.** A runtime restart does not
  turn an existing command/status disagreement into an immediate alarm.
- **EnergyPlus validation is not claimed.** Fan power or airflow ratio can
  establish status, but the current fixture has no independent final BAS fan
  command. Deriving command from status would make proof tautological.

## Notes

Dispatch from the direction output, not `yFault` alone. Confirm authority and
point identity remotely before sending a technician: a correct fan paired with
the wrong cell's command produces a perfectly repeatable false diagnosis.
