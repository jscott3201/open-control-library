---
schema: cxf-library/fault-card/v1
id: FPB-0001
name: Terminal fan proof-of-operation failure
equipment: fpb
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "Library proof-of-operation precedents AHU-0039, TOWER-0004, PMP-0003, HW-0009, and FCU-0006 — final command versus independent status with separate direction timers"
  - "ASHRAE Guideline 36-2021 section 5.1.6 — equipment proof semantics only; no portable terminal-fan timer values"
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — documented PFPU/SFPU topologies and future validation source, not threshold evidence"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FPB-0002, FPB-0003, FPB-0004]
playbooks: [proof-of-operation, fan-powered-terminal-faults]
operating_states: "series or parallel fan-powered terminal states in which the final terminal-fan command and same-fan independent proof are authoritative"
preconditions: "Bind fan_cmd after subtype sequence ownership, fan delay, occupancy, low-flow/heating logic, and normal interlocks. Bind independent fan_status for the same fan; command echo is invalid. A series fan may run continuously while occupied, whereas a parallel fan may legitimately remain off in many modes. Exclude smoke/emergency, freeze/condensate protection, exercise, maintenance, local hand operation, and any intentional fan delay not represented in the final command. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points: [fan_cmd, fan_status]
outputs:
  - name: yFault
    description: True while either terminal-fan command/proof mismatch has matured through its own timer
  - name: yFailToStart
    description: Diagnostic direction flag; true after a final start command remains without independent proof for start_proof_time. False never means NO_EVAL
  - name: yUnexpectedRun
    description: Diagnostic direction flag; true after independent operation continues without a final run command for stop_proof_time. False never means NO_EVAL
params:
  start_proof_time:
    default: 60.0
    unit: s
    description: "ADOPTED_TUNABLE final-command-to-proof allowance; commission above normal fan response, proof pickup, and telemetry latency."
    cxf: startProof.delayTime
  stop_proof_time:
    default: 60.0
    unit: s
    description: "ADOPTED_TUNABLE command-off-to-proof-dropout allowance; commission above coast-down, proof dropout, and telemetry latency."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: terminal fan, zone airflow, and fan-dependent FPB diagnostics
  savings_range: "Direction-dependent; unexpected operation wastes fan electricity, while fail-to-start is primarily delivery and coverage loss"
  climate_sensitivity: both
  runtime_estimation: "For yUnexpectedRun only, measured fan kW times mismatch hours is a proxy. Do not assign avoided fan energy to yFailToStart."
emissions: {scope: "2", method: PROXY_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:b7e84c2382c4a0204115bd7efa28032d", date: 2026-10-05}
---

## Description

This rule checks whether the fan inside a series or parallel fan-powered
terminal did what its final Boolean command requested. Direction identifies the
observed mismatch, not its cause or the subtype's expected operating schedule.

## Detection Logic

```
fail_to_start  = fan_cmd AND NOT fan_status
unexpected_run = NOT fan_cmd AND fan_status
yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![FPB-0001 block graph](diagram.svg)

Independent `TrueDelay(delayOnInit=true)` lanes prevent elapsed time from
crossing a mismatch-direction reversal. Agreement clears both lanes immediately.

## Possible Diagnoses

1. Failed motor, ECM, contactor, controller output, breaker, or wiring.
2. Seized wheel, failed bearing, belt, coupling, or fan relay.
3. Bad current, airflow, pressure, speed, rotation, or auxiliary-contact proof.
4. Local hand/thermostat ownership or a subtype-specific interlock omitted from command.
5. Wrong terminal or upstream AHU fan bound to either canonical point.

## Energy Impact

Unexpected operation can waste terminal-fan electricity and alter delivered
heating or airflow. Fail-to-start is principally a comfort and availability
finding; this Boolean pair cannot price its zone or plant effect.

## Emissions Impact

Scope 2 proxy applies only to unexpected operation using measured fan kW,
mismatch hours, and the applicable electricity factor.

## Deviations

- **Both 60 s timers are adopted.** No cited source establishes portable FPB proof windows.
- **Subtype schedule remains host-side.** The same graph is valid for a continuously running series fan and an intermittently commanded parallel fan because it judges agreement, not when command should be on.
- **Final command and independent proof are mandatory.** Upstream enable creates false positives; command echo creates a blind spot.
- **No whole-rule suppression is encoded.** Fail-to-start can remove another rule's premise while unexpected run can leave that rule physically meaningful.
- **No empirical FPR or TPR is claimed in this slice.** The LBNL adapter and exact dataset-to-canonical-point mappings are deferred to PR11.
