---
schema: cxf-library/fault-card/v1
id: ERV-0005
name: Supply/exhaust airflow imbalance
equipment: erv
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "Library-authored from conservation of flow and ERV commissioning practice: compare the two device-local air streams on one averaging basis, while retaining any intentional design offset as a site configuration obligation"
  - "Library precedents: ERV-0001 (device-local recovery boundaries and evaluability), SYS-0008 (two directional air-balance findings with per-condition persistence), and CHW-0005 (MultiplyByParameter plus Greater for a dynamic threshold)"
  - "points/erv.points.json erv_supply_airflow and erv_exhaust_airflow — Brick 1.4.4 / ASHRAE 223 / QUDT-grounded stream and unit contract"
  - "NREL Standard Work Specification 6.0303.1p and PNNL Building America balanced-ventilation guidance — public support for balancing incoming/outgoing ERV flow for recovery and pressure control"
  - "DOE/NREL Ventilation Integrated Comfort System report pp.28-29 — public example of frost prevention intentionally unbalancing core-path airflow, requiring the host frost-mode exclusion"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [ERV-0001, ERV-0004, SYS-0008]
playbooks: [erv-effectiveness]
operating_states: "ERV enabled and intended to operate in a balanced-flow mode, after both fans/dampers have completed their normal start or transition"
preconditions: "Both flow measurements must represent the same ERV, the streams that actually traverse the recovery device, and the same time-averaging basis in finite nonnegative L/s. Reversed polarity or signed bidirectional bindings are invalid; the graph intentionally does not repair them. minimum_evaluable_flow must be configured from this unit's size and sensor accuracy before deployment. The unit must have a balanced-flow intent: where design calls for building pressurization or another nonzero offset, the host must normalize/bias the measurements or configure a separate approved limit. Exclude smoke control, kitchen/lab exhaust offsets, purge, commissioning/balancing, demand-control ramps, frost strategies that intentionally unbalance flow, and sensor calibration failures. erv_enabled is in-graph; all other mode and quality gates remain host-side."
points:
  - erv_supply_airflow
  - erv_exhaust_airflow
  - erv_enabled
outputs:
  - name: yFault
    description: True while one evaluable directional imbalance remains active and enabled continuously for sustained_duration
  - name: yFlowOk
    description: Evaluability flag — true only when mean absolute flow exceeds minimum_evaluable_flow; false means NO_EVAL and the host must ignore yFault
  - name: ySupplyHigh
    description: Immediate diagnostic flag — evaluable supply flow exceeds exhaust by more than max_imbalance_fraction of mean flow
  - name: yExhaustHigh
    description: Immediate diagnostic flag — evaluable exhaust flow exceeds supply by more than max_imbalance_fraction of mean flow
params:
  max_imbalance_fraction:
    default: 0.15
    unit: "1"
    description: "ADOPTED_TUNABLE: directional difference allowed as a fraction of mean absolute flow. Must remain positive and be commissioned against design pressure/offset intent; 0.15 is an executable starting point, not a universal ventilation requirement."
    cxf: allowedDiff.k
  minimum_evaluable_flow:
    default: 100.0
    unit: "L/s"
    description: "NO_PORTABLE_DEFAULT: runnable 100 L/s placeholder below which sensor noise and ratios are not trusted. Set a positive instance value from design flow, turndown, and both sensors' usable range before deployment."
    cxf: flowOk.t
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE: continuous imbalance in one direction required before alarm (15 min). One card parameter drives both direction timers; hosts must set both paths together."
    cxf:
      - supplyHeld.delayTime
      - exhaustHeld.delayTime
energy_impact:
  affected_subsystem: "ERV fan energy, recovered heating/cooling, and building pressurization/infiltration"
  savings_range: "Site-specific. Imbalance can reduce useful recovery, indicate a blocked or failed air path, and drive infiltration/exfiltration load; no portable percentage follows from the two flow measurements alone."
  climate_sensitivity: both
  runtime_estimation: "Qualitative only. A host with pressure, fan power, temperatures, and expected effectiveness may estimate fan/infiltration/recovery penalties while yFault is active; this rule does not infer them from flow difference alone."
emissions:
  scope: "1+2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:5e63c030131568b1c91e53f28f1df15c"
  date: 2026-10-05
---

## Description

