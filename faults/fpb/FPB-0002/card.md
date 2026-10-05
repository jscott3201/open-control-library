---
schema: cxf-library/fault-card/v1
id: FPB-0002
name: Primary airflow tracking failure
equipment: fpb
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: HIGH
estimation_method: QUALITATIVE_ONLY
source:
  - "Library executable precedent VAV-0004 — active-airflow-setpoint tracking semantics and two-sided diagnostic direction"
  - "Buildings.Controls.OBC.ASHRAE.G36 terminal-unit sequences — primary airflow measurement and active setpoint are distinct terminal control points; mechanism only"
  - "LBNL FDD simulated FPU dataset, DOI 10.25984/1881324 — PFPU/SFPU topology and future validation source, not threshold evidence"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [FPB-0001, FPB-0003, FPB-0004, FPB-0005, VAV-0004, AHU-0001, AHU-0024, AHU-0031]
playbooks: [fan-powered-terminal-faults]
operating_states: "enabled series or parallel FPB in a settled occupied/ventilation/control state where active primary airflow tracking is expected"
preconditions: "primary_airflow must be the AHU-fed primary inlet stream, excluding induced/plenum fan flow; primary_airflow_sp must be the settled final active target, not a design minimum/maximum. The upstream AHU fan and sufficient static pressure must be available. Validate airflow units, calibration and K-factor, controller pressure-independence, and data freshness. Exclude AHU shutdown, startup, setpoint ramps, balancing, overrides, and maintenance. minimum_airflow_sp is adoption-blocking and must be commissioned. ySetpointOk covers only the numerical setpoint floor; any other unmet host obligation is NO_EVAL, not healthy."
points: [primary_airflow, primary_airflow_sp]
outputs:
  - name: yFault
    description: True after primary airflow remains more than the allowed fraction above or below its active setpoint for sustained_duration
  - name: ySetpointOk
    description: "Evaluability flag — true only when primary_airflow_sp is strictly above minimum_airflow_sp. FALSE MEANS NO_EVAL, regardless of yFault"
  - name: yFlowLow
    description: Diagnostic direction flag; true immediately when an evaluable flow is more than the allowed fraction below target. False never means NO_EVAL
  - name: yFlowHigh
    description: Diagnostic direction flag; true immediately when an evaluable flow is more than the allowed fraction above target. False never means NO_EVAL
params:
  minimum_airflow_sp:
    default: 50.0
    unit: L/s
    description: "NO_PORTABLE_DEFAULT and adoption-blocking floor below which relative tracking is not evaluated; equality is NO_EVAL."
    cxf: setpointOk.t
  max_tracking_error_fraction:
    default: 0.20
    unit: "1"
    description: "ADOPTED_TUNABLE maximum absolute residual as a fraction of the positive active setpoint; both comparisons are strict."
    cxf: allowedError.k
  sustained_duration:
    default: 600.0
    unit: s
    description: "ADOPTED_TUNABLE continuous out-of-band duration; commission above box response and active-setpoint settling."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: FPB primary-air delivery, upstream AHU fan/static-pressure loop, and served zone
  savings_range: "Site-dependent; low flow is chiefly comfort/ventilation risk, while high flow can increase fan, cooling, and reheat energy"
  climate_sensitivity: both
  runtime_estimation: "QUALITATIVE_ONLY. Flow error-hours do not establish avoidable energy without pressure, load, thermal state, and a causal diagnosis."
emissions: {scope: "1/2", method: QUALITATIVE_EMISSIONS}
verified: {engine_rev: 41e997f, content_id: "cxf:fnv1a128:5c3e3827711a6dd02246c4398a19d26c", date: 2026-10-05}
---

## Description

This rule detects primary air delivered materially above or below the active
FPB target. It deliberately measures only the AHU-fed inlet stream; neither a
series fan's total discharge nor a parallel fan's induced branch is equivalent.

## Detection Logic

```
setpoint_ok  = primary_airflow_sp > minimum_airflow_sp
allowed      = primary_airflow_sp * max_tracking_error_fraction
flow_high    = (primary_airflow - primary_airflow_sp) > allowed
flow_low     = (primary_airflow_sp - primary_airflow) > allowed
yFlowHigh    = setpoint_ok AND flow_high
yFlowLow     = setpoint_ok AND flow_low
yFault       = setpoint_ok AND (flow_high OR flow_low),
               sustained for sustained_duration
```

![FPB-0002 block graph](diagram.svg)

The positive setpoint floor defines the valid domain. Cross-multiplied
residuals avoid `Divide` entirely, so zero setpoint cannot evaluate an unsafe denominator.

## Possible Diagnoses

1. Stuck, disconnected, or miscalibrated primary-air damper/actuator.
2. Insufficient or excessive upstream duct static pressure or failed AHU reset.
3. Blocked inlet, damaged flow ring, wrong K-factor, or sensor bias.
4. Stale, misbound, or incorrect active setpoint/units.
5. Pressure-dependent controller or subtype total-flow point bound as primary flow.

## Energy Impact

High primary flow can raise AHU fan, cooling, and reheat energy; low flow can
miss ventilation and comfort targets. The signature alone does not identify
which effect is avoidable, so the card remains qualitative.

## Emissions Impact

Scope 1/2 qualitative by the serving heating/cooling system. Quantify only
after isolating the cause and measuring the affected fan or thermal input.

## Deviations

- **The graph does not divide.** The brief's fractional-error equation is algebraically equivalent in the enforced positive-setpoint domain; cross-multiplication makes zero-denominator safety structural.
- **50 L/s is not portable.** It is an adoption-blocking placeholder because meaningful minimum flow scales with box size and ventilation design.
- **20% and 600 s are adopted.** Sources support the mechanism, not universal thresholds.
- **Direction handoff preserves persistence.** One timer follows the low/high OR; a continuously out-of-band sampled reversal remains one tracking fault.
- **`ySetpointOk` is not the whole host gate.** AHU availability, settled target, calibration, and controller behavior remain external obligations.
- **No empirical FPR or TPR is claimed.** The LBNL replay adapter is deferred to PR11.
