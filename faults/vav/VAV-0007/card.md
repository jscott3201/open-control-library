---
schema: cxf-library/fault-card/v1
id: VAV-0007
name: VAV airflow tracking CUSUM
equipment: vav
status: verified
phase: 3
method: statistical
severity: 3
category: COMFORT_ENERGY
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.1 — the normalized statistic (eq. 1) and the two-sided CUSUM recursion (eqs. 2-3)"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.3 — CFMerror = CFMactual − CFMsetpoint, the fault classes it detects, and the occupied-only / first-hour-excluded computation rule"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.4 (point requirements), §5.1.5 (threshold-selection protocol), §5.2.2 Table 4 (normal-operation error statistics), §5.2.3 and Table 5 (k = 3 on the Iowa Energy Center data; alarm-limit ranges)"
  - "Calibration method: tools/simharness/harness.py `vavcal` mode and tools/simharness/README.md"
  - "Library precedent: SYS-0010 (Discrete.UnitDelay sample grid and seed behaviour), SYS-0006 (sub-condition flags), HP-0001 and VAV-0001 (per-binding placeholder parameters)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [VAV-0002, VAV-0004, VAV-0008, VAV-0009]
playbooks: [stuck-actuator, vav-min-flow-reheat]
operating_states: "all (fan running) — the box's flow loop is supposed to track its setpoint in every mode, and the rule gates itself on occupancy rather than on mode"
preconditions: "AHU fan running: with no branch pressure the box cannot track anything and every zone accumulates at once. zone_airflow_sp must be the ACTIVE setpoint the damper loop is tracking (the dictionary's own contract) — a stale or wrongly-bound setpoint produces a real, sustained, entirely fictitious bias, and CUSUM is built to find exactly that. sigma_flow_error is a COMMISSIONING parameter in L/s and does not travel between boxes: it scales with box size, so a 2000 L/s box and a 200 L/s box differ by about 10× and the shipped default is one 280 L/s box's number (see Deviations). Commission it from normal-operation data that INCLUDES ordinary setpoint activity, per §5.1.5 — a sigma fitted on steady-state samples only makes every damper stroke look like a fault. sample_period MUST equal the host's tick interval, and the alarm limit is a per-sample quantity: halve the tick and the same bias alarms in half the wall-clock time. Pressure-dependent boxes and boxes commissioned with the flow sensor disabled must be excluded host-side — there is no setpoint to track. Read yOccupiedOk before yFault: while it is false the accumulators are held at zero and the verdict is NO_EVAL, not a healthy box."
points:
  - zone_airflow
  - zone_airflow_sp
  - occ_scheduled
outputs:
  - name: yFault
    description: True while either accumulator sits above alarm_limit_h — the box's flow error has leaned one way for long enough that the accumulated evidence exceeds the alarm limit
  - name: yHigh
    description: "Sub-condition flag — the positive (S) accumulator is above the limit: the box is delivering more air than asked. Diagnostic detail only; a false yHigh never means NO_EVAL"
  - name: yLow
    description: "Sub-condition flag — the negative (T) accumulator is above the limit: the box is starving its zone. Not an evaluability output either"
  - name: yOccupiedOk
    description: Evaluability signal — true once occ_scheduled has held true for occupied_warmup. While false both accumulators are forced to zero and no verdict exists; false means NO_EVAL and the host must ignore yFault
