---
schema: cxf-library/fault-card/v1
id: FPB-0003
name: Reheat valve closed with unintended temperature rise
equipment: fpb
status: verified
phase: 2
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Library leak-mechanism precedents FCU-0005 and VAV-0009 — air temperature rise across reheat while the hydronic valve is commanded shut"
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — documented PFPU fan/reheat branch and SFPU downstream-fan reheat topology; future validation source only"
  - "Library-authored instantaneous coil-local adaptation; no cited source publishes 5%, 3 K, or 600 s as portable FPB limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FPB-0001, FPB-0002, FPB-0006, FCU-0005, VAV-0009]
playbooks: [fan-powered-terminal-faults]
operating_states: "hydronic-reheat FPB states with proven airflow through the reheat coil and no legitimate reheat request"
preconditions: "Apply only to hydronic reheat with available hot water at meaningful temperature and pressure. fan_status must prove airflow through the evaluated coil path. Bind physical coil-local temperatures: SFPU downstream of the series fan/immediately upstream of the coil and immediately at coil outlet; PFPU immediately around the fan/reheat branch before primary/branch mixing. A mixed zone-discharge proxy is invalid unless the host supplies a validated derived coil-leaving estimate. Exclude freeze/exercise/commissioning, intentional minimum valve position, sensor faults, and warm-soak after a prior heating call. Inputs must be fresh and aligned; unmet obligations are NO_EVAL, not healthy."
points: [rht_vlv_cmd, rht_coil_entering_temp, rht_coil_leaving_temp, fan_status]
outputs:
  - name: yFault
    description: True after proven coil airflow, a closed valve command, and material coil-local temperature rise persist for sustained_duration
  - name: yValveClosed
    description: Diagnostic sub-condition flag; true when rht_vlv_cmd is strictly below valve_closed_threshold. False never means NO_EVAL
  - name: yTemperatureRise
    description: Diagnostic sub-condition flag; true when coil leaving minus entering temperature is strictly above temperature_rise_threshold. False never means NO_EVAL
params:
  valve_closed_threshold:
    default: 5.0
    unit: "%"
    description: "ADOPTED_TUNABLE command below which the reheat valve counts as closed; equality is not closed."
    cxf: valveClosed.t
  temperature_rise_threshold:
    default: 3.0
    unit: K
    description: "ADOPTED_TUNABLE coil-local rise allowance above sensor error, fan heat, piping migration, and normal residual heat; equality is clear."
    cxf: riseHigh.t
  sustained_duration:
    default: 600.0
    unit: s
    description: "ADOPTED_TUNABLE continuous signature duration; no existing 600 s leakage rule or cited source establishes a portable precedent."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: FPB hydronic reheat coil, upstream heating plant, and any cooling needed to offset leaked heat
  savings_range: "Site-dependent; coil rise and primary airflow can support a thermal proxy once the branch/total-flow relationship is established"
  climate_sensitivity: heating-water availability and simultaneous-cooling exposure
  runtime_estimation: "PROXY_ESTIMATION: fault hours times a validated coil airflow and air-side temperature rise can estimate leaked heat; do not substitute primary flow blindly for PFPU branch flow."
emissions: {scope: "1/2", method: PROXY_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:4d6cece087e01b0d5e88794f4e8f2d30", date: 2026-10-05}
---

## Description

This rule identifies heat added across a hydronic FPB reheat coil while its
valve is commanded shut and airflow through that coil is proven. Coil-local
measurement is essential, especially before PFPU branch air mixes with primary air.

## Detection Logic

```
valve_closed = rht_vlv_cmd < valve_closed_threshold
rise         = rht_coil_leaving_temp - rht_coil_entering_temp
rise_high    = rise > temperature_rise_threshold
yValveClosed = valve_closed
yTemperatureRise = rise_high
yFault = fan_status AND valve_closed AND rise_high,
         sustained for sustained_duration
```

![FPB-0003 block graph](diagram.svg)

Both thresholds are strict. The complete three-part candidate feeds one
`TrueDelay(delayOnInit=true)`; loss of any premise clears a mature alarm immediately.

## Possible Diagnoses

1. Valve seat passing from wear, debris, fouling, or unsuitable close-off pressure.
2. Actuator/linkage not reaching the seat despite a closed command.
3. Manual bypass, three-way piping, or unintended gravity circulation.
4. Residual hot-water availability or warm-soak not excluded by the host.
5. Entering/leaving sensor bias, swap, or PFPU mixed-discharge misbinding.

## Energy Impact

Leaked reheat can be paid for twice when primary cooling removes it again.
Temperature rise is only a proxy; PFPU primary airflow is not automatically
the branch coil airflow required to turn that rise into thermal power.

## Emissions Impact

Scope 1 and/or 2 depends on heating and cooling sources. Use validated coil
airflow, rise, runtime, and source-specific factors before quantifying savings.

## Deviations

- **All three defaults are adopted.** The brief labeled 600 s as library precedent, but no existing 600 s leakage rule supports that claim; it is recorded as `ADOPTED_TUNABLE` instead.
- **The rule is hydronic-only.** Electric reheat needs proof/status logic and safety treatment rather than a fictitious valve command.
- **Temperatures are coil-local.** This is stricter than common VAV/FCU proxies and prevents PFPU branch mixing from erasing the signature.
- **Fan proof is in-graph, but other gates remain host-side.** Hot-water availability, warm-soak, freeze/exercise, and point quality still determine evaluability.
- **The rule is related to leak siblings but not placed in CLU-01.** A single AHU simultaneous-command repair cannot reliably clear a physically passing terminal valve.
- **No empirical FPR or TPR is claimed.** LBNL replay and mapping are deferred to PR11.
