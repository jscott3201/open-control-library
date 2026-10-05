---
schema: cxf-library/fault-card/v1
id: FCU-0006
name: FCU fan proof-of-operation failure
equipment: fcu
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "Library proof-of-operation precedents AHU-0039, TOWER-0004, PMP-0003, and HW-0009 — final command versus independent status, separate direction timers, and initialization-safe persistence"
  - "ASHRAE Guideline 36-2021 section 5.1.6 — equipment is proven when its digital status matches the state set by its digital command; semantic grounding only, not a source for these shipped timer values"
  - "Library-authored FCU fan timing adaptation; no cited source publishes 60 s start and 60 s stop proof limits as portable values"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FCU-0001, FCU-0002, FCU-0003, FCU-0004, FCU-0005]
playbooks: [proof-of-operation, fcu-faults]
operating_states: "all states in which this FCU has an actively controlled fan and its final individual fan command and independent run proof are authoritative"
preconditions: "The FCU must have an actively commanded fan; passive/convection units are not applicable. Bind fan_cmd to the final fan output after occupancy, mode, condensate, freeze, and local interlock logic, not to unit enable or thermostat demand. Bind fan_status to independent motor-current, airflow, speed, rotation, or auxiliary-contact proof for the same fan, never to a command echo. Exclude fan coast-down, exercise, maintenance, condensate-alarm shutdown, freeze protection, and local hand operation unless represented in the final command. For ECM and multi-speed fans, Boolean normalization proves only operation and must not hide a failed requested speed stage. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points:
  - fan_cmd
  - fan_status
outputs:
  - name: yFault
    description: True while either final command/status mismatch has matured through its own proof timer
  - name: yFailToStart
    description: Diagnostic direction flag; true after a final start command remains without independent run proof for start_proof_time. False never means NO_EVAL
  - name: yUnexpectedRun
    description: Diagnostic direction flag; true after independent operation continues without a final run command for stop_proof_time. False never means NO_EVAL
params:
  start_proof_time:
    default: 60.0
    unit: s
    description: "ADOPTED_TUNABLE final-command-to-proof allowance. Commission above normal device response, acceleration, proof pickup, and telemetry latency."
    cxf: startProof.delayTime
  stop_proof_time:
    default: 60.0
    unit: s
    description: "ADOPTED_TUNABLE command-off-to-proof-dropout allowance. Commission above normal deceleration, coast-down, proof dropout, and telemetry latency; intentional sequence operation must remain represented in the final command."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: FCU fan motor, served zone airflow, and coil rules that require fan operation
  savings_range: "Direction-dependent: unexpected operation can waste fan electricity and unwanted heating/cooling; fail-to-start is primarily delivery and diagnostic-coverage loss"
  climate_sensitivity: both
  runtime_estimation: "For yUnexpectedRun only, host proxy = measured fan kW x mismatch hours. Do not assign avoided fan energy to yFailToStart; any comfort or coil penalty needs separate data."
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:8c4af7be56126b213921f77982c3f3e7"
  date: 2026-10-05
---

## Description

This rule checks whether the FCU fan did what its final Boolean command
requested. Commanded on without independent proof is a fail-to-start; proven on
without command is unexpected operation. The direction identifies the mismatch,
not its cause, and neither diagnostic output is an evaluability gate.

## Detection Logic

```
fail_to_start  = fan_cmd AND NOT fan_status
unexpected_run = NOT fan_cmd AND fan_status

yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![FCU-0006 block graph](diagram.svg)

Each direction has its own `TrueDelay(delayOnInit=true)`. Agreement clears both
lanes immediately. A direct mismatch reversal clears the old diagnostic and
starts the other timer from zero; elapsed time never transfers between lanes.

## Possible Diagnoses

1. Failed motor, ECM, controller, breaker, contactor, or output wiring.
2. Seized fan wheel, failed belt, coupling, or bearing.
3. Condensate, freeze, or local safety interlock omitted from command.
4. Bad current, airflow, speed, rotation, or auxiliary-contact proof.
5. Local thermostat, hand switch, or independent controller owns the fan.

## Energy Impact

The effect is direction-dependent. Unexpected operation can waste measured
electrical energy during the mismatch. Fail-to-start is primarily availability,
comfort, and diagnostic-coverage loss; these two booleans cannot price it.

## Emissions Impact

Scope 2 is proxy-only for unexpected operation: multiply independently measured
device kW by mismatch hours and an appropriate operating emissions factor. Do
not claim avoided energy or emissions for fail-to-start without another model.

## Deviations

- **Both timers are adopted commissioning values.** No cited source establishes
  universal FCU fan proof windows. Configure them independently around
  the actual sequence, proof device, sampling, and network latency.
- **The command is final and device-scoped.** An upstream enable, demand, or
  fleet request can disagree with status while downstream logic works correctly.
- **Status is independent proof.** Command echo makes the graph tautological;
  proof type determines whether electrical operation, rotation, or delivery was
  actually demonstrated.
- **No whole-rule suppression is encoded.** Fail-to-start can invalidate another
  rule's running premise, but unexpected operation may leave that rule physically
  meaningful; current metadata cannot suppress by direction.
- **`delayOnInit=true` is explicit on both lanes.** Evaluator restart into an
  existing mismatch must serve the full configured proof time.
- **No empirical FPR or TPR is claimed.** Current simulation telemetry cannot
  provide both an independent final command and field-like proof for this device.
- **Passive fan coils are explicitly not applicable.** A host must not synthesize a
  command/status pair for a unit that moves air only by convection.
- **The Boolean pair proves fan operation, not the requested ECM or multi-speed stage.**
  A wrong-stage failure needs speed or stage feedback and is outside this graph.

## Notes

Read the direction before interpreting FCU-0002 through FCU-0005: fail-to-start removes
their airflow premise, while unexpected operation can leave their temperature signatures
physically meaningful.