params:
  mean_flow_error:
    default: 0.0
    unit: L/s
    description: "Mean flow error under normal operation, subtracted before normalizing. The source measured 0 CFM over four boxes and eight days (§5.2.2 Table 4), so the shipped default is zero; a box with a known standing offset carries it here rather than letting the offset accumulate."
    cxf: meanC.k
  sigma_flow_error:
    default: 2.36
    unit: L/s
    description: "Standard deviation of the flow error under normal operation — the scale everything else is measured in. The source's 5 CFM (§5.2.2 Table 4) converted to L/s, measured on ~280 L/s Iowa Energy Center boxes at one-minute sampling. PER-BOX COMMISSIONING VALUE: it scales with box size and with the flow sensor's own noise, so a large box needs its own number and an unretuned default there is silently 10× too small. Floor it above the flow sensor's resolution — the graph divides by this constant unguarded, and a zero or near-zero sigma produces an infinite or noise-dominated z."
    cxf: sigmaC.k
  slack_k:
    default: 3.0
    unit: "1 (multiples of sigma)"
    description: "Slack parameter: the part of each sample's normalized error that is written off as normal variation before anything accumulates. The source set k = 3 for its Iowa Energy Center charts (§5.2.3), so only excursions past 3 sigma build evidence at all — at the shipped sigma that is a 7.1 L/s sustained bias before the rule can ever fire."
    cxf: kC.k
  alarm_limit_h:
    default: 20.0
    unit: "1 (accumulated sigma-samples)"
    description: "Alarm limit both accumulators are compared against. The source's Table 5 gives 3 to 180 for the S side and 3 to 100 for the T side as the range that caught every injected fault with no false alarms; 20 sits near the middle of the overlap in log terms — about 7× the largest healthy excursion its Table 6 recorded, and 5× inside the tightest detection bound. Dimensionless, so it transfers across box sizes once sigma is commissioned per box. One card parameter drives both comparators and hosts must set them together; a site wanting the source's asymmetric limits sets the two block paths separately."
    cxf: [sHigh.t, tHigh.t]
  sample_period:
    default: 60.0
    unit: s
    description: "The accumulators' sample interval — one tick of feedback through Discrete.UnitDelay. MUST equal the host's tick interval; at any other ratio the delayed accumulator value is between one and two periods old and the recursion stops being the source's. It also sets the clock the alarm limit is denominated in: h is evidence per sample, so the same bias alarms in half the wall-clock time at half the tick. The shipped 60 s is the interval the source's own statistics and limits were measured at (§5.2.2). One card parameter, two block paths, always set together."
    cxf: [sPrev.samplePeriod, tPrev.samplePeriod]
  occupied_warmup:
    default: 3600.0
    unit: s
    description: "How long occ_scheduled must hold true before the accumulators are allowed to run. The source treats the first hour of each occupied period as unoccupied so steady-state conditions can develop (§5.1.3); this is that hour, and it also covers the morning warm-up excursion that would otherwise be the day's largest accumulation."
    cxf: occGate.delayTime
energy_impact:
  affected_subsystem: VAV zone energy
  savings_range: 1-3% of zone energy per affected box; the finding is early detection rather than a large instantaneous waste term
  climate_sensitivity: neutral
  runtime_estimation: "over-delivery only, and small by construction: waste_kw ≈ (zone_airflow − zone_airflow_sp) × cp × |sat − zone_temp|, assembled host-side exactly as VAV-0004 does. This rule fires on biases below that rule's threshold, so the per-box number is correspondingly smaller and the case for fixing it is the multiplier across a building's boxes plus the head start before the miss grows"
emissions:
  scope: "1|2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:e234d75706a2e9484f7249ffa215c442"
  date: 2026-10-05
---

## Description

The box tracks its airflow setpoint slightly wrong, and keeps doing it. A damper
caught part way through its stroke, a differential-pressure pickup starting to
foul, a flow loop tuned soft enough to sit permanently a little off target — none
of these produce the 30% miss VAV-0004 waits for, and all of them are faults.
This rule accumulates the error instead of thresholding it. Each sample's flow
error is normalized against the box's own commissioned noise, the part of it that
looks like ordinary variation is written off, and whatever is left is added to a
running sum that grows only while the error keeps leaning the same way.
Symmetric noise never builds; a small persistent bias always does. Two sums run,
one per direction, so a box over-delivering and a box starving its zone arrive as
different findings. This is the airflow channel of NIST's VPACC, the first of
three (VAV-0008 temperature, VAV-0009 reheat-coil ΔT).

## Detection Logic

```
CFMerror    = zone_airflow − zone_airflow_sp
z           = (CFMerror − mean_flow_error) / sigma_flow_error
yOccupiedOk = occ_scheduled held continuously true for occupied_warmup   (false ⇒ host reports NO_EVAL)

while yOccupiedOk:   S = max(0, z − k + S_prev)     T = max(0, −z − k + T_prev)
otherwise:           S = 0                          T = 0

yHigh = S > alarm_limit_h     yLow = T > alarm_limit_h     yFault = yHigh OR yLow
```

Block graph (`rule.cxf.jsonld`):

![VAV-0007 block graph](diagram.svg)

The library's first feedback accumulator: `sReset`/`tReset` feed
`Discrete.UnitDelay` instances whose outputs return to `sSum`/`tSum`, which the
engine accepts because a unit delay's output at tick *i* is its input at tick
*i−1* by construction and so cuts the direct-feedthrough graph. `sample_period`
must therefore equal the host's tick, and the delay's `y_start` is left at the
CDL default of 0.0 — which is the correct initial accumulator value, so this
rule needs no warmup mask.

There is no persistence timer, because `alarm_limit_h` is the timer: time enters
through accumulated evidence rather than through a dwell on a boolean. That cuts
both ways on the falling edge. A rule with a `TrueDelay` drops the moment its
condition clears; this one drains at `slack_k` per sample from wherever the
accumulator got to, so an alarm outlives the fix by however long the sum takes to
fall back through the limit — minutes for a small bias, an hour for a large one.
The comparison is strict and the graph divides by `sigma_flow_error` unguarded,
which is safe only because that divisor is a commissioned constant with a
documented floor and not a live signal.

