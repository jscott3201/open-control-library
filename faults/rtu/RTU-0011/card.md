---
schema: cxf-library/fault-card/v1
id: RTU-0011
name: RTU supply-air temperature tracking failure
equipment: rtu
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "Veronica, Automatically Detecting Faulty Regulation in HVAC Controls, HVAC&R Research 19(4), 2013, pp.412-422, DOI 10.1080/10789669.2013.789369 — regulated variables are checked against user allowance bands; semantic evidence, not this graph's thresholds"
  - "Library CHW-0007 and HW-0010 — strict mirrored tracking-error topology with operating-premise gating and persistent alarm"
  - "Library-authored packaged-DX adaptation; no cited source publishes the shipped 2 K and 900 s combination as portable"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [RTU-0001, RTU-0002, RTU-0003, RTU-0004, RTU-0007, RTU-0008, RTU-0009, RTU-0010]
playbooks: [rtu-compressor-refrigerant]
operating_states: "normal automatic RTU heating or cooling after airflow and active mechanical delivery are established and the active SAT target has settled"
preconditions: "sat and sat_sp must be the same RTU's discharge temperature and final active mode-specific target, not an occupied default or cooling-only constant during heating. sf_status must prove stable supply airflow; comp_status or htg_status must represent active mechanical conditioning. Exclude startup, defrost, post-heat fan delay, mode/setpoint changes, demand response, low-ambient protection, and OEM capacity or safety limits. Suspend or smooth evaluation across normal staged/cycling-DX off intervals. Points must be healthy, fresh, aligned, and in degC. yConditioningActive false means NO_EVAL; true does not prove fan status or the other host obligations."
points:
  - sat
  - sat_sp
  - sf_status
  - comp_status
  - htg_status
outputs:
  - name: yFault
    description: "True after proven fan operation and mechanical conditioning remain outside either side of the active SAT band for sustained_duration"
  - name: yTooWarm
    description: "Diagnostic direction flag; true while evaluable SAT is strictly more than tracking_error above setpoint. False never means NO_EVAL"
  - name: yTooCold
    description: "Diagnostic direction flag; true while evaluable SAT is strictly more than tracking_error below setpoint. False never means NO_EVAL"
  - name: yConditioningActive
    description: "Evaluability subcondition; true when compressor or heating proof is active. False means NO_EVAL; true does not establish every host precondition"
params:
  tracking_error:
    default: 2.0
    unit: K
    description: "ADOPTED_TUNABLE symmetric settled SAT allowance; equality is clear. Commission above combined sensor uncertainty, cycling ripple, and normal deadband."
    cxf: [tooWarmRaw.t, tooColdRaw.t]
  sustained_duration:
    default: 900.0
    unit: s
    description: "LIBRARY_PRECEDENT continuous out-of-band duration; commission above normal mode, staging, and setpoint-settling dynamics."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: RTU compressor, heating section, supply fan, and served zones
  savings_range: "Site-dependent; tracking failure can increase runtime or miss comfort and humidity targets"
  climate_sensitivity: both
  runtime_estimation: "QUALITATIVE_ONLY. Temperature error-hours do not determine excess energy without load, airflow, power, cycling, and a causal baseline."
emissions:
  scope: "1/2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:0ae66df187840b9fde91f9796ee3082a"
  date: 2026-10-05
---

## Description

This rule reports an RTU whose discharge temperature remains materially
outside its final active target while the supply fan and mechanical heating
or cooling are proven active. Warm/cold outputs identify the observed direction,
not the failed component or commanded mode.

## Detection Logic

```
error               = sat - sat_sp
conditioning_active = comp_status OR htg_status
running_conditioning = sf_status AND conditioning_active
too_warm            = error > tracking_error
too_cold            = -error > tracking_error

yConditioningActive = conditioning_active
yTooWarm = running_conditioning AND too_warm
yTooCold = running_conditioning AND too_cold
yFault = running_conditioning AND (too_warm OR too_cold),
         sustained for sustained_duration
```

![RTU-0011 block graph](diagram.svg)

Comparisons are strict and direction flags are immediate. One
`TrueDelay(delayOnInit=true)` follows the warm/cold OR, so a direct sampled
direction handoff preserves age while any in-band or non-running tick resets it.

## Possible Diagnoses

1. Active setpoint misbound, stale, overridden, or not delivered locally
2. Insufficient heating/cooling capacity, failed stage, or abnormal cycling
3. Low supply airflow, dirty filter/coil, or failed fan proof
4. Economizer or outdoor-air damper introducing the wrong air condition
5. Refrigerant charge, condenser airflow, compressor, or heat-section fault
6. Sensor bias or normal OEM limit/transition omitted from host gating

## Energy Impact

A tracking fault may increase compressor, heating, or fan runtime and can
miss temperature or humidity targets. The sign alone does not establish
waste; no energy is inferred from temperature error without a causal model.

## Emissions Impact

Scope 1 and/or 2, qualitative. Quantify only after isolating the cause and
measuring affected fuel or electrical input against a defensible baseline.

## Deviations

- **The 2 K / 900 s pair is adopted library logic.** The sources support SAT
  tracking as a diagnostic, not portable packaged-unit thresholds.
- **Confidence is MEDIUM rather than the brief's proposed HIGH.** Cycling DX,
  active-setpoint semantics, airflow, OEM limits, and transitions require
  commissioned host gates before the same signature is causal.
- **Mechanical status is an OR, not a mode table.** Heating-only, cooling-only,
  or both-active raw states are detectable; the host validates the actual mode.
- **Direction handoff preserves persistence.** A sampled warm-to-cold transition
  remains continuously out of band; an actual in-band sample resets the timer.
- **Proof faults remain related, not suppressors.** Current metadata cannot
  suppress tracking only for RTU-0010's fail-to-start direction.
- **No empirical FPR or TPR is claimed.** The current harness lacks a defensible
  final active RTU SAT setpoint plus aligned mechanical-status mapping.
