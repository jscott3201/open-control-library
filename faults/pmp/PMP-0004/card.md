---
schema: cxf-library/fault-card/v1
id: PMP-0004
name: Pump short-cycling
equipment: pmp
status: verified
phase: 2
method: rule
severity: 3
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "DOE/Hydraulic Institute, Improving Pumping System Performance: A Sourcebook for Industry, 2nd ed., PDF p.50 / printed p.48 — repeated starts wear controllers, contacts, seals, and bearings"
  - "Library-authored executable adaptation; no cited source publishes a portable starts-per-hour limit for every hydronic pump"
  - "Library graph precedents TOWER-0003, RTU-0001, HW-0001, and HP-0002 — rising-edge pulse integrated into a rolling event count"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [PMP-0003, VFD-0004]
playbooks: [vfd-pump-faults]
operating_states: "all plant states in which this individual pump may legitimately be started; seasonal lockout with no starts is a valid clear result"
preconditions: "Bind pump_status to independent per-pump proof of actual motor operation, not a command or an OR across a lead/lag fleet. Status transitions must represent real starts rather than communication loss, COV replay, or a changing software encoding. The host must report NO_EVAL for the first evaluation_window after engine load or state reset: the MovingAverage publishes an extrapolated pace while its window fills, so even one early start can raise the raw graph. Acquire fast enough to observe the shortest OFF/ON cycle, then evaluate on a fixed tick in the legal 57.2–360 s band at the defaults; 60 s is recommended and count_scale must equal evaluation_window/tick. Exclude approved exercise, commissioning, and functional-test sequences. When any obligation is unmet the verdict is NO_EVAL, not healthy."
points:
  - pump_status
outputs:
  - name: yFault
    description: True while the observed rising-edge count is strictly above max_starts in the trailing evaluation_window
params:
  evaluation_window:
    default: 3600.0
    unit: s
    description: "Trailing start-count window. LIBRARY_PRECEDENT: one hour follows the verified short-cycling family, not a universal pump requirement. Retuning it also requires retuning count_scale."
    cxf: rate.delta
  max_starts:
    default: 4.0
    unit: "1/window"
    description: "Allowed starts per evaluation_window. ADOPTED_TUNABLE commissioning placeholder; the pump/motor manufacturer and plant sequence are authoritative. The strict comparison leaves exactly four clear and faults at five."
    cxf: cntHigh.t
  count_scale:
    default: 60.0
    unit: "1"
    description: "Derived coupling evaluation_window/evaluator_tick. The default is 3600/60 and is correct only at a 60 s fixed tick; update it whenever the window or tick changes."
    cxf: count.k
energy_impact:
  affected_subsystem: Pump motor, VFD/starter, coupling, seals, and hydronic staging
  savings_range: "No portable kWh or avoided-maintenance value; the dominant benefit is reduced starts and component wear"
  climate_sensitivity: loop-dependent
  runtime_estimation: "none in-rule — QUALITATIVE_ONLY. The graph sees only run proof and cannot distinguish acceleration energy, pump size, or the cause of cycling"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, one July + one January week, one individual HW pump, plant mode at 60 s"
    scenarios: 2
    failures: 0
    notes: "single RunPeriod with timeline/cadence validation; strictly positive pump active power is the disclosed status proxy and the graph copy uses count_scale=60. Cycles completed inside 120 s remain unobservable"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:33ee395683a57a95cc32a753cedc0482"
  date: 2026-10-05
---

## Description

This rule reports an individual hydronic pump starting too often. Repeated
starts stress the motor, starter or drive, coupling, bearings, and seals, and
usually point to unstable staging, a narrow control deadband, insufficient
system storage, or unreliable proof. The signature is direct—observed starts
per rolling hour—but the cause and acceptable count remain pump- and
sequence-specific.

## Detection Logic

```
start  = rising edge of pump_status
count  = MovingAverage(start, evaluation_window) × count_scale
yFault = count > max_starts
```

![PMP-0004 block graph](diagram.svg)

`Logical.Edge` counts OFF-to-ON proof transitions only. Its initialization pulse
has zero area at the first tick, but the partially filled moving-average window
is an extrapolated event pace; the first full window is therefore host-gated.
The comparison is strict and has no added persistence: the fifth observed start
inside the default trailing hour raises the raw verdict. `count_scale` is
load-bearing because the moving average integrates one-tick pulses.

## Possible Diagnoses

1. Differential-pressure or temperature deadband set too narrowly
2. Lead/lag staging with inadequate minimum on/off times
3. Oversized pump or insufficient hydronic buffer at low load
4. VFD/control-loop hunting that repeatedly crosses the run threshold
5. Overload, safety, or drive fault repeatedly tripping and auto-resetting
6. Chattering contactor, current switch, auxiliary contact, or communication
7. Approved exercise/test sequence not excluded by the host

## Energy Impact

PROTECTIVE and QUALITATIVE_ONLY. Short cycles spend proportionally more time
accelerating and less time delivering stable flow, but this Boolean rule cannot
price that loss. The stronger value is avoided motor, starter/drive, seal, and
bearing wear. Size the opportunity host-side from motor power, observed starts,
and manufacturer start limits.

## Emissions Impact

Scope 2, qualitative. Avoiding unnecessary acceleration and premature
component replacement reduces electrical and embodied emissions, but neither
term is computable from run proof alone.

## Deviations

- **The four-start limit is adopted, not source-transcribed.** Pump and motor
  manufacturers publish application-specific limits; `4/window` is an
  executable commissioning placeholder and must be reviewed per asset.
- **`count_scale` is added to the planning table.** The engine's continuous-time
  MovingAverage returns a pulse rate, so `evaluation_window/tick` is required to
  recover an event count. Leaving 60 at a 300 s tick reports five times too many.
- **No alarm delay is copied from the sibling cards.** The PR brief defines
  `yFault = count > max_starts`; adding TOWER-0003's persistence would change the
  event-window semantics and introduce an unrequested parameter.
- **The first full window is NO_EVAL.** Partial-window extrapolation is pinned in
  vectors rather than hidden; model reload also loses all prior start history.
- **The evaluator band is `[evaluation_window/63,
  evaluation_window/(2×(max_starts+1)))`.** The lower bound protects the
  64-sample MovingAverage ring; the upper bound keeps the first integer count
  above the strict threshold observable. Defaults yield 57.2–360 s, with 60 s
  recommended. The evaluator must also resolve the shortest real OFF/ON dwell.
- **Confidence is MEDIUM rather than the brief's proposed HIGH.** Rising-edge
  detection is direct, but the shipped start limit has no portable source and a
  status point can be a proof/communications artifact until commissioned.
- **No Pump Delivery Failure cluster is added.** Cycling has no single trigger
  whose repair should clear the mutually different delivery/proof signatures.