## Possible Diagnoses

Per §5.1.3, the CUSUM of CFMerror detects stuck dampers, differential-pressure
sensor faults, and unstable airflow control.

1. Damper stuck or actuator slipping — the classic case, and the direction flags
   split it: `yHigh` on a box that will not close down, `yLow` on one that will
   not open up
2. Differential-pressure flow sensor drifting or partly plugged — a fouling
   pickup reads low, the loop opens against it, and the *measured* error is small
   and one-signed for weeks, which is precisely the signature a threshold rule
   cannot see
3. Unstable or badly retuned flow control — a loop that overshoots symmetrically
   leaves both sums at zero, but one that limit-cycles around an offset does not
4. A mis-bound or stale `zone_airflow_sp` — a real sustained error against a
   number nobody is controlling to. Check the binding before the box
5. Branch static pressure chronically short of what this box needs, in which case
   the box is behaving correctly and the finding belongs upstream

## Energy Impact

COMFORT_ENERGY, MEDIUM confidence, PROXY_ESTIMATION. The mechanism is
VAV-0004's and so is the waste term — over-delivered air costs
`(zone_airflow − zone_airflow_sp) × cp × |sat − zone_temp|`, assembled host-side
because neither temperature is an input here; under-delivery costs unmet load and
complaints instead. What differs is scale: this rule fires on biases below the
threshold rule's 30%, so each box's number is small and the case for the work
order is the count of boxes and the months of drift caught early rather than the
instantaneous kilowatts. Confidence is MEDIUM because the method is validated on
real VAV boxes (the source detected all three of its injected faults with no
false alarms) while every parameter is per-site. Climate-neutral.

## Emissions Impact

Scope 1 or 2, PROXY_EMISSIONS, MEDIUM confidence. Which applies follows what
conditions the excess air: a hot-water reheat coil working against an
over-delivering box is on-site combustion, while the fan moving the air and the
chiller cooling it are purchased electricity. Avoided-emissions basis: marginal
operating emissions rate (MOER). Per-box quantities are small; the population is
not.

## Deviations

- **The shipped k and h come from the source's real-data section, not from its
  illustration.** Figure 9's k = 0.5, h = 5 is a synthetic demonstration on
  random normal data. §5.2.3 states that k was set to three for the Iowa Energy
  Center charts, and Table 5 publishes alarm-limit ranges per channel measured
  against injected faults. The airflow channel's own numbers are what ship.
- **`alarm_limit_h` defaults to 20, which is a library choice inside the
  source's interval.** Table 5 gives 3–180 (S) and 3–100 (T); Table 6's healthy
  peaks over seven zone-days are ≤ 3. Twenty is near the geometric centre of the
  overlap — roughly 7× the observed healthy ceiling and 5× below the tightest
  detection bound — and the source is explicit that its own numbers are
  preliminary and site-dependent.
- **One `h` for both directions, though the source's ranges are asymmetric.** Its
  S range runs to 180 and its T range to 100, and other channels' T sides are
  marked "not used" entirely. A single parameter keeps the common case honest and
  the two block paths stay individually settable for a site that wants the split.
- **`sigma_flow_error` ships as a per-box placeholder in the strong sense.** The
  5 CFM of Table 4 was measured on four ~280 L/s boxes; flow-sensor noise scales
  with the box, so the default is a worked example rather than a portable
  constant, and left unretuned on a 2000 L/s box it makes every ordinary
  fluctuation a 10 sigma event. Same contract as HP-0001's fitted baseline and
  VAV-0001's ventilation requirement, and the point dictionary is where a host
  looks for the binding.
- **The simulation harness cannot supply this channel's statistics, and does not
  pretend to.** `vavcal` computes normal-operation mean and standard deviation
  for the temperature and reheat-ΔT channels only. A modelled terminal's airflow
  is a solved quantity with no damper-loop dynamics and no DP-sensor noise in it,
  so a simulated CFMerror would report solver residuals rather than the
  quantity being calibrated. The airflow channel's defaults are therefore the
  source's own field measurements, and commissioning per §5.1.5 remains
  mandatory.
- **Occupancy is a bound point, not a host-side precondition.** The library's
  stance keeps gating in the host, but CUSUM's semantics require the accumulator
  to be *zeroed* at the start of each occupied period rather than merely ignored:
  a frozen non-zero sum would carry last night's history into this morning. The
  `Reals.Switch` pair selecting zero is the only way to express that in-graph.
  Precedent for occupancy as an ordinary input: AHU-0026, SYS-0008.
