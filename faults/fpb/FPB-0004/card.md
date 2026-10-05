---
schema: cxf-library/fault-card/v1
id: FPB-0004
name: Terminal fan airflow degradation
equipment: fpb
status: verified
phase: 3
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — PFPU/SFPU topology and restricted-fan-flow fault class; future empirical replay source, not threshold evidence"
  - "Library host-baseline precedent PMP-0006 — positive-baseline cross-multiplied residual, numerical validity output, and explicit model-readiness obligations"
  - "Library-authored thresholds and persistence; no cited source publishes 50 L/s, 20%, or 900 s as portable FPB limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FPB-0001, FPB-0002, FPB-0005, FPB-0006]
playbooks: [fan-powered-terminal-faults]
operating_states: "stable automatic PFPU or SFPU fan operation with proven fan status and a ready same-path expected-airflow model"
preconditions: "fan_airflow_expected is host-fitted on a known-good period, frozen/versioned, fresh, and in-domain for the current fan command/speed, pressure, subtype, damper/topology, and operating state. Actual and expected must represent the same fan path: PFPU fan/plenum branch; SFPU series-fan path. fan_status must prove the same fan. Exclude startup, speed/mode/damper transitions, smoke operation, maintenance, local control, and invalid airflow sensing. minimum_expected_airflow is adoption-blocking. yBaselineOk proves only numerical positivity; other unmet obligations are NO_EVAL, not healthy."
points: [fan_status, fan_airflow, fan_airflow_expected]
outputs:
  - name: yFault
    description: True after proven fan operation and a valid expected baseline remain more than the allowed airflow fraction above actual delivery for sustained_duration
  - name: yBaselineOk
    description: "Evaluability flag — true only when fan_airflow_expected is strictly above minimum_expected_airflow. FALSE MEANS NO_EVAL"
  - name: yAirflowLow
    description: Diagnostic sub-condition flag; true when a numerically valid expected baseline exceeds actual fan-path airflow by more than the allowed fraction. False never means NO_EVAL
params:
  minimum_expected_airflow:
    default: 50.0
    unit: L/s
    description: "NO_PORTABLE_DEFAULT and adoption-blocking expected-airflow floor; equality is NO_EVAL."
    cxf: baselineOk.t
  max_airflow_drop_fraction:
    default: 0.20
    unit: "1"
    description: "ADOPTED_TUNABLE maximum actual-flow deficit as a fraction of positive expected flow; strict comparison."
    cxf: allowance.k
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE continuous low-delivery duration; commission against fan and baseline response."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: terminal fan path, served airflow, and any compensating primary-air or reheat response
  savings_range: "Not directly estimated; airflow-deficit hours rank degraded terminals but do not establish fan-power savings"
  climate_sensitivity: both
  runtime_estimation: "PROXY_ESTIMATION only: integrate positive expected-minus-actual fan-path airflow over evaluable fault hours. Do not convert to kWh without pressure/power and a causal model."
emissions: {scope: "2", method: QUALITATIVE_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:6329b30093556d09175f8d877b734a2a", date: 2026-10-05}
---

## Description

This rule detects fan-path airflow materially below a known-good, current-condition
baseline while the terminal fan is proven on. It reports delivery degradation,
not a specific dirty filter, fan, belt, damper, voltage, or sensor cause.

## Detection Logic

```
baseline_ok = fan_airflow_expected > minimum_expected_airflow
residual    = fan_airflow_expected - fan_airflow
allowance   = fan_airflow_expected * max_airflow_drop_fraction
airflow_low = baseline_ok AND residual > allowance
yBaselineOk = baseline_ok
yAirflowLow = airflow_low
yFault      = fan_status AND airflow_low, sustained for sustained_duration
```

![FPB-0004 block graph](diagram.svg)

Cross-multiplication makes the relative test safe without a `Divide`; all
comparisons are strict and persistence uses `delayOnInit=true`.

## Possible Diagnoses

1. Restricted intake/discharge, dirty filter, or obstructed fan-path damper.
2. Fouled/damaged wheel, slipping belt/coupling, low speed, voltage, or torque limit.
3. Different fan configuration, pressure, or subtype state than the baseline.
4. Actual/expected path mismatch, bad sensor, stale model, or degraded training data.

## Energy Impact

Low delivered airflow can extend fan/reheat/primary-air operation or miss load.
The residual is a delivery proxy, not measured power; no kWh is claimed.

## Emissions Impact

Scope 2 qualitative through fan and compensating HVAC electricity.

## Deviations

- **No Divide block is used.** Cross-multiplication is equivalent in the positive expected-flow domain and structurally safe at zero.
- **The expected point is site-fitted, not a universal fan curve.** Readiness, training, features, path scope, and domain checks remain host obligations.
- **All defaults require commissioning.** 50 L/s is adoption-blocking; 20% and 900 s are adopted tunables.
- **Proof remains related rather than a suppressor.** FPB-0001 fail-to-start invalidates the premise, but unexpected run can still leave this degradation signature meaningful.
- **No empirical FPR/TPR is claimed.** LBNL mapping/replay is deferred to PR11.
