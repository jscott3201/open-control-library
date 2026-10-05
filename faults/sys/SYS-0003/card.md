---
schema: cxf-library/fault-card/v1
id: SYS-0003
name: Lighting on with no occupancy
equipment: sys
status: verified
phase: 2
method: rule
severity: 4
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §16, SYS-0003 (pdf pp. 140-141) — equation, AlarmDelay 30 min, severity 4 (info), the four diagnoses, and the whole impact profile"
  - "The reference's own provenance line for that card: Mazzetto 2025 — 1,149 occurrences in 10 months at one facility"
  - "PNNL EEM-18 (lighting occupancy sensors) — the reference's PNNL cross-reference for the 15-20% annual figure"
  - "Library precedent: AHU-0018 (the host-evaluated occupancy boolean, and the same Not/And/TrueDelay shape)"
g36: null
clusters: [CLU-04]
suppresses: []
suppressed_by: []
related: [AHU-0018, SYS-0004, SYS-0008]
playbooks: [after-hours-operation]
operating_states: "all — the conjunction is self-gating, since two of its three terms are the unoccupied test"
preconditions: "The occupancy sensor and the lighting circuit must cover the same space. This is a per-instance binding claim the graph cannot check, and getting it wrong is the rule's main false-positive path: a corridor PIR paired with an open-plan circuit reports a fault every time the corridor empties. occ_scheduled is host-evaluated from the lighting schedule for THAT space, not the AHU's — a building whose HVAC and lighting schedules differ needs the lighting one here, and schedule provenance that is stale or unknown is NO_EVAL rather than unoccupied. lighting_status should be a proven circuit status (current sensor, relay auxiliary contact, panel feedback) and not the command: a command point makes this rule an audit of the BAS's intent, which is exactly the thing diagnosis 1 says has already failed. Occupancy-sensor timeout is the host's to reconcile — a sensor whose timeout is long relative to alarm_delay delays the finding, and one that drops a stationary occupant produces a fault that is true given the data and wrong about the building."
points:
  - lighting_status
  - occ_sensor
  - occ_scheduled
outputs:
  - name: yFault
    description: True while the lighting circuit has been energized with the occupancy sensor unoccupied and the schedule closed, continuously for alarm_delay
params:
  alarm_delay:
    default: 1800.0
    unit: s
    description: "Continuous persistence required before the alarm asserts (30 min). The reference's only tunable for this rule."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Lighting electrical energy
  savings_range: "100% of lighting energy while active; 15-20% annually (EEM-18, lighting occupancy sensors)"
  climate_sensitivity: neutral
  runtime_estimation: "waste_kw = lighting_circuit_kw"
emissions:
  scope: "2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:763feed2eb268c080053a6ce13419359"
  date: 2026-10-05
---

## Description

Lights burning in an empty building after hours. The rule wants two independent
witnesses before it says the space is empty — the occupancy sensor and the
schedule — because either alone is wrong often enough to be useless: a schedule
says nothing about the person working late, a PIR says nothing about the person
sitting still. That is what makes a finding worth dispatching, and it is also
why the rule under-reports: lights on all night in a room whose sensor has
failed to a permanent "occupied" are invisible here. Prevalence is the reason
the card exists — Mazzetto (2025) logged 1,149 occurrences in ten months at a
single facility, and every hour of it is 100% waste. It is the only card in the
library that leaves the mechanical plant, and it earns its place because the
schedule that is wrong here is usually the master schedule AHU-0018 is failing
on (hence CLU-04) and the fix is the same BAS work order.

## Detection Logic

