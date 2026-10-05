---
schema: cxf-library/fault-card/v1
id: CHW-0009
name: Chiller short-cycling
equipment: chw
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "PNNL-29078, Building Re-Tuning Training Guide: Central Utility Plant Heating and Cooling Control Guide, PDF pp.90-91 (§10.1) — low-load inability to turn down can overshoot the internal setpoint and short-cycle a chiller"
  - "Library graph precedents PMP-0004, TOWER-0003, RTU-0001, and HW-0001 — rising-edge pulse integrated into a rolling event count"
  - "Library-authored threshold adaptation; the chiller manufacturer and plant sequence, not this source, define acceptable starts and minimum off time"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [CHW-0001, CHW-0007, CHW-0008]
playbooks: [chiller-efficiency]
operating_states: "all plant states in which this individual chiller may legitimately start; seasonal shutdown with no starts is a valid clear result"
preconditions: "Bind chiller_status to independent per-machine proof of capacity-producing operation, not availability, command, a fleet OR, or an enumeration that changes while continuously running. Status edges must represent real machine starts rather than communication loss or integration replay. The host reports NO_EVAL for the first evaluation_window after engine load/state reset because MovingAverage extrapolates an event pace while history fills. Acquire fast enough to observe the shortest real OFF/ON dwell, then evaluate on a fixed tick satisfying evaluation_window/63 <= tick < evaluation_window/(2*max_starts); defaults permit about 57.2 to under 600 s, with 60 s recommended. count_scale must equal evaluation_window/tick. Exclude approved exercise, tests, seasonal switchover, and maintenance. Commission max_starts and minimum on/off behavior against the chiller manufacturer's limits and the actual staging sequence."
points:
  - chiller_status
outputs:
  - name: yFault
    description: True while the observed per-machine rising-edge count is strictly above max_starts in the trailing evaluation_window
params:
  evaluation_window:
    default: 3600.0
    unit: s
    description: "Trailing start-count window. LIBRARY_PRECEDENT: one hour is an executable observation window, not a universal manufacturer requirement."
    cxf: rate.delta
  max_starts:
    default: 3.0
    unit: "1/window"
    description: "Allowed starts per window. ADOPTED_TUNABLE commissioning placeholder; strict comparison leaves exactly three clear and faults at four."
    cxf: cntHigh.t
  count_scale:
    default: 60.0
    unit: "1"
    description: "Derived coupling evaluation_window/evaluator_tick. The default 3600/60 is valid only at a 60 s fixed tick and must change with window or cadence."
    cxf: count.k
energy_impact:
  affected_subsystem: Chiller compressor, starter/drive, oil system, refrigerant cycle, and plant staging
  savings_range: "No portable kWh or avoided-maintenance value; primary benefit is protection from destructive cycling and correction of low-load staging"
  climate_sensitivity: strongest in shoulder/low-load cooling periods
  runtime_estimation: "QUALITATIVE_ONLY. Start proof alone cannot price acceleration, oil/refrigerant transients, machine size, or the cycling cause"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, July + January weeks, two individual parallel chillers, plant mode at 60 s"
    scenarios: 4
    failures: 1
    notes: "single RunPeriod with timeline/cadence validation; graph copies use count_scale=60. Both January machines and one July machine are clear; the other July power-based status series contains repeated sampled starts and correctly raises the raw cycling finding. Cycles completed inside 120 s remain unobservable and no OEM damage claim is inferred"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:1d73386b26deb3b315e35f31e43ffbff"
  date: 2026-10-05
---

## Description

This rule counts starts of one chiller over a trailing window. PNNL identifies
low-load inability to turn down as a common mechanism: the machine quickly
overshoots its internal target and shuts off, then restarts when load returns.
Unstable staging, narrow deadbands, missing storage, safety trips, or unreliable
proof can create the same observable signature.

## Detection Logic

```
start = rising edge of chiller_status
count = MovingAverage(start, evaluation_window) × count_scale

yFault = count > max_starts
```

![CHW-0009 block graph](diagram.svg)

`Logical.Edge` counts OFF-to-ON proof transitions. The initialization pulse has
zero area, but the partially filled moving-average window extrapolates a pace;
the first complete window is therefore host-NO_EVAL. The fourth observed start
inside the default trailing hour raises the immediate raw verdict, with no
additional delay.

## Possible Diagnoses

1. Chiller oversized for shoulder-season or process load
2. Staging/deadband or minimum on/off timers configured too narrowly
3. Insufficient loop volume or thermal storage
4. CHWST reset/control causing low-load setpoint overshoot
5. Compressor, starter/drive, oil, flow, freeze, or safety trip and auto-reset
6. Chattering or stale run proof and communication replay
7. Multiple chiller statuses incorrectly ORed into one point
8. Approved exercise/test sequence not host-gated

## Energy Impact

PROTECTIVE and QUALITATIVE_ONLY. Short cycling can waste transient energy and
accelerate component wear, but a Boolean start counter cannot quantify either.
Use machine power, OEM start limits, and plant staging history host-side.

## Emissions Impact

Scope 2, qualitative. Avoided transient electricity and premature component
replacement are not inferable from status edges alone.

## Deviations

- **The three-start limit is adopted, not source-transcribed.** The OEM minimum
  on/off and starts-per-hour limits are authoritative for each machine.
- **`count_scale` is added to the brief's parameter table.** MovingAverage
  returns pulse rate; multiplying by `evaluation_window/tick` recovers count.
- **The first full window is NO_EVAL.** Partial-window event pace and reload
  history loss are exposed in vectors rather than hidden.
- **The legal evaluator band is `[evaluation_window/63,
  evaluation_window/(2×max_starts))`.** The lower edge protects the 64-sample
  ring; the upper edge preserves observability of the first integer count above
  a strict threshold. Real OFF/ON dwell may require faster acquisition still.
- **Confidence is MEDIUM rather than the brief's proposed HIGH.** Edge counting
  is direct, but the shipped count and status quality require commissioning.
- **A fleet OR is invalid.** Two chillers can cycle independently while the OR
  remains continuously true, making every lag-machine start invisible.
- **CLU-06 is unchanged.** Cycling relates to efficiency but does not share the
  cluster's current efficiency trigger/fix semantics, so the cluster is not
  broadened in this slice.
