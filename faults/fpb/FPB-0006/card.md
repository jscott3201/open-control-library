---
schema: cxf-library/fault-card/v1
id: FPB-0006
name: Reheat-coil heat-transfer degradation
equipment: fpb
status: verified
phase: 3
method: statistical
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — PFPU/SFPU reheat topology and coil-fouling fault classes; future empirical replay source, not threshold evidence"
  - "Library host-baseline precedent PMP-0006 and coil-performance precedents AHU-0038/CHW-0005/CHW-0006 — positive baseline, in-domain gate, and non-causal degradation naming"
  - "Library-authored thresholds and persistence; no cited source publishes 90%, 3 K, 30%, or 900 s as portable FPB limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FPB-0003, FPB-0004, HW-0010]
playbooks: [fan-powered-terminal-faults]
operating_states: "hydronic-reheat FPB at stable near-full heat with proven coil-path airflow and a ready expected-delta-T model"
preconditions: "Apply only to hydronic reheat. Hot-water supply must be available at adequate temperature, flow, and pressure; plant tracking faults make this verdict NO_EVAL. rht_delta_t_expected must be trained on known-good operation, frozen/versioned, fresh, positive, and in-domain for current fan/airflow, entering temperature, valve state, and hot-water condition. Bind the same coil-local temperatures required by FPB-0003, including PFPU branch-local sensors before mixing. fan_status must prove airflow through that coil path. Exclude startup warm-up, valve exercise, freeze protection, maintenance, unstable fan/airflow, sensor faults, and plant transitions. minimum_expected_delta_t is adoption-blocking; yBaselineOk proves only numerical positivity."
points: [rht_vlv_cmd, rht_coil_entering_temp, rht_coil_leaving_temp, rht_delta_t_expected, fan_status]
outputs:
  - name: yFault
    description: True after proven fan operation, near-full heat command, a valid expected rise, and material actual-rise deficit persist for sustained_duration
  - name: yBaselineOk
    description: "Evaluability flag — true only when rht_delta_t_expected is strictly above minimum_expected_delta_t. FALSE MEANS NO_EVAL"
  - name: yFullHeatCommand
    description: Diagnostic sub-condition flag; true when rht_vlv_cmd is strictly above full_command_threshold. False never means NO_EVAL
  - name: yTemperatureRiseLow
    description: Diagnostic sub-condition flag; true when a numerically valid expected rise exceeds actual coil rise by more than the allowed fraction. False never means NO_EVAL
params:
  full_command_threshold:
    default: 90.0
    unit: "%"
    description: "ADOPTED_TUNABLE command above which the valve counts as near-full heat; equality is clear."
    cxf: fullHeat.t
  minimum_expected_delta_t:
    default: 3.0
    unit: K
    description: "NO_PORTABLE_DEFAULT and adoption-blocking expected-rise floor; equality is NO_EVAL."
    cxf: baselineOk.t
  max_delta_t_drop_fraction:
    default: 0.30
    unit: "1"
    description: "ADOPTED_TUNABLE maximum actual-rise deficit as a fraction of positive expected rise; strict comparison."
    cxf: allowance.k
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE continuous low-transfer duration; commission against coil, valve, fan, and hot-water response."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: FPB hydronic reheat coil, terminal fan/air path, and serving hot-water plant
  savings_range: "Not directly estimated; expected-minus-actual temperature-rise hours rank poor delivery but can reflect plant or airflow faults"
  climate_sensitivity: heating season and hot-water availability
  runtime_estimation: "PROXY_ESTIMATION only: combine validated coil airflow with positive expected-minus-actual rise over evaluable fault hours. Do not call the gap avoidable energy until cause is isolated."
emissions: {scope: "1/2", method: QUALITATIVE_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:573e7d690f4b0f4e699a19df07db6869", date: 2026-10-05}
---

## Description

This rule detects actual reheat-coil air temperature rise materially below a
known-good current-condition expectation at proven airflow and near-full valve
command. It reports heat-transfer degradation without overclaiming fouling.

## Detection Logic

```
actual_rise = rht_coil_leaving_temp - rht_coil_entering_temp
baseline_ok = rht_delta_t_expected > minimum_expected_delta_t
full_heat   = rht_vlv_cmd > full_command_threshold
allowance   = rht_delta_t_expected * max_delta_t_drop_fraction
rise_low    = baseline_ok AND (rht_delta_t_expected - actual_rise) > allowance
yBaselineOk = baseline_ok
yFullHeatCommand = full_heat
yTemperatureRiseLow = rise_low
yFault = fan_status AND full_heat AND rise_low, sustained for sustained_duration
```

![FPB-0006 block graph](diagram.svg)

The relative deficit is cross-multiplied in the positive-baseline domain;
there is no `Divide`. Persistence uses `delayOnInit=true`.

## Possible Diagnoses

1. Air- or water-side fouling, coil air bypass, or obstructed flow path.
2. Low hot-water temperature/flow/pressure or serving-plant tracking failure.
3. Valve/actuator/linkage not delivering the commanded full stroke.
4. Excessive or mis-modeled airflow/fan condition.
5. Sensor bias/location, actual/expected scope mismatch, or bad baseline model.

## Energy Impact

Poor transfer can extend heating runtime or miss comfort. The temperature-rise
deficit is a performance proxy, not automatically avoidable thermal energy.

## Emissions Impact

Scope 1/2 qualitative by the hot-water source and compensating equipment.

## Deviations

- **The rule names observable degradation, not fouling.** Hydraulic, actuator, airflow, plant, sensor, and model causes share the signature.
- **No Divide block is used.** Cross-multiplication is equivalent only in the positive expected-rise domain enforced by yBaselineOk.
- **The expected point is site-fitted.** Known-good training, inputs, model readiness, and hot-water condition stay host-side.
- **All defaults require commissioning.** 3 K is adoption-blocking; 90%, 30%, and 900 s are adopted tunables.
- **No empirical FPR/TPR is claimed.** LBNL mapping/replay is deferred to PR11.
