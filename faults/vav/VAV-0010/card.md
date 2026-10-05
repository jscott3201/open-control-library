---
schema: cxf-library/fault-card/v1
id: VAV-0010
name: Zone temperature sensor drift (neighbor-median)
equipment: vav
status: verified
phase: 3
method: statistical
severity: 3
category: COMFORT_ENERGY
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "Yang, H., Cho, S., Tae, C.-S., Zaheeruddin, M. (2008). Sequential rule based algorithms for temperature sensor fault detection in air handling units. Energy Conversion and Management 49(8), 2291-2306. doi:10.1016/j.enconman.2008.01.029 — drift as a distinct sensor-fault class detected by rule-based comparison against a reference, and the isolation caveat this card inherits"
  - "Library-authored: no reference card exists for a neighbor-comparison drift rule; name, severity, thresholds and the median reference are argued here. This is the rule VAV-0008's diagnosis 3 and faults/vav/README.md have pointed at since the CUSUM batch"
  - "Library precedent: SYS-0005 (pairwise cross-validation, ambiguous verdict — this card is its fleet-referenced descendant with a NAMED verdict), SYS-0009/SYS-0010 (single-sensor adjudication form), HP-0001 (commissioning parameters shipped as documented placeholders)"
  - "points/vav.points.json zone_temp_neighbor_median — the self-excluding median contract (population >= ~5 zones, divergent-purpose zones excluded) this rule stands on"
g36: null
clusters: [CLU-09]
suppresses: []
suppressed_by: []
adjudicates:
  points: [zone_temp]
  verdict: invalid_while_active
related: [SYS-0005, SYS-0009, SYS-0010, VAV-0008]
playbooks: [sensor-drift]
operating_states: "occupied, host-enforced. Unoccupied setback diverges zones legitimately — different masses recover at different rates — so the comparison means something only while every zone is holding an occupied setpoint. This rule is stateless, so the gate is the host's standard job (contrast the VAV-0007/0008/0009 accumulators, which had to reset in-graph)."
preconditions: "The median contract does the heavy lifting — read points/vav.points.json zone_temp_neighbor_median before instantiating: the median EXCLUDES this zone (a drifting sensor must not pull its own reference), covers >= ~5 sibling zones on the same AHU, and omits zones whose setpoints legitimately diverge (server rooms, vestibules, unconditioned buffers). drift_threshold must exceed the site's occupied SETPOINT SPREAD plus normal zone-to-zone scatter: zones commanded to different setpoints differ by design, and a threshold inside that spread alarms on the design. Delivery quality is resolved before this rule runs — a held-over stale value reads as divergence, and the rule is right about the number and wrong about the sensor (SYS-0005's caveat, inherited). Per the sensor-health family's standing constraint, no other card may list this rule in its suppresses."
points:
  - zone_temp
  - zone_temp_neighbor_median
outputs:
  - name: yFault
    description: True while this zone's temperature has stayed more than drift_threshold from its neighbor median, in either direction, continuously for persist_time. The verdict names THIS zone's sensor — see adjudicates
  - name: yHigh
    description: "Sub-condition flag, undelayed — the zone reads above the median by more than drift_threshold right now. Not an evaluability output; false never means NO_EVAL"
  - name: yLow
    description: "Sub-condition flag, undelayed — the zone reads below the median by more than drift_threshold. Same kind as yHigh"
params:
  drift_threshold:
    default: 3.0
    unit: "°C"
    description: "Divergence from the neighbor median beyond which this zone is the outlier. COMMISSIONING PLACEHOLDER: it must clear the site's occupied setpoint spread plus honest zone-to-zone scatter, which no source publishes as a portable number. 3.0 is deliberately wider than SYS-0005's 2.0 pair band — a median of different rooms is a looser reference than a co-located pair. One value feeds both directions through the in-graph negation."
    cxf: [highCmp.t, lowCmp.t]
  persist_time:
    default: 7200.0
    unit: s
    description: "Continuous divergence required before the alarm asserts (2 h). Drift is permanent, so latency is cheap and false positives are not: two hours rides out lunch loads, solar swings, and a propped-open door. delayOnInit = true serves the full persistence on a controller restart."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Zone sensing accuracy — every control decision and every VAV diagnostic on this zone is downstream of this sensor
  savings_range: "sensor-dependent; the recalibration itself is near-free, and the recovered waste belongs to whatever the wrong reading was driving (a zone held 2 °C cold buys cooling nobody asked for)"
  climate_sensitivity: neutral
  runtime_estimation: "none — no direct waste term. The cost of a drifted zone sensor is the conditioning bought against a false reading and the diagnostic coverage lost while it is believed; both are accounted for by the rules this one adjudicates (Energy Impact Reference §4.4, SYS-0005's treatment)"
emissions:
  scope: "1|2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:633c71d93a7cb6a8f5f86e6f18b81077"
  date: 2026-10-05
---

## Description

