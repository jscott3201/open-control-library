---
schema: cxf-library/fault-card/v1
id: CHW-0007
name: Chilled-water supply temperature tracking failure
equipment: chw
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "PNNL-29078, Building Re-Tuning Training Guide: Central Utility Plant Heating and Cooling Control Guide, PDF pp.91 and 96 (§10) — the plant maintains a chilled-water supply target and should trend supply temperature during operation"
  - "EPA Facilities Manual, Volume 2, ch.9 Table 9-2 — chiller BAS monitoring includes leaving-water setpoint, start/stop, failure, and chilled-water temperature above setpoint"
  - "Library-authored executable adaptation of AHU-0033's strict mirrored tracking-error topology; no source publishes the shipped 1 K / 20% / 900 s combination as portable"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [CHW-0001, CHW-0002, CHW-0005, CHW-0006, CHW-0008, CHW-0009]
playbooks: [chiller-efficiency]
operating_states: "normal automatic chiller operation after startup, with the individual machine proven running and loaded above minimum_load"
preconditions: "chwst and chwst_sp must describe the same controllable leaving-water target. Prefer this chiller's evaporator outlet temperature and the final active setpoint used by its controller. A common-header measurement/setpoint is acceptable only when the deployment proves the staged machine(s) truly control that same mixed-header target; it is not interchangeable with an individual barrel outlet by assumption. chiller_status and chiller_load must belong to the same machine, not a fleet OR/max. Flow and minimum-flow permissives must be established. Exclude startup pull-down, setpoint/reset ramps, staging transfers, pump/valve transients, ice-making, and manufacturer current/lift/surge/freeze/demand limits. Temperature points must be healthy, fresh, aligned, and in degC. yLoadOk is numerical evaluability only: false means NO_EVAL; true does not prove any of the host obligations. A stopped chiller is also NO_EVAL even though yLoadOk can remain true."
points:
  - chwst
  - chwst_sp
  - chiller_status
  - chiller_load
outputs:
  - name: yFault
    description: True while a running, meaningfully loaded chiller has remained outside either side of the active CHWST band for sustained_duration
  - name: yTooWarm
    description: Diagnostic direction flag — true when the evaluable leaving-water temperature is more than tracking_error above setpoint
  - name: yTooCold
    description: Diagnostic direction flag — true when the evaluable leaving-water temperature is more than tracking_error below setpoint
  - name: yLoadOk
    description: Evaluability flag — true only when chiller_load is strictly above minimum_load; false means NO_EVAL
params:
  tracking_error:
    default: 1.0
    unit: K
    description: "Symmetric leaving-water tracking allowance. ADOPTED_TUNABLE: commission above combined sensor uncertainty and the settled control deadband; equality is clear."
    cxf: [warm.t, cold.t]
  minimum_load:
    default: 20.0
    unit: "%"
    description: "Per-machine load floor. ADOPTED_TUNABLE: excludes low-load cycling and unloading behavior; equality is not evaluable."
    cxf: loadOk.t
  sustained_duration:
    default: 900.0
    unit: s
    description: "Continuous out-of-band duration. LIBRARY_PRECEDENT informed by hydronic/SAT tracking timescales; verify against this plant's pull-down, reset, and staging dynamics."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Chiller compressor, chilled-water plant, and air-side cooling delivery
  savings_range: "Site-dependent; overcooling increases lift while undercooling can increase pump/fan demand and compromise comfort or humidity control"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "QUALITATIVE_ONLY. Error-hours do not determine excess kW without load, lift, flow, air-side response, and an efficiency baseline"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, July week, two individual parallel chillers, plant mode at 60 s"
    scenarios: 2
    failures: 0
    notes: "single RunPeriod with timeline/cadence validation; strictly positive per-machine electricity is the run-status proxy, PLR is per-machine load, and direct evaporator outlet temperature is compared with the shared target. Expectations begin 1800 s after the same machine runs above 20%; January had no evaluable window and is not counted"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:c2159704c8d4e994bfed070006ff5d4e"
  date: 2026-10-05
---

## Description

This rule reports a chiller or genuinely representative plant leaving-water
temperature that cannot hold its active target under meaningful load. A warm
direction can mean insufficient capacity, lost flow, fouling, refrigerant or
sensor trouble, or a current/lift limit. A cold direction can mean aggressive
staging, a misapplied setpoint, or a control loop driving below target. The
direction narrows investigation; it does not identify the failed component.

## Detection Logic

```
error          = chwst - chwst_sp
load_ok        = chiller_load > minimum_load
running_loaded = chiller_status AND load_ok
warm           = error > tracking_error
cold           = -error > tracking_error

yLoadOk  = load_ok
yTooWarm = running_loaded AND warm
yTooCold = running_loaded AND cold
yFault   = running_loaded AND (warm OR cold), sustained for sustained_duration
```

![CHW-0007 block graph](diagram.svg)

Both directional comparisons are strict. The diagnostic flags are gated by
machine status and load but are not persistence outputs. The single
`TrueDelay(delayOnInit=true)` follows their OR, so a sampled direct handoff from
too warm to too cold preserves timing: the machine never re-entered its band.
Any in-band, stopped, or low-load tick resets the timer, and recovery clears
immediately.

## Possible Diagnoses

1. Insufficient evaporator flow, closed isolation valve, or failed pump proof
2. Fouled evaporator tubes or low refrigerant charge
3. Compressor current, demand, surge, lift, or freeze-limit operation
4. Incorrect staging or a machine too small for the present load
5. Leaving-water sensor bias, stale value, or machine/header misbinding
6. Active setpoint not delivered to the chiller controller
7. Over-responsive local control, bad tuning, or unexcluded setpoint reset
8. Intentional ice-making or other non-comfort operating mode not host-gated

## Energy Impact

COMFORT_ENERGY and QUALITATIVE_ONLY. Overcooling normally increases compressor
lift; water that is too warm can force more air/water flow and can miss space or
humidity targets. This graph has no causal baseline or flow/power measurement,
so it does not turn temperature error into claimed savings.

## Emissions Impact

Scope 2, qualitative. Any avoided electrical energy is established only after
the host quantifies the chiller, pump, and air-side response.

## Deviations

- **All three defaults require commissioning.** PNNL supports the control and
  low-load context, not a universal 1 K error, 20% load floor, or 900 s delay.
- **Confidence is MEDIUM rather than the brief's proposed HIGH.** The signature
  is direct, but operating limits, setpoint transitions, and machine/header
  topology can produce the same evidence until binding and host gates are
  commissioned.
- **Common-header binding is conditional.** A mixed-header temperature can be a
  real plant target, but cannot validate an individual barrel by name alone.
- **Direction handoff preserves persistence.** One timer after the OR measures
  continuous out-of-band operation; an actual in-band sample resets it.
- **No automatic suppression is added.** CHW-0008 cannot suppress only its
  fail-to-start direction in current rule-wide metadata, and tracking during an
  unexpected run remains useful evidence. Hosts order diagnosis by direction.
- **CLU-06 is unchanged.** This control-capability signature is related to
  efficiency and approach findings, but the existing cluster is not broadened
  into a mixed staging/proof cluster in this slice.