```
yFault = lighting_status
     AND NOT occ_sensor
     AND NOT occ_scheduled
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![SYS-0003 block graph](diagram.svg)

Five blocks, no arithmetic and no thresholds — every input is already a boolean,
so there is nothing to compare and nothing to tune but the delay.

Each of the three conjuncts blocks the fault by itself, and any of them going
the other way drops a live alarm on the same tick: `TrueDelay` delays the rising
edge only, so a schedule that opens mid-fault clears the finding immediately and
a later unoccupied period starts a fresh 30 minutes.

`persist` asserts at exactly `T + delayTime`, so the realized test is "lit and
unoccupied for strictly more than `alarm_delay`" at tick resolution. Continuous
means continuous — someone crossing the room discards the elapsed time rather
than pausing it, which is the intended trade: a full timer restart per PIR trip
means an intermittent sensor produces silence rather than a stream of 30-minute
findings. `delayOnInit = true` (CDL default `false`) makes a controller restart
into an already-lit empty building wait out the full 30 minutes.

## Possible Diagnoses

The reference's four, in its order:

1. Lighting control override active — a panel in HAND, or a BACnet
   priority-array entry holding the circuit on. Most common, cheapest to fix
2. Occupancy sensor bypassed — disconnected, taped over, or decommissioned in
   software after nuisance-switching complaints
3. Timer or photocell failure — a local astronomic timeclock that drifted or
   lost its battery, or a photocell reading a lit interior
4. BAS schedule misconfiguration — the lighting schedule never edited from the
   default, or wrong holidays and time zone (the same root cause as AHU-0018)

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT — the reference's profile.
While the fault is active the whole circuit is waste: `waste_kw =
lighting_circuit_kw`, with no thermal term and no baseline to model, which is
why confidence is HIGH on a rule this simple. EEM-18 puts occupancy-based
lighting control at 15-20% of annual lighting energy. Climate-neutral: the waste
scales with unoccupied hours, not weather. The reference's severity 4 (info)
against CRITICAL_WASTE answers a different question — every kilowatt-hour is
unnecessary, but nothing breaks and nobody is uncomfortable, so it is a work
order for the next scheduled visit rather than a callout.

## Emissions Impact

Scope 2, DIRECT_EMISSIONS, HIGH confidence; the reference's range is 500-5,000
kg CO₂e/yr, and its parenthetical is the point — "lighting waste, high MOER
overnight." Avoided-emissions basis MOER (marginal). The fault runs almost
entirely when solar is off the grid and the marginal generator is gas or coal,
so its emissions rank routinely beats its energy-cost rank in regions with cheap
overnight power.

## Deviations

- **Severity 4, from the chapter, against the family README's 3.**
  `faults/sys/README.md` lists this rule at severity 3 and its own note says the
  SYS-0001-057 rows are provisional transcriptions to be re-verified when each
  card is authored. The chapter says "Severity: 4 (info)" and wins; the README
  row needs updating by whoever owns that file.
- **`occ_scheduled` replaces the reference's schedule-evaluation call.** The
  reference writes `NOT in_occupied_schedule(current_time, occ_schedule)`, a
  function over a calendar; the block graph has no clock, so the host evaluates
  the schedule (time zone, holidays, exceptions) and feeds the boolean, as
  AHU-0018 does. `points/sys.points.json` records it as a derived point.
- **The point is named `occ_scheduled` here and `occ_schedule` in
  `points/ahu.points.json`.** One concept, two spellings. Cards bind by exact
  name within their own family dictionary so nothing breaks, but it is worth
  resolving library-wide.
- **One delay, not two.** This card's reference entry lists a single tunable,
  `AlarmDelay = 30 min`, so there is one `TrueDelay` and time-to-alarm is 30
  minutes flat. AHU-0018 carries a `grace_period` because its own entry gives
  it one; none was invented here to match the sibling's shape.
- **No thresholds, so the library's strict-comparison deviation does not apply.**
  Every input is a boolean and the graph contains no `Reals` block, so there is
  nothing to retune per binding.
- **`delayOnInit = true`** (CDL default `false`), the library's standing choice:
  a controller restart into an already-lit empty building waits out the full 30
  minutes rather than alarming on the first tick.
- **`TrueDelay` asserts at exactly `T + delayTime`,** so the realized test is
  "lit and unoccupied for strictly more than `alarm_delay`" at tick resolution.
- **The rule sees status, never power.** `lighting_circuit_kw` in
  `runtime_estimation` is a host-side nameplate or metered value; no such point
  is bound and the graph produces a boolean. Accumulation is the host's.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph. There is no NO_EVAL logic
  in the graph: it computes the fault given valid data.

## Notes

Two failure paths lead to different trades. If `occ_sensor` and `occ_scheduled`
disagree night after night, the schedule is the suspect and the fix is a BAS
edit at $0. If they agree and the circuit stays on anyway, the suspect is
downstream of the BAS — a HAND switch, a welded relay, a local timeclock — and
the fix needs an electrician.

Where this fires alongside AHU-0018 on the same nights, treat it as one
finding. The [after-hours-operation](../../../playbooks/after-hours-operation.md)
playbook is shared for that reason, and CLU-04 exists to make the master
schedule the thing that gets fixed rather than three symptoms of it.
