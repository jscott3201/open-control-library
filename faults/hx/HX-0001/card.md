---
schema: cxf-library/fault-card/v1
id: HX-0001
name: Hydronic heat-exchanger effectiveness degradation
equipment: hx
status: verified
phase: 2
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: BASELINE_COMPARISON
source:
  - "EnergyPlus 25.1 Engineering Reference, Heat Exchangers — epsilon-NTU model using both flow-capacity rates and inlet temperatures: https://bigladdersoftware.com/epx/docs/25-1/engineering-reference/heat-exchangers.html"
  - "Guelpa and Verda, Applied Energy 258 (2020), DOI 10.1016/j.apenergy.2019.114059 — field fouling detection on 325 district-heating HX substations from primary mass flow and temperatures on both sides"
  - "DOE FEMP, Energy Management Information System Capabilities — reduced HX heat transfer from temperature sensors as a condition-based maintenance signal: https://www.energy.gov/cmei/femp/energy-management-information-system-capabilities"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HX-0002, HX-0003]
playbooks: [hydronic-heat-exchanger-faults]
operating_states: "One indirect liquid-to-liquid HX exchanging heat in a settled heating or cooling state, with both branch flows established and a frozen clean/design expected-effectiveness model ready and in domain"
preconditions: "All six physical derivation inputs must describe the same HX: primary/secondary entering/leaving temperatures plus individual branch flows, aligned in time and correctly scaled. The host computes effectiveness only after proving positive finite thermal capacity rates, sufficient entering-temperature separation, configured density/cp for each fluid (including glycol concentration), and agreement of independently calculated side heat rates within commissioned uncertainty. The expected model must be frozen, independently fitted/commissioned, ready, fresh, and in domain for the current flow-capacity ratio, entering temperatures, direction, and control state. Suspend and re-warm after starts, direction/setpoint/pump/valve/stage changes. A common-header flow, duplicated side point, same-window fitted target, imbalance, or invalid denominator means NO_EVAL, not healthy. Steam/phase-change, air/refrigerant, potable, direct-contact, and aggregate-bank service are excluded."
points:
  - effectiveness
  - effectiveness_expected
outputs:
  - name: yFault
    description: True after actual effectiveness remains more than effectiveness_allowance below the valid expected value for alarm_delay
  - name: yEffectivenessLow
    description: Diagnostic sub-condition flag; true when expected minus actual effectiveness strictly exceeds the allowance. False never means NO_EVAL
params:
  effectiveness_allowance:
    default: 0.125
    unit: "1"
    description: "NO_PORTABLE_DEFAULT executable placeholder: 0.125 effectiveness points is a binary-exact vector fixture, not a field recommendation. Commission from clean-model error, sensor/fluid-property uncertainty, and the minimum actionable degradation before enabling this rule."
    cxf: shortfallHigh.t
  alarm_delay:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE 15-minute persistence after the host's independent settling/re-warm gate. Retune to the installation time constant and data cadence; no cited source establishes a universal duration."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Hydronic exchange plus upstream heating/cooling and pumping needed to replace lost transfer
  savings_range: Site-specific; the field source estimates about 1.6% primary-energy reduction across its whole district network from cleaning detected fouling, not a per-HX savings claim
  climate_sensitivity: both
  runtime_estimation: "lost_kw = max(effectiveness_expected - effectiveness, 0) × min(C_primary, C_secondary) × abs(primary_entering_temp - secondary_entering_temp), evaluated only with the same validated host derivation"
emissions:
  scope: "1+2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:95d59df8187626eb12cc97061e4b7f9b"
  date: 2026-10-05
---

## Description

An indirect liquid heat exchanger loses effectiveness when fouling, scale,
blocked channels, internal bypass, wrong fluid properties, or hydraulic changes
reduce the heat it moves for the opportunity available. This rule compares a
host-validated actual thermal effectiveness with a frozen clean/design expected
value for the same operating condition. It reports degradation, not a root
cause and not a raw "approach" temperature.

The four-port point identity matters as much as the arithmetic. Primary and
secondary are fixed topology labels; heating usually makes signed transfer
positive and cooling negative. The host converts both directions to a positive
effectiveness before the graph sees them.

## Detection Logic

```text
shortfall = effectiveness_expected - effectiveness
yEffectivenessLow = shortfall > effectiveness_allowance
yFault = yEffectivenessLow continuously for alarm_delay
```

![HX-0001 block graph](diagram.svg)

The graph has no `Divide`. The host publishes `effectiveness` only after safe
denominator, fluid-property, timestamp, and side-energy-balance checks. A
denominator guard downstream of a division would not prevent that division
from evaluating; moving the validated thermodynamic derivation to the host
also supports water/glycol properties the CXF graph does not carry.

Both comparisons use finite dimensionless scalars and the threshold is strict.
`yEffectivenessLow` is immediate diagnostic evidence; only `yFault` is delayed.

## Possible Diagnoses

1. Plate/tube fouling, scale, biological film, or blocked channels.
2. Internal gasket/bypass leakage or incorrect HX piping.
3. Insufficient or maldistributed flow not caught by the commissioned floors.
4. Degraded or misconfigured glycol concentration/fluid properties.
5. Temperature/flow sensor bias, time misalignment, or swapped side/location.
6. Expected model drift, wrong domain, or baseline trained on abnormal data.

## Energy Impact

EFFICIENCY_LOSS with BASELINE_COMPARISON and MEDIUM confidence. Lost transfer
must be replaced by upstream boilers, chillers, heat pumps, district energy, or
longer pumping. The estimator uses the same validated available-rate basis as
the effectiveness calculation; this two-point graph alone cannot produce kW.
Guelpa and Verda's 1.6% is a network-wide expected benefit from a cleaning
program across 325 substations, not a savings range to assign to one alarm.

## Emissions Impact

Scope 1+2, PROXY_EMISSIONS. Apply the marginal emissions rate of the actual
replacement heat source and electricity used while the fault is active. Do not
infer fuel/electric split from transfer direction alone.

## Deviations

- **The thermodynamic ratio is host-derived.** EnergyPlus documents the
  epsilon-NTU physics, but the repository graph intentionally compares two safe
  scalars instead of dividing inside CXF. This is a safety and fluid-property
  adaptation, not a claim that the host model is standardized.
- **`effectiveness_allowance = 0.125` is not portable.** No source supplies a
  universal threshold. The exact binary value makes strict-boundary vectors
  unambiguous; deployment must replace it before enabling evaluation.
- **The field method is precedent, not a transcribed algorithm.** Guelpa and
  Verda use a calibrated fouling workflow under variable district-heating
  conditions. This card keeps the baseline/error-domain obligation but does not
  claim to reproduce their full method.
- **No in-graph readiness flag.** Baseline/domain, denominator, and balance
  validity depend on provenance and configuration beyond two boundary points;
  they are mandatory host NO_EVAL gates.
- **No suppression.** HX-0002 may explain why HX-0001 is unevaluable, but rule
  IDs are not equipment-instance scoped. A host gates the same instance rather
  than globally suppressing every HX-0001 when any HX-0002 is active.
- **Initial scope excludes steam.** Phase change needs a different capacity and
  topology contract even though some trade usage calls it hydronic.