- **The first-hour exclusion is a `TrueDelay` with `delayOnInit = true`**, so it
  is served after every occupied-period start and after a controller restart. The
  source's reason is settling to steady state; it also covers the morning warm-up
  excursion, which would otherwise be the largest single accumulation of the day.
- **`y_start` is left at the CDL default of 0.0 and no warmup gate is needed** —
  the opposite call from SYS-0010, and for a good reason rather than
  inconsistency. There the seed fabricated a step against a live reading; here
  zero *is* the correct initial accumulator, so the delay's two-sample seed
  persistence costs at most one sample of accumulation and can never invent a
  fault.
- **`sample_period` and the alarm limit are coupled through the tick.** The
  accumulators advance once per sample, so h is evidence per sample and
  time-to-alarm scales inversely with the tick rate. A host moving from 60 s to
  300 s ticks keeps the same detection floor but takes five times as long, and
  the fix is to re-derive h, never to leave it alone and assume the rule is
  unchanged. Both `UnitDelay` instances take the value together through the
  list-form `cxf` path.
- **No persistence timer on the fault path**, unlike almost every other card
  here. Adding one would be redundant against evidence that is already
  time-integrated, and would delay a finding the accumulator has already
  established.
- **The alarm drains rather than latching or clearing cleanly.** Once the error is
  corrected the sum falls by k per sample and `yFault` stays true until it passes
  back below h — 23 minutes in the pinned vector. Hosts should treat the assert
  as the event and hold the work order, not track the falling edge.
- **Strict `>` on both limits, bracketed rather than pinned.** The vectors walk
  the accumulator across h one unit per sample and assert on the samples either
  side, because a normalized running sum is a computed double that cannot be
  parked exactly on the limit — AHU-0022's argument, and its arithmetic here is
  off by an ulp per sample.
- **Overlap with VAV-0004 is complementary, and neither suppresses the other.**
  053 catches a big fast miss on one look (30% of setpoint, sustained 20 min);
  this catches a small one that never clears — at the shipped parameters, roughly
  7 to 60 L/s on a 200 L/s box, a band 053 is structurally blind to. A box far
  enough out will trip both, which is a corroboration rather than a duplicate.
- **The library's units are the dictionary's, so the source's CFM figures are
  converted.** 5 CFM → 2.36 L/s at 0.4719 L/s per CFM; k and h are dimensionless
  and cross unchanged. Hosts on CFM convert before binding, per SCHEMA.md.
- **`points` needs nothing new.** §5.1.4 names room temperature, both zone
  setpoints, airflow setpoint, actual airflow and occupancy as points already in
  the local controller; this channel uses three of them and adds no
  instrumentation. VAV-0009's `vav_dat` is the family's only new point.
- **`playbooks` lists `stuck-actuator` first, which this batch's assignment did
  not.** The card was assigned `vav-min-flow-reheat` with the rest of the family,
  and it stays; but diagnoses 1 and 2 are a stuck damper and a failing flow
  sensor, which is `stuck-actuator`'s subject and VAV-0004's binding for the
  same reasons. Both are recorded here because `playbooks/` is single-writer.
- **Severity 3, phase 3, `method: statistical`, `g36: null`.** Severity follows
  VAV-0004, the threshold rule on the same signal; phase 3 and the 1xx band are
  SCHEMA.md's for advanced statistical rules, matching the reserved VAV-0010.
  VPACC predates G36 by two decades and no §5.16 clause covers terminal flow
  tracking.
- **The direction flags are drawn as neutral pills, not red ones.** SYS-0006
  and SYS-0008 draw sub-condition flags in the fault colour, which reads as
  three alarms leaving the graph; amber stays reserved for evaluability
  (`yOccupiedOk`). Worth reconciling library-wide rather than per card.
- Operating states and the remaining preconditions are declared in frontmatter
  for host enforcement rather than encoded in the block graph, per the library's
  design stance.

## Notes

Commission `sigma_flow_error` on data that includes ordinary setpoint activity,
not on a quiet afternoon. Every setpoint change produces a genuine tracking
excursion while the damper strokes, and a sigma fitted without any of them makes
those excursions look like evidence. The source's own boxes, sampled every
minute through real occupied days, peaked at an accumulated 3 — so on a
reasonably tuned box the strokes cost nothing, but that is a fact about their
boxes and their sampling interval, and it is the first thing to check when a
newly deployed instance alarms on its first day.

Read the direction flags before dispatching. `yLow` with a damper reading full
open is a starved branch or a plugged flow pickup; `yHigh` with a damper reading
closed is an actuator off its shaft or a flow sensor reading low against a
correctly positioned blade. Both are cheaper to tell apart from a day of trend
than from a ladder.
