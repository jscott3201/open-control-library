---
schema: cxf-library/fault-card/v1
id: HP-0007
name: Heat-pump compressor proof-of-operation failure
equipment: hp
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Library proof-of-operation precedents AHU-0039, TOWER-0004, PMP-0003, and HW-0009 — final command versus independent status, separate direction timers, and initialization-safe persistence"
  - "ASHRAE Guideline 36-2021 section 5.1.6 — equipment is proven when its digital status matches the state set by its digital command; semantic grounding only, not a source for these shipped timer values"
  - "Library-authored heat-pump compressor timing adaptation; no cited source publishes 300 s start and 120 s stop proof limits as portable values"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HP-0001, HP-0002, HP-0003, HP-0004, HP-0005, HP-0006, RTU-0001, HP-0008]
playbooks: [proof-of-operation, heat-pump-faults]
operating_states: "all states in which a final per-compressor command and independent proof for that same compressor or circuit are authoritative"
preconditions: "Bind comp_cmd downstream of anti-short-cycle delay, safety lockouts, staging, demand response, and OEM permissives; an upstream heating/cooling demand is not a final compressor command. Bind comp_status to actual compressor electrical, inverter, pressure, or auxiliary proof, not demand or command echo. Instantiate per compressor/circuit where possible. An OR command/status pair proves only that some compressor runs and cannot detect a failed lag compressor while the lead remains on. Exclude defrost transitions, pump-down, crankcase or oil-management sequences, emergency heat, service, and manufacturer restart delays not represented in the final command. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points:
  - comp_cmd
  - comp_status
outputs:
  - name: yFault
    description: True while either final command/status mismatch has matured through its own proof timer
  - name: yFailToStart
    description: Diagnostic direction flag; true after a final start command remains without independent run proof for start_proof_time. False never means NO_EVAL
  - name: yUnexpectedRun
    description: Diagnostic direction flag; true after independent operation continues without a final run command for stop_proof_time. False never means NO_EVAL
params:
  start_proof_time:
    default: 300.0
    unit: s
    description: "ADOPTED_TUNABLE final-command-to-proof allowance. Commission above normal device response, acceleration, proof pickup, and telemetry latency."
    cxf: startProof.delayTime
  stop_proof_time:
    default: 120.0
    unit: s
    description: "ADOPTED_TUNABLE command-off-to-proof-dropout allowance. Commission above normal deceleration, coast-down, proof dropout, and telemetry latency; intentional sequence operation must remain represented in the final command."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: Heat-pump compressor circuit, delivered heating/cooling, and refrigerant/performance diagnostics
  savings_range: "Direction-dependent: unexpected compressor operation can waste substantial electrical energy; fail-to-start is primarily lost capacity and may shift load to auxiliary heat"
  climate_sensitivity: both
  runtime_estimation: "For yUnexpectedRun only, host proxy = measured compressor kW x mismatch hours. Auxiliary-heat or unmet-load effects during yFailToStart require separate points and models."
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:3666ba36035e5c81277de17f12f77ae3"
  date: 2026-10-05
---

## Description

This rule checks whether the heat-pump compressor did what its final Boolean command
requested. Commanded on without independent proof is a fail-to-start; proven on
without command is unexpected operation. The direction identifies the mismatch,
not its cause, and neither diagnostic output is an evaluability gate.

## Detection Logic

```
fail_to_start  = comp_cmd AND NOT comp_status
unexpected_run = NOT comp_cmd AND comp_status

yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![HP-0007 block graph](diagram.svg)

Each direction has its own `TrueDelay(delayOnInit=true)`. Agreement clears both
lanes immediately. A direct mismatch reversal clears the old diagnostic and
starts the other timer from zero; elapsed time never transfers between lanes.

## Possible Diagnoses

1. Compressor, contactor, inverter, capacitor, disconnect, or power failure.
2. High/low-pressure, temperature, current, oil, or OEM safety lockout.
3. Final command incorrectly bound upstream of anti-cycle or permissive logic.
4. Bad compressor proof, command/status wiring, or multi-compressor aggregation.
5. OEM defrost, pump-down, protection sequence, or local service control.

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
  universal heat-pump compressor proof windows. Configure them independently around
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
- **The 300 s start allowance is an adopted commissioning placeholder for
  final-command-to-proof latency.** It is not permission to bind upstream demand;
  ordinary anti-short-cycle timing belongs before comp_cmd.
- **Fleet OR aggregation has a documented blind spot: a lag compressor can fail while
  the lead keeps both OR signals true.** Per-compressor instances are required whenever
  telemetry permits.
- **A brief compressor-off interval during defrost is raw fail-to-start if the final
  command stays true.** The host excludes the transition unless the OEM final command
  already represents it.

## Notes

Use yFailToStart to question the running premise of HP-0001 through HP-0006 and the
compressor-status premise of related RTU-0001. yUnexpectedRun may leave those
measurements meaningful, so no whole-rule suppression is encoded.
