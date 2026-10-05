---
schema: cxf-library/fault-card/v1
id: HX-0002
name: Heat exchanger active with one-side flow missing
equipment: hx
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: DIRECT_MEASUREMENT
source:
  - "EnergyPlus 25.1 Input/Output Reference, HeatExchanger:FluidToFluid — generic four-port model and control modes request both connection flows when exchange operates: https://bigladdersoftware.com/epx/docs/25-1/input-output-reference/group-condenser-equipment.html#heatexchangerfluidtofluid"
  - "EnergyPlus 25.1 official PlantLoopChainHeating.idf and PlantLoopChainCooling.idf test models — separate supply/demand-side mass flows and operation status"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HX-0001, HX-0003, PMP-0001, PMP-0003]
playbooks: [hydronic-heat-exchanger-faults]
operating_states: "A controlled liquid-to-liquid HX in a final automatic state that presently expects both individual branches to flow"
preconditions: "exchange_cmd must be the final downstream both-flow expectation after temperature feasibility, pump/valve ownership, anti-cycle, local/HAND, freeze, pressure, minimum-flow, and other normal sequence logic. Availability, an upstream plant enable, or a supervisory status that permits zero flow is invalid. Both meters must be individual branches on the same HX, fresh, nonnegative in the declared inlet-to-outlet direction, and correctly converted to L/s; common-header, fleet, or duplicated flow is invalid. Configure each threshold above meter zero/noise but below the minimum legitimate established flow, and configure alarm_delay above the slowest permitted start/transport latency. Passive/uncontrolled exchangers and sequences that intentionally flow one side only while armed are NO_EVAL. Exclude maintenance, flushing, fill/purge, drain-down, exercise, and sensor invalidity."
points:
  - exchange_cmd
  - primary_flow
  - secondary_flow
outputs:
  - name: yFault
    description: True while either side-specific missing-flow diagnostic has matured
  - name: yPrimaryFlowMissing
    description: Delayed diagnostic direction flag; true when final exchange command is active and primary flow remains below its floor for alarm_delay. False never means NO_EVAL
  - name: ySecondaryFlowMissing
    description: Delayed diagnostic direction flag for the secondary side; false never means NO_EVAL
params:
  primary_flow_min:
    default: 1.0
    unit: L/s
    description: "NO_PORTABLE_DEFAULT executable placeholder. Commission above the primary meter's zero/noise/resolution and below minimum legitimate established branch flow before enabling the rule."
    cxf: primaryLow.t
  secondary_flow_min:
    default: 1.0
    unit: L/s
    description: "NO_PORTABLE_DEFAULT executable placeholder with the same side-specific commissioning requirement; unequal sides need not share a threshold."
    cxf: secondaryLow.t
  alarm_delay:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE 15-minute proof window. Set above the slowest valid final-command-to-flow latency and point delivery time. One value is applied to both independent timers."
    cxf: [primaryHeld.delayTime, secondaryHeld.delayTime]
energy_impact:
  affected_subsystem: HX heat delivery/rejection and the pumps/plant running without a complete transfer path
  savings_range: Site-specific; protective/delivery consequences may dominate energy waste
  climate_sensitivity: both
  runtime_estimation: "Qualitative from this rule. Use measured pump/plant power and the lost validated heat-transfer expectation only after identifying which side and component failed."
emissions:
  scope: "1+2"
  method: QUALITATIVE_ONLY
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:570e5bd05f74bc3a19c7e61c7543a042"
  date: 2026-10-05
---

## Description

A four-port liquid heat exchanger cannot transfer useful heat when one required
branch has no flow. This rule compares a final both-flow exchange command with
individual primary and secondary branch meters, delays each missing-side
signature independently, and tells the operator which side failed.

The rule does not prove a pump fault. A closed isolation valve, clogged
strainer, air lock, pressure problem, meter failure, or correct local sequence
can create the same observation. Final command semantics and branch scope are
therefore adoption requirements.

## Detection Logic

```text
primary_candidate   = exchange_cmd AND primary_flow < primary_flow_min
secondary_candidate = exchange_cmd AND secondary_flow < secondary_flow_min

yPrimaryFlowMissing   = primary_candidate continuously for alarm_delay
ySecondaryFlowMissing = secondary_candidate continuously for alarm_delay
yFault = yPrimaryFlowMissing OR ySecondaryFlowMissing
```

![HX-0002 block graph](diagram.svg)

Each side owns a `TrueDelay` with `delayOnInit = true`. If the missing side
reverses, the old lane clears and the new lane starts from zero; elapsed time is
not inherited through an OR. Both flags may mature if both flows are missing.
The strict `LessThreshold` makes exactly the configured floor safe.

## Possible Diagnoses

1. Side pump failed, tripped, lost coupling, or never received its final command.
2. Isolation/control valve closed, failed, or under local/HAND ownership.
3. Clogged strainer/plate passages, air lock, low pressure, or frozen path.
4. Failed check valve or hydraulic interaction preventing the intended branch.
5. Flow meter zero/scaling/freshness failure or common-header misbinding.
6. Upstream enable bound instead of the final both-flow expectation.

## Energy Impact

PROTECTIVE with DIRECT_MEASUREMENT and MEDIUM confidence. Pumps and plant may
consume energy while the exchanger delivers little or no useful transfer, but
this Boolean/flow signature cannot quantify the loss safely. In low-temperature
or protective service, delivery/freeze/equipment consequences can outweigh
energy cost.

## Emissions Impact

Scope 1+2, QUALITATIVE_ONLY. Quantify only after measuring the active plant and
pump energy plus any replacement heat source used during the incomplete path.

## Deviations

- **Thresholds have no portable default.** The numeric 1 L/s values exist for
  executable vectors only. Meter size, design flow, glycol, and minimum stable
  control flow are installation-specific and must replace them.
- **One delay is intentionally duplicated onto two blocks.** This preserves a
  direction reversal reset that a single delay after `(primary OR secondary)`
  cannot provide.
- **The final command is stricter than ordinary enable.** EnergyPlus's operation
  status/control behavior is physical precedent, not a claim that every BAS
  exposes the needed state. Without it the rule is not deployable.
- **No raw low-flow outputs.** The two exported direction flags are delayed
  findings, not mathematical evaluability flags. False never means NO_EVAL.
- **No suppression or cluster.** Pump proof/delivery rules can help diagnose a
  side, but no one causal trigger or repair clears all HX findings reliably and
  global rule-ID suppression would cross equipment instances.