A zone temperature sensor that drifts takes its whole zone with it: the box
dutifully conditions the room to a wrong number, the occupants adjust the
setpoint to fight it, and every VAV diagnostic on that zone inherits the lie.
A single drifted sensor is invisible from inside its own control loop — the
loop closes on the sensor, so the trend looks healthy. What exposes it is the
rest of the fleet: sibling zones on the same air handler see the same supply
air and roughly the same weather, so a zone that reads persistently far from
its neighbors' median is the outlier, and the median — unlike SYS-0005's
two-sensor pair — says *which* sensor to distrust.

## Detection Logic

```
err     = zone_temp − zone_temp_neighbor_median

yHigh   = err  > drift_threshold          this zone reads above the fleet
yLow    = −err > drift_threshold          below the fleet (in-graph negation)

yFault  = (yHigh OR yLow) continuously for persist_time
```

Block graph (`rule.cxf.jsonld`):

![VAV-0010 block graph](diagram.svg)

Six blocks, stateless. The median is computed by the host per the point
contract — self-excluding, so a drifting sensor cannot drag its own
reference toward itself. Trunk-level events are free immunity: a failed AHU
or a building-wide swing moves the zone *and* the median together, and the
subtraction sees only the residual (`median_moves_with_zone` pins it). The
Or sits before the single persistence delay, so a sensor that flips from
reading high to reading low without ever closing is one continuous fault.
Both comparisons are strict; equality at the threshold reads healthy.

## Possible Diagnoses

1. **Sensor drift or failed calibration** — the finding this card names.
   Verify with a handheld reference at the thermostat; recalibrate or
   replace, per the [sensor-drift](../../../playbooks/sensor-drift.md)
   playbook.
2. **Bad placement rather than bad electronics** — a sensor over a copier,
   in supply-air wash, or on a sun-struck wall drifts with the source, not
   with age. Same signature, different work order: move it.
3. **A genuinely divergent zone that belongs out of the population** — a
   space repurposed since commissioning (new server load, new wall). The fix
   is the median population list, not the sensor.
4. **The zone really is out of control** — starved airflow or a failed box
   holds the space off setpoint while the sensor tells the truth. The VAV
   family's own rules (VAV-0004, VAV-0008) fire alongside this one in that
   case; this card alone, with the box quiet, points at the sensor.

## Energy Impact

COMFORT_ENERGY, LOW confidence, QUALITATIVE_ONLY. The drifted sensor itself
consumes nothing; the waste is whatever the wrong number drives — typically
over-conditioning of one zone and setpoint-fighting by its occupants — and
that is counted by the downstream rules this card adjudicates. Confidence is
LOW on SYS-0005's grounds: the mechanism is literature-backed but the shipped
thresholds are placeholders with no fault-injection validation behind them
yet. Climate-neutral.

## Emissions Impact

Scope 1 or 2 by what the false reading drives (reheat vs cooling),
QUALITATIVE_EMISSIONS. No direct term; abatement rides the downstream fix.

## Deviations

- **The median names the verdict; the pair could not.** SYS-0005 ships
  `verdict: ambiguous` because two sensors that disagree carry no majority.
  Five or more siblings do: this card adjudicates `zone_temp` as
  `invalid_while_active`, the single-sensor form SYS-0009/0010 established.
  The cost is the median contract itself — population size and composition
  are host obligations the graph cannot check.
- **drift_threshold 3.0 °C and persist_time 7200 s are library defaults, not
  literature values.** Yang et al. (2008) validate pairwise comparison
  thresholds on co-located AHU sensors; no source publishes a
  neighbor-median band. Shipped wider (3.0 vs the pair's 2.0) and slower
  (2 h single delay vs the pair's chained 60 + 30 min) because rooms are a
  looser reference than a shared duct; both retune at commissioning against
  the measured occupied spread.
- **One delay, not SYS-0005's chained two.** The reference specified that
  card's `drift_duration` + `AlarmDelay` split, so it was transcribed; this
  card has no source to honor and one `persist_time` is one fewer number to
  commission wrong.
- **The negation is in-graph** (`MultiplyByParameter · k = −1`) so the one
  published threshold stays positive and feeds both comparators — the same
  reason VAV-0008 keeps its constants positive rather than shipping a
  negative the retuner must remember.
- **Host-side occupancy gating, against the batch-19 precedent and for the
  house default.** The CUSUM trio gated in-graph because accumulators need
  their STATE reset; this rule is stateless, so `operating_states:
  occupied` does the job with zero blocks. The persistence delay does run
  through unoccupied hours — a 3 a.m. assertion is possible and the host
  discards it as NO_EVAL, which is suppression of output, not state, and
  therefore safe here.
- **`delayOnInit = true`** (CDL default `false`), the library's standing
  choice, does real work: a controller restart serves the full two hours
  before re-alarming rather than re-asserting into a zone that may have
  been fixed.
- **Severity 3, `category: COMFORT_ENERGY`, name** are library-authored;
  mirrored from SYS-0005, the nearest shipped relative. `g36: null` — G36
  has no fleet-relative sensor check.

## Notes

This closes the reservation VAV-0008's Notes and the family README have
carried since batch 19: the drift rule that decides whether the CUSUM
cards' `zone_temp` input can be believed. Run it beside them — a zone that
trips VAV-0008 *and* this card is a sensor problem wearing a comfort
problem's clothes.
