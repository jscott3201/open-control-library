---
schema: cxf-library/fault-card/v1
id: RTU-0010
name: RTU supply-fan proof-of-operation failure
equipment: rtu
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
  - "Library-authored RTU supply fan timing adaptation; no cited source publishes 60 s start and 120 s stop proof limits as portable values"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [RTU-0001, RTU-0002, RTU-0003, RTU-0004, RTU-0005, RTU-0006, RTU-0008, RTU-0009, RTU-0011]
playbooks: [proof-of-operation]
operating_states: "all states in which the final RTU supply-fan command and independent proof for that same fan are authoritative"
preconditions: "Bind sf_cmd to the final supply-fan command after smoke, freeze, heat-exchanger, post-heat fan-delay, and safety logic; unit enable is valid only when it demonstrably is that final command. A commanded post-heat run must keep sf_cmd true. Bind sf_status to independent electrical, airflow, speed, rotation, or auxiliary-contact proof for the same fan. Exclude purge, smoke control, ventilation override, service, local hand mode, and any fan-delay state omitted from sf_cmd. The 120 s stop timer must exceed normal mechanical coast and proof dropout, not conceal an upstream command. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points:
  - sf_cmd
  - sf_status
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
    default: 120.0
    unit: s
    description: "ADOPTED_TUNABLE command-off-to-proof-dropout allowance. Commission above normal deceleration, coast-down, proof dropout, and telemetry latency; intentional sequence operation must remain represented in the final command."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: RTU supply fan, ventilation, and every air-side/capacity rule that assumes supply airflow
  savings_range: "Direction-dependent: unexpected operation can waste fan and conditioning energy; fail-to-start chiefly threatens ventilation, capacity, comfort, and downstream inference"
  climate_sensitivity: both
  runtime_estimation: "For yUnexpectedRun only, host proxy = measured fan kW x mismatch hours plus separately modeled conditioning. Boolean proof cannot price yFailToStart."
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:9ce62aa58b24bd636ac88bdc9f0b6504"
  date: 2026-10-05
---

## Description

This rule checks whether the RTU supply fan did what its final Boolean command
requested. Commanded on without independent proof is a fail-to-start; proven on
without command is unexpected operation. The direction identifies the mismatch,
not its cause, and neither diagnostic output is an evaluability gate.

## Detection Logic

```
fail_to_start  = sf_cmd AND NOT sf_status
unexpected_run = NOT sf_cmd AND sf_status

yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![RTU-0010 block graph](diagram.svg)

Each direction has its own `TrueDelay(delayOnInit=true)`. Agreement clears both
lanes immediately. A direct mismatch reversal clears the old diagnostic and
starts the other timer from zero; elapsed time never transfers between lanes.

## Possible Diagnoses

1. Motor, contactor, belt, fan wheel, VFD, overload, or disconnect failure.
2. Smoke, freeze, high-static, heat-exchanger, or OEM safety interlock.
3. Failed or misconfigured current, airflow, speed, or auxiliary proof.
4. Post-heat fan delay or purge omitted from the final command binding.
5. Unauthorized local/manual operation, welded contactor, or second owner.

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
  universal RTU supply fan proof windows. Configure them independently around
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
- **The stop allowance is 120 s, twice the 60 s start allowance, to accommodate
  mechanical coast and proof dropout.** A controlled post-heat run must keep the final
  sf_cmd true; the timer does not legalize an upstream binding. Both values remain
  site-tuned placeholders, not portable source values.
- **A purge or smoke-control run is raw unexpected operation unless it is included in
  sf_cmd.** The host excludes that state; the graph does not encode an upstream mode
  gate.

## Notes

Fail-to-start contests the running or airflow premise of RTU-0001 through RTU-0006 and
can distort RTU-0008/0009 refrigerant evidence. Unexpected run does not invalidate those
rules automatically, so the relationship remains informational rather than a whole-rule
suppression.
