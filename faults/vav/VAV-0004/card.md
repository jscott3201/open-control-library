---
schema: cxf-library/fault-card/v1
id: VAV-0004
name: VAV airflow tracking error
equipment: vav
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "HVAC FDD Reference v1.0 §10, VAV-0004"
  - "Schein et al. 2006 (VPACC)"
  - "ORNL Im et al. 2025"
  - "Gunay et al. 2020"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [AHU-0020, VAV-0001, VAV-0005, FPB-0002]
playbooks: [stuck-actuator]
operating_states: "all (fan running)"
preconditions: "AHU fan running — a box cannot track a setpoint with no branch pressure behind it, and every zone on a stopped fan would report this fault. Airflow sensor and active setpoint both available and fresh; a stale setpoint held at its last value while the measurement moves reads as a tracking error that is really a communication fault. The evaluability gate is signalled in-rule by ySetpointOk: when it is false the verdict is NO_EVAL, not healthy. Boxes commissioned with the airflow sensor disabled (pressure-independent boxes converted to pressure-dependent control) must be excluded host-side — there is no setpoint to track."
points:
  - zone_airflow
  - zone_airflow_sp
outputs:
  - name: yFault
    description: True while the fractional airflow tracking error has stayed above tracking_error_threshold, at an evaluable setpoint, for tracking_duration plus alarm_delay
  - name: ySetpointOk
    description: Evaluability signal — true when zone_airflow_sp exceeds min_evaluable_setpoint; false means NO_EVAL and the host must ignore yFault
