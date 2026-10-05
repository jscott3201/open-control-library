---
schema: cxf-library/fault-card/v1
id: CHW-0008
name: Chiller proof-of-operation failure
equipment: chw
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "EPA Facilities Manual, Volume 2, ch.9 Table 9-2 — chiller BAS monitoring includes per-equipment start/stop and failure"
  - "Library proof-of-operation precedents PMP-0003 and HW-0009 — independent command/status mismatch lanes with separate initialization-safe timers"
  - "Library-authored chiller timing adaptation; no cited source publishes 300 s start proof and 120 s stop proof as portable limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [CHW-0007, CHW-0009]
playbooks: [chiller-efficiency]
operating_states: "all states in which the final individual-machine stage command and independent chiller run proof are authoritative"
preconditions: "Bind chiller_cmd to the final per-machine request issued downstream of plant enable, lead/lag selection, normal anti-recycle logic, and applicable BAS interlocks; a plant enable or cooling demand cannot say which machine was requested. Bind chiller_status to independent proof that this chiller is actually producing cooling or operating a compressor, not availability, alarm-free, enable-ready, an echoed command, or a fleet OR. For modular/multi-circuit equipment, define the rule instance boundary consistently. Both points must be fresh and time-aligned, and each proof time must exceed worst-case delivery latency plus the normal sequence for that direction. Exclude approved anti-recycle lockout, pump/valve pre-run and post-run, oil-system preparation, shutdown unloading/coast-down, demand response, emergency or manufacturer safety actions, manual/local operation, maintenance, and functional tests. When ownership, authority, or mode is unknown the host reports NO_EVAL."
points:
  - chiller_cmd
  - chiller_status
outputs:
  - name: yFault
    description: True while either independent command/status mismatch has matured through its direction-specific proof timer
  - name: yFailToStart
    description: True after a final per-chiller start command remains unproved for start_proof_time
  - name: yUnexpectedRun
    description: True after proven operation continues without a final per-chiller command for stop_proof_time
params:
  start_proof_time:
    default: 300.0
    unit: s
    description: "Allowed start/permissive sequence before failure-to-start. ADOPTED_TUNABLE: set longer than normal oil, valve, pump/flow, starter, and delivery latency but shorter than operator response."
    cxf: startProof.delayTime
  stop_proof_time:
    default: 120.0
    unit: s
    description: "Allowed unload/coast-down after final command removal. ADOPTED_TUNABLE and independent of the start window."
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: Chiller, compressor/starter or drive, chilled-water delivery, and plant staging
  savings_range: "Direction-dependent: fail-to-start is primarily availability/protection; unexpected run can consume the chiller and associated plant power for the full mismatch interval"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "For yUnexpectedRun only, host proxy = mismatch hours × measured or defensible operating kW for the same chiller and auxiliaries; yFailToStart has no in-rule energy estimate"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:aae864b287b6e0c975e77111d088cfd6"
  date: 2026-10-05
---

## Description

This rule asks whether an individual chiller did what its final stage command
requested. Commanded but unproved operation can indicate a failed permissive,
locked-out machine, starter/drive fault, missing flow, or bad status. Proven
operation after command removal can indicate manual/local control, welded
hardware, a second controller, or a command bound upstream of the real owner.

## Detection Logic

```
fail_to_start  = chiller_cmd AND NOT chiller_status
unexpected_run = NOT chiller_cmd AND chiller_status

yFailToStart   = fail_to_start sustained for start_proof_time
yUnexpectedRun = unexpected_run sustained for stop_proof_time
yFault         = yFailToStart OR yUnexpectedRun
```

![CHW-0008 block graph](diagram.svg)

The two conditions are structurally mutually exclusive and each has its own
`TrueDelay(delayOnInit=true)`. A mismatch that reverses direction therefore
clears the old flag and serves the new direction's complete timer; time never
accumulates across lanes.

## Possible Diagnoses

`yFailToStart`:

1. Chilled/condenser-water flow, valve, or pump permissive not established
2. Active anti-recycle, oil-system, freeze, lift, current, or safety lockout
3. Starter, VFD, compressor, control transformer, or disconnect failure
4. Final stage command landed on the wrong machine
5. Run-proof sensor, integration, or point freshness failure

`yUnexpectedRun`:

6. Local/manual mode, service override, or second plant controller
7. Welded contactor or command output stuck active
8. Command bound upstream of chiller-internal logic
9. Normal unload/coast-down longer than the configured stop window

## Energy Impact

PROTECTIVE with a direction-dependent proxy. Failure to start threatens cooling,
humidity control, and low-flow/freeze protection but is not excess chiller kW.
Unexpected operation can be sized host-side from the same machine's measured
power and mismatch duration; the graph itself reads no power.

## Emissions Impact

Scope 2, proxy-only for unexpected run. Do not assign avoided electricity to
the fail-to-start direction.

## Deviations

- **The timers are adopted, not source-transcribed.** Chiller permissive and
  coast-down sequences vary materially; 300/120 s are commissioning starts.
- **No rule-wide suppression targets CHW-0007.** Fail-to-start makes tracking
  non-evaluable, but unexpected run can still be loaded and meaningfully fail
  tracking. Current metadata cannot express one directional suppression safely.
- **The final command is downstream of ordinary anti-recycle behavior.** If a
  BAS request is bound upstream, correct machine protection looks like failure.
- **`delayOnInit=true` on both lanes** prevents immediate alarms when commands
  are re-driven and statuses repopulate after a runtime restart.
- **One card severity/category cannot describe both directions perfectly.**
  Severity 2 PROTECTIVE follows failure-to-start; hosts may route unexpected run
  as a lower urgency energy/override finding using the direction output.
- **No EnergyPlus validation is claimed.** Part-load ratio or power can proxy
  status, but the current model exposes no independent final per-chiller BAS
  stage command; fabricating it from status would make proof tautological.
- **CLU-06 is unchanged.** A command/proof disagreement is diagnosis ordering,
  not a member of the existing efficiency-triggered cluster.
