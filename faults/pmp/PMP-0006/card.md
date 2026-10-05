---
schema: cxf-library/fault-card/v1
id: PMP-0006
name: Pump input-power degradation
equipment: pmp
status: verified
phase: 2
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: BASELINE_COMPARISON
source:
  - "DOE/Hydraulic Institute, Improving Pumping System Performance: A Sourcebook for Industry, 2nd ed., PDF pp.106 and 112–113 — assessment uses baseline energy plus flow, head, speed, fluid, and electrical data to identify efficiency degradation"
  - "Library-authored host-fitted expected-power residual; no universal pump curve or portable residual threshold is claimed"
  - "Library precedents AHU-0038 (host-published baseline and validity gate) and CHW-0001 (positive-baseline cross-multiplied degradation comparison)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [PMP-0001, PMP-0002, VFD-0001, VFD-0005]
playbooks: [vfd-pump-faults]
operating_states: "normal automatic pump operation after startup and before coast-down, with stable staging and a valid expected-power model"
preconditions: "The host owns pump_kw_expected. Train it on known-good operation for this pump, document the fit period and model inputs, freeze or version the fit for evaluation, and publish it only when ready, fresh, and in-domain. Typical inputs may include speed, individual-branch flow, differential pressure, staging, and fluid properties; all used inputs must be valid. pump_kw and pump_kw_expected must cover the same motor/drive electrical boundary and use kW. Exclude startup, coast-down, exercise, manual/bypass operation, safety/current/torque/demand limiting, and changes in parallel-pump configuration. A same-drive VFD-0001 or VFD-0005 is related evidence; suppress only if that deployment proves the active drive state or speed input invalidates this baseline and can scope the association to this pump. yBaselineOk checks numerical positivity only—false means NO_EVAL, while true does not prove freshness/domain validity."
points:
  - pump_status
  - pump_kw
  - pump_kw_expected
outputs:
  - name: yFault
    description: True while a running pump's measured input power has remained above the valid expected baseline by more than the allowed fraction for sustained_duration
  - name: yBaselineOk
    description: Evaluability flag — true only when expected power is above minimum_expected_kw; false means NO_EVAL
  - name: yPowerHigh
    description: Diagnostic direction flag — true when a numerically valid baseline has a positive residual above allowance; status is not part of this flag
params:
  minimum_expected_kw:
    default: 0.5
    unit: kW
    description: "Expected-power floor for numerical evaluability. NO_PORTABLE_DEFAULT: configure above the model's low-load/noise region; equality is not evaluable."
    cxf: baselineOk.t
  max_positive_residual_fraction:
    default: 0.15
    unit: "1"
    description: "Allowed positive fraction of expected input power. ADOPTED_TUNABLE; 0.15 is an executable starting band, not a published pump-wide limit."
    cxf: allowance.k
  sustained_duration:
    default: 1800.0
    unit: s
    description: "Continuous excess required before alarm. ADOPTED_TUNABLE 30-minute window; commission against model residuals and plant time constants."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Pump, motor, VFD/starter, and hydronic delivery path
  savings_range: "Measured opportunity while active is the positive power residual; annual savings require evaluable runtime and persistence"
  climate_sensitivity: loop-dependent
  runtime_estimation: "waste_kw = max(0, pump_kw - pump_kw_expected) while yFault is active; integrate only over host-evaluable intervals and do not treat model error as measured waste"
emissions:
  scope: "2"
  method: MEASURED_KWH_X_GRID_FACTOR
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:8d0553cbafa876eb8c3007a3178aa9a3"
  date: 2026-10-05
---

## Description

This rule detects a pump motor/drive assembly drawing materially more active
power than a known-good model expects at the same operating condition. The
signature can accompany mechanical drag, impeller/strainer degradation,
incorrect speed feedback, bypassed control, or reduced motor/drive efficiency;
it can also be manufactured by a stale, out-of-domain, or badly scoped
baseline. The model remains host-side and is published as an ordinary derived
point.

## Detection Logic

```
baseline_ok = pump_kw_expected > minimum_expected_kw
residual    = pump_kw - pump_kw_expected
allowance   = pump_kw_expected × max_positive_residual_fraction
power_high  = baseline_ok AND residual > allowance

yBaselineOk = baseline_ok
yPowerHigh  = power_high
yFault      = pump_status AND power_high, sustained for sustained_duration
```

![PMP-0006 block graph](diagram.svg)

The graph deliberately cross-multiplies instead of dividing. For a positive
valid baseline, `actual−expected > fraction×expected` is exactly the requested
relative-residual test and cannot evaluate a zero denominator. Both comparisons
are strict. `yPowerHigh` exposes residual direction independently of run status;
only `yFault` is status-gated and persisted with `delayOnInit=true`.

## Possible Diagnoses

1. Fouled or damaged impeller, blocked strainer, or unexpected hydraulic load
2. Bearing, seal, coupling, or alignment drag
3. Motor or drive efficiency degradation
4. Incorrect speed feedback or control in bypass/manual mode
5. Different parallel-pump staging than the baseline condition
6. Actual/expected points covering different electrical boundaries
7. Stale, out-of-domain, or degradation-trained expected-power model
8. Active power sensor scaling or wiring error

## Energy Impact

EFFICIENCY_LOSS with BASELINE_COMPARISON. During evaluable alarm intervals the
positive residual is directly measured kW above the host model, so the host may
integrate it to kWh. That estimate inherits model error and must exclude invalid
domains, startup, and mode changes. A low residual is intentionally not accused
by this rule because it can mean successful turndown or a separate delivery
failure.

## Emissions Impact

Scope 2. Multiply evaluable excess kWh by the applicable marginal or accounting
grid factor. Report the baseline/model uncertainty with the estimate.

## Deviations

- **No Divide block is used.** Downstream Boolean gating does not short-circuit
  an elementary Divide; cross-multiplication is equivalent in the positive
  baseline domain and guarantees no zero-denominator evaluation.
- **Expected power is a host-derived point, not a universal curve.** The point
  contract requires model inputs, known-good fit period, electrical boundary,
  and in-domain readiness. The graph cannot certify those obligations.
- **All three defaults need commissioning.** The 0.5 kW floor has
  NO_PORTABLE_DEFAULT; the 15% residual and 1800 s duration are adopted
  tunables, not source-transcribed thresholds.
- **Only positive residual is in scope.** Low power may indicate successful
  reset, broken coupling, bad metering, or another delivery fault; conflating
  the directions would erase diagnosis.
- **VFD relationships stay informational in the library metadata.** Whether a
  same-drive VFD-0001/0005 invalidates this model depends on its inputs and
  operating state; unconditional ID-level suppression could silence every pump
  or a baseline that does not use the disputed signal.
- **No simulation validation is claimed.** The plant harness now preserves
  per-pump power, flow, and status proxies, but it has no frozen expected-power
  model trained on a disjoint known-good period. Synthetic vectors validate
  graph behavior only.