params:
  tracking_error_threshold:
    default: 0.30
    unit: "1"
    description: Fractional deviation of measured airflow from setpoint above which tracking counts as failed (0.30 = 30% of setpoint, in either direction)
    cxf: ratioHigh.t
  min_evaluable_setpoint:
    default: 50.0
    unit: L/s
    description: Airflow setpoint below which the fraction is not meaningful — at low flow the box's differential-pressure sensor is near its noise floor and a large percentage error is a small absolute one
    cxf: spOk.t
  tracking_duration:
    default: 900.0
    unit: s
    description: Continuous violation required before the tracking error counts as sustained rather than a damper stroke in progress (15 min)
    cxf: track.delayTime
  alarm_delay:
    default: 300.0
    unit: s
    description: Further persistence required after tracking_duration before the alarm asserts (5 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: VAV zone energy
  savings_range: 2-5% of zone energy; comfort is the primary impact
  climate_sensitivity: neutral
  runtime_estimation: "over-delivery only: waste_kw ≈ (zone_airflow − zone_airflow_sp) × cp × |sat − zone_temp|, applied host-side from the AHU's supply air temperature and the zone temperature"
emissions:
  scope: "1|2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:14bf3a6e9f1e4f032ceae6e45dd294f9"
  date: 2026-10-05
---

## Description

The box is not delivering the air it is being asked for. A VAV terminal is a
flow controller: the zone loop computes an airflow setpoint and the damper loop
strokes the blade until measured flow matches it, so a persistent gap means that
inner loop has lost its authority — a stuck damper, an actuator off its shaft, a
fouled flow sensor, or not enough static pressure at the branch. The test is a
fraction rather than an absolute flow so one threshold covers a 60 L/s office
and a 900 L/s conference room; that costs something at the bottom of the range,
where a box holding minimum measures a few pascals of velocity pressure and a
30% error is a handful of litres per second inside the sensor's noise, which is
why the rule carries an explicit evaluability test. Comfort is the first
casualty and energy the second, hence severity 3.

## Detection Logic

```
ratio       = |zone_airflow − zone_airflow_sp| / zone_airflow_sp
ySetpointOk = zone_airflow_sp > min_evaluable_setpoint     (false ⇒ host reports NO_EVAL)
yFault      = (ratio > tracking_error_threshold) AND ySetpointOk
              held continuously for tracking_duration, then a further alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![VAV-0004 block graph](diagram.svg)

Taking the absolute value before the division makes the test symmetric: 100 L/s
and 300 L/s against a 200 L/s setpoint are both 50% out and both report, since a
damper stuck open is as much a failure of the flow loop as one stuck shut.
`gate` carries the evaluability branch and is also what makes the unguarded
division safe — CDL `Divide` follows IEEE-754, so a zero setpoint yields ±∞ or
NaN and a near-zero denominator amplifies noise into a fraction of any
magnitude, but a denominator small enough to misbehave is below
`min_evaluable_setpoint` by construction. False `yFault` under false
`ySetpointOk` means *unknown*, not *healthy*, and the host must treat it that
way. `track` and `persist` are two delays in series, so 20 minutes of continuous
violation are required; any momentary return to setpoint drops both timers and
discards the accumulated time, so the alarm describes one continuous excursion
or nothing. Both comparisons are strict, matching the reference. Both
`delayOnInit` flags are `true`, holding the full 1200 s across a restart.

## Possible Diagnoses

1. VAV damper stuck — mechanical failure of the blade, shaft, or linkage
2. VAV damper actuator disconnected, so the loop commands into thin air while
   the blade sits wherever it was left
3. Airflow sensor fouled or failed — a lint-blocked or water-logged
   differential-pressure pickup reads low and the loop opens the damper against
   a measurement that will not move
4. Duct obstruction downstream of the box — a closed fire damper, a collapsed
   flex duct, or a balancing damper someone shut during a complaint call
5. Insufficient system static pressure at this branch, in which case the box is
   working correctly and the fault is upstream

## Energy Impact

COMFORT_ENERGY, MEDIUM confidence, PROXY_ESTIMATION. The reference puts the loss
at 2–5% of zone energy and names comfort as the primary impact. The waste term
is computable only in the over-delivery direction —
`waste_kw ≈ (zone_airflow − zone_airflow_sp) × cp × |sat − zone_temp|`, assembled
host-side since neither temperature is a rule input; under-delivery costs unmet
load and complaints instead. MEDIUM confidence: the mechanism is well
established (Schein et al. 2006's VPACC rules are the origin of this test, with
ORNL's Im et al. 2025 dataset as ground truth for stuck boxes) but the per-zone
figure depends on the direction of failure and on what the reheat coil does
about it. Climate-neutral; no PNNL measure maps to this fault. The multiplier is
the point — what matters is the fraction of a building's boxes doing this.

## Emissions Impact

Scope 1 or 2, PROXY_EMISSIONS, MEDIUM confidence; typically 50–500 kg CO₂e/yr
per zone. Which scope applies follows what the excess air is conditioned by: an
over-delivering box on a hot-water reheat coil drives on-site combustion (scope
1), while the fan moving the air and the chiller cooling it are purchased
electricity (scope 2). Avoided-emissions basis: marginal operating emissions
rate (MOER).

## Deviations

- **Evaluability is an output, not just a precondition.** The reference labels
  its low-setpoint vector NO_EVAL rather than NO_FAULT, and
  `zone_airflow_sp > min_evaluable_setpoint` is computable from this rule's own
  inputs, so SCHEMA.md requires exposing it as `ySetpointOk`.
- **Two delays in series rather than one.** The reference separates
  `tracking_duration` (15 min) from `AlarmDelay` (5 min), so both stay
  independently tunable even though a single 1200 s delay behaves identically at
  the defaults (AHU-0027's arrangement). A site that wants a shorter duration
  test changes one parameter.
- **The tracking-duration condition alone is not a fault.** If the error clears
  after `track` has matured but before `persist` has served its 5 minutes, both
  timers discard their accumulated time and a host reading `yFault` never learns
  the box was 15 minutes into the condition. Intended: the alarm describes one
  continuous excursion.
- **The division is unguarded, and the gate is what makes that safe.** AHU-0021's
  arrangement verbatim — NaN cannot raise a comparison, ±∞ and noise-inflated
  finite ratios can, and `gate` holds `yFault` down over exactly the interval
  the host is told to disregard.
- **Both comparisons are strict**, which is the reference's own notation on both
  sides rather than a substitution forced by the block set, so no measure-zero
  deviation arises. The boundary is exact rather than approximately exact:
  140 L/s against 200 L/s evaluates to precisely the same double as the 0.30
  threshold parameter.
- **`playbooks: [stuck-actuator]` is library-assigned, not transcribed.** The
  reference's card carries no playbook row, but diagnoses 1 and 2 are literally a
  stuck damper and a disconnected actuator and the playbook's procedures apply
  unmodified. Its *Applies to* row lists AHU-0020, AHU-0014 and AHU-0015;
  the addition is recorded here because `playbooks/` is single-writer.
- **`related` adds VAV-0005.** Hunting and tracking failure are the two ways a
  box's flow loop misbehaves, and a damper oscillating hard enough will also fail
  an averaged tracking test.
- **Vector tick is the library's usual 300 s.** This rule holds no windowed
  statistics — the only state is the two delay timers — so nothing couples the
  verdict to the host's tick interval, unlike VAV-0005. Both delays are exact
  multiples of 300 s.
- `track.delayOnInit` and `persist.delayOnInit` are both `true` (Modelica/CDL
  default is `false`), the library's standing choice against alarming on the
  first tick after a controller restart.
- Severity 3 (warning), phase 2, method `rule`, and the four tunable defaults are
  the reference's chapter 10 card; its §5.8.2 index carries no severity column.
  `g36: null` — Schein's VPACC work predates G36 and no §5.16 clause covers
  terminal-unit flow tracking.
- Operating states are declared, not gated: the reference marks the fault
  applicable in every state with the fan running, and the graph has nothing to
  exclude.

## Notes

This rule says the flow loop is failing, not which part. The cheapest split of
diagnoses 1–3 is to plot `zone_dmpr_pos` alongside the two flow points for a
day: a damper that never moves while the error persists is actuator or linkage
([stuck-actuator](../../../playbooks/stuck-actuator.md) step 3), a damper that
strokes its full range while the measurement barely responds is a sensor, a
plugged pickup, or an obstruction — check the sensor first, since a fouled
pickup and a collapsed flex duct look identical from the BAS and one is a
five-minute fix. Diagnosis 5 is not about this box at all: a dozen boxes short
of air on the same riser is a system problem and replacing twelve actuators will
not fix it.

`min_evaluable_setpoint` deserves a moment at commissioning. At the 50 L/s
default a box whose minimum setpoint is 40 L/s is never evaluated while it sits
at minimum, which is most of the year for an interior zone. That is deliberate —
the measurement is not trustworthy down there — but it means a damper stuck at
minimum stays invisible until the zone calls for cooling and the setpoint rises
past 50 L/s.
