---
schema: cxf-library/fault-card/v1
id: FPB-0005
name: Primary airflow sensor disagreement
equipment: fpb
status: verified
phase: 3
method: meta
severity: 3
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — primary-airflow sensor-bias fault class; future empirical replay source, not threshold evidence"
  - "Library sensor-health precedents SYS-0005 and SYS-0006 — independent-reference comparison, directional residuals, and explicit adjudication uncertainty"
  - "Library-authored thresholds and persistence; no cited source publishes 50 L/s, 15%, or 900 s as portable FPB limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
adjudicates:
  points: [primary_airflow]
  verdict: ambiguous
related: [FPB-0002, FPB-0004]
playbooks: [fan-powered-terminal-faults, sensor-drift]
operating_states: "steady FPB primary-air operation with sufficient reference flow and an independent, ready same-stream reference"
preconditions: "primary_airflow_reference must be independent of primary_airflow, known-good/in-domain, documented, fresh, and time-aligned. Valid sources include a certified redundant sensor, calibrated independent damper/pressure model, or validated upstream branch balance; a calculation that consumes primary_airflow is circular and invalid. Both points must use L/s and the same primary inlet stream/location. Exclude startup, setpoint/damper/static-pressure transitions the reference cannot follow, AHU shutdown, and insufficient steady flow. minimum_reference_airflow is adoption-blocking. yReferenceOk covers numerical positivity only. Because adjudicates.verdict is ambiguous, yFault establishes disagreement but does not by itself authorize automatic invalidation of primary_airflow; deployment must separately certify the reference before directional adjudication."
points: [primary_airflow, primary_airflow_reference]
outputs:
  - name: yFault
    description: True after the primary airflow measurement and independent reference remain more than the allowed fraction apart for sustained_duration; the verdict does not prove which value is wrong
  - name: yReferenceOk
    description: "Evaluability flag — true only when primary_airflow_reference is strictly above minimum_reference_airflow. FALSE MEANS NO_EVAL"
  - name: yPositiveBias
    description: Diagnostic direction flag; true when the measurement is above the valid reference by more than the allowed fraction. False never means NO_EVAL
  - name: yNegativeBias
    description: Diagnostic direction flag; true when the measurement is below the valid reference by more than the allowed fraction. False never means NO_EVAL
params:
  minimum_reference_airflow:
    default: 50.0
    unit: L/s
    description: "NO_PORTABLE_DEFAULT and adoption-blocking reference-flow floor; equality is NO_EVAL."
    cxf: referenceOk.t
  max_disagreement_fraction:
    default: 0.15
    unit: "1"
    description: "ADOPTED_TUNABLE allowed absolute residual as a fraction of positive reference flow; both comparisons are strict."
    cxf: allowance.k
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE continuous disagreement duration; commission above measurement/reference lag and normal control transitions."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: primary airflow sensing and every FPB control/diagnostic decision consuming it
  savings_range: "No direct savings; avoided impact belongs to downstream control errors corrected after the bad value is isolated"
  climate_sensitivity: neutral
  runtime_estimation: "QUALITATIVE_ONLY. Disagreement hours measure diagnostic exposure, not energy waste."
emissions: {scope: "1/2", method: QUALITATIVE_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:dea3bb3d4caf1dc61fdbb108eaaa20e3", date: 2026-10-05}
---

## Description

This rule gives the canonical primary-airflow sensor an independent second
opinion. Sustained disagreement is a sensor-integrity finding, but an ordinary
host-derived reference can be wrong too, so the verdict remains ambiguous.

## Detection Logic

```
reference_ok = primary_airflow_reference > minimum_reference_airflow
allowance    = primary_airflow_reference * max_disagreement_fraction
positive     = (primary_airflow - primary_airflow_reference) > allowance
negative     = (primary_airflow_reference - primary_airflow) > allowance
yPositiveBias = reference_ok AND positive
yNegativeBias = reference_ok AND negative
yReferenceOk  = reference_ok
yFault = reference_ok AND (positive OR negative), sustained for sustained_duration
```

![FPB-0005 block graph](diagram.svg)

No `Divide` exists. One timer follows the direction OR, so a sampled sign
reversal without reconvergence preserves age; any in-band sample resets it.

## Possible Diagnoses

1. Primary airflow sensor bias, scaling/K-factor, tubing, or pickup fault.
2. Reference sensor/model bias, stale version, or out-of-domain extrapolation.
3. Circular reference, different stream/location, time misalignment, or unit mismatch.
4. Real transient the reference cannot reproduce.

## Energy Impact

The disagreement itself uses no energy. Impact is whatever wrong airflow
control or downstream diagnosis the erroneous member causes.

## Emissions Impact

Scope 1/2 qualitative through downstream heating, cooling, and fan decisions.

## Deviations

- **Ambiguous means no automatic victim.** The graph's input roles differ, but the reference contract is not inherently trusted enough to invalidate primary_airflow without deployment certification.
- **No Divide block is used.** Positive-reference cross-multiplication is safe at zero and negative raw references.
- **All defaults require commissioning.** The floor is adoption-blocking; 15% and 900 s are adopted.
- **FPB-0002 is not automatically suppressed.** A biased measurement can explain tracking error, but an ambiguous reference cannot silently erase it.
- **No empirical FPR/TPR is claimed.** LBNL replay is deferred to PR11.