Energy recovery depends on two comparable air streams. A failed fan, loaded
filter, closed damper, poor balance, or bad sensor can leave one stream carrying
substantially more air than the other, reducing useful recovery and pushing the
building away from its intended pressure. This rule normalizes their difference
by mean flow, rejects low-flow operation, and reports which stream is high.

## Detection Logic

```text
mean_flow    = (|erv_supply_airflow| + |erv_exhaust_airflow|) / 2
allowed_diff = max_imbalance_fraction × mean_flow

yFlowOk      = mean_flow > minimum_evaluable_flow
ySupplyHigh  = yFlowOk AND (erv_supply_airflow − erv_exhaust_airflow > allowed_diff)
yExhaustHigh = yFlowOk AND (erv_exhaust_airflow − erv_supply_airflow > allowed_diff)

yFault = (erv_enabled AND ySupplyHigh) sustained for sustained_duration
      OR (erv_enabled AND yExhaustHigh) sustained for sustained_duration
```

Block graph (`rule.cxf.jsonld`):

![ERV-0005 block graph](diagram.svg)

The graph never divides: multiplying the positive evaluable mean by the allowed
fraction is algebraically equivalent and remains defined at zero flow. Both
comparisons and the flow floor are strict. Each direction owns a startup-
conservative timer, so a direct reversal resets persistence rather than allowing
two opposite short intervals to combine.

## Possible Diagnoses

1. Supply or exhaust fan failed, overridden, or running at the wrong speed
2. Loaded filter/core, blocked intake/discharge, or closed/stuck damper
3. Belt, wheel, or runaround-device problem changing system resistance
4. Unit never balanced, or balancing changed after filter/fan modifications
5. Airflow sensor bias, reversed polarity, mismatched averaging, or wrong unit
6. Intentional pressure offset not represented in the binding/configuration

## Energy Impact

COMFORT_ENERGY, MEDIUM confidence, QUALITATIVE_ONLY. The observed flow difference
is direct, but its energy consequence is not: it depends on fan curves, envelope
leakage, weather, pressure intent, and recovery effectiveness. The most valuable
output is often operational — which air path to inspect first.

## Emissions Impact

Scope 1 + 2, QUALITATIVE_EMISSIONS. Fan waste is scope 2; extra conditioning
from lost recovery or pressure-driven outdoor air follows the building's electric
cooling and electric/fuel heating systems. No emissions value is inferred here.

## Deviations

- **Division-free dynamic comparison.** The roadmap writes `difference / mean`;
  this graph compares each difference with `fraction × mean`, exactly equivalent
  when evaluable and defined even when both flows are zero.
- **One timer per direction.** A single delay after the directional OR would not
  reset on an instantaneous reversal, contradicting the required vector. The
  SYS-0008 per-condition idiom preserves the stated reset behavior.
- **`ySupplyHigh`/`yExhaustHigh` are immediate flow-gated diagnostics.** They are
  not persistence outputs and are not gated by `erv_enabled`; `yFault` is. False
  direction flags never mean NO_EVAL — that meaning belongs only to `yFlowOk`.
- **Absolute values serve only mean-flow evaluability.** Signed differences still
  choose direction, so a negative binding can look evaluable and alarm backwards;
  vectors pin this raw behavior and the host must reject such data.
- **0.15 is ADOPTED_TUNABLE; 100 L/s is NO_PORTABLE_DEFAULT.** Neither is called
  a standard requirement. Both must be reconciled with unit size, sensor range,
  and intentional pressure offset.
- **Frost-mode unbalance is explicitly excluded.** DOE's VICS testing shows a
  legitimate tempering strategy that changes the two core paths differently;
  evaluating that interval would diagnose the frost sequence as a flow fault.
- **No suppression or new cluster.** ERV-0004 can coexist with balanced airflow,
  and degraded effectiveness can remain a real separate finding. The shared
  investigation order is documented in the playbook.
- **No empirical validation claim.** The current EnergyPlus harness lacks two
  defensible device-local airflow measurements and operating-state mappings;
  synthetic vectors cover thresholds, evaluability, timing, and bad bindings.

## Notes

Do not "fix" a design pressure offset by widening the threshold until every unit
passes. Normalize to the intended offset first; the residual is the imbalance
this rule is meant to detect.
