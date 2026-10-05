---
schema: cxf-library/fault-card/v1
id: VAV-0009
name: Reheat coil leakage CUSUM
equipment: vav
status: verified
phase: 3
method: statistical
severity: 3
category: CRITICAL_WASTE
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.1 — the normalized statistic (eq. 1) and the two-sided CUSUM recursion (eqs. 2-3)"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.3 — dTerror = discharge minus supply temperature, computed only while the reheat coil is commanded off; the leaking valve/element attribution"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.4 — point requirements, and the entering-air ≈ supply-air workaround this card adopts where the box has no entering-air sensor"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.1.5 — slack from normal-operation data, alarm limits from fault-injection data, collected per VAV box type"
  - "Bushby, Castro, Schein, House (2001), NIST/CEC PIER Project 2.3, §5.2 — Iowa Energy Center validation: the stuck/leaking-valve studies measured discharge rises of 3.7-11.9 °F, and Table 5 reports k = 3 with the campaign's alarm-limit ranges"
  - "Harness calibration (committed method): tools/simharness/harness.py `vavcal` mode and tools/simharness/README.md — healthy reheat-off dTerror measured across the B2B OfficeMedium-4004 zones, and the supply-broadcast approximation bias validated below 0.005 °C"
  - "Library-authored: the HVAC FDD Reference v1.0 publishes no CUSUM card, so name, severity, category and the shipped defaults are argued on this card"
  - "Sibling precedent: VAV-0007/VAV-0008 (this batch's recursion and reset topology), SYS-0006 (sub-condition flags), VAV-0003 (the valve seen from the command side)"
  - "Engine pin e2ff2f8: crates/oce-blocks/src/discrete.rs (UnitDelay sample grid, loop-cut contract), crates/oce-graph/src/topo.rs (the emit-before sort that admits the feedback path)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [VAV-0003, VAV-0007, VAV-0008, FCU-0002, FPB-0003]
playbooks: [vav-min-flow-reheat]
operating_states: "occupied with the reheat coil commanded off, both gated in-graph. While the schedule is unoccupied, for exclusion_time after it goes occupied, or while rht_vlv_cmd is at/above reheat_closed_threshold, the accumulators are forced to zero. yOccupiedOk and yReheatOk publish the two gates separately."
preconditions: "Five host obligations. (1) This is the one VPACC channel that needs a discharge-air sensor: many-but-not-all boxes have one (points/vav.points.json vav_dat), and a box without it runs the source's reduced two-channel VPACC (VAV-0007/VAV-0008) rather than a substitute signal. (2) `sat` is the serving AHU's supply-air temperature broadcast to the box, standing in for entering-air per §5.1.4's own workaround — bind the loop actually serving this box; the committed harness method measured the approximation bias below 0.005 °C in simulation, but a long or leaky duct run raises the healthy baseline, which is what error_mean absorbs and why it is commissioned per box. (3) rht_vlv_cmd is the COMMAND, not position feedback — a leak with the valve commanded open is invisible here by construction and belongs to VAV-0003. (4) Tick on the sample_period grid; both unit delays advance on that clock. (5) error_mean, error_sigma, slack_k and alarm_limit_h are per-box-type commissioning values (§5.1.5); the shipped set is one simulated medium-office box plus the source's Iowa campaign. yOccupiedOk and yReheatOk are evaluability flags: either false means NO_EVAL, not healthy."
points:
  - vav_dat
  - sat
  - rht_vlv_cmd
  - occ_scheduled
outputs:
  - name: yFault
    description: True while either cumulative sum is strictly above alarm_limit_h — the discharge has run away from the supply temperature, with the coil commanded shut, for long enough that the accumulated normalized error passed the limit
  - name: yHigh
    description: "Sub-condition flag, undelayed — the S (upper) chart is above the limit: heat is being added across a coil commanded off, the leak direction. Not an evaluability output; a false yHigh never means NO_EVAL"
  - name: yLow
    description: "Sub-condition flag, undelayed — the T (lower) chart is above the limit: the discharge reads persistently BELOW the supply broadcast, which no leak can produce. An instrumentation finding (wrong loop bound, sensor swap or drift), not a coil finding"
  - name: yOccupiedOk
    description: "Evaluability flag — true only while the schedule is occupied AND exclusion_time has elapsed since it went occupied. FALSE MEANS NO_EVAL: the accumulators are held at zero by construction"
  - name: yReheatOk
    description: "Evaluability flag — true only while rht_vlv_cmd is strictly below reheat_closed_threshold. FALSE MEANS NO_EVAL: with the coil legitimately commanded open, a discharge rise is the coil doing its job and the accumulators are held at zero"
params:
  error_mean:
    default: 0.44
    unit: "°C"
    description: "Expected reheat-off rise, eq. 1's x-bar — fan heat and duct gain make a small positive dT normal, so zero would be wrong here. 0.44 °C is the committed harness measurement (vavcal, B2B OfficeMedium-4004, occupied reheat-off ticks); commission it per box, since fan type, duct run and box size all move it."
    cxf: meanC.k
  error_sigma:
    default: 0.39
    unit: "°C"
    description: "Normalization denominator, eq. 1's sigma-hat. Unlike VAV-0008's floor, this channel has a real healthy spread to measure and 0.39 °C is the same harness measurement — but simulation noise is cleaner than real sensors, so treat it as a starting point, not a statistic about your box."
    cxf: sigmaC.k
  slack_k:
    default: 3.0
    unit: "1 (standard deviations per sample)"
    description: "Slack. The normalized error must exceed k before anything accumulates. 3.0 is what Table 5 reports for the source's Iowa Energy Center campaign — measured, unlike the Figure-9 illustration the sibling cards ship — and still a commissioning parameter per §5.1.5. One constant feeds both charts."
    cxf: kC.k
  alarm_limit_h:
    default: 100.0
    unit: "1 (accumulated standard deviations)"
    description: "Alarm limit, sitting inside Table 5's published range for the campaign. At the shipped defaults a 5 °C leak rise adds about 8.7 per minute and alarms in roughly 12 minutes; the drain back through the limit after repair takes as long as the slack allows. One parameter binds both charts; split sHigh.t and tHigh.t deliberately if a site wants the source's per-chart limits."
    cxf: [sHigh.t, tHigh.t]
  reheat_closed_threshold:
    default: 1.0
    unit: "%"
    description: "The command level below which the coil counts as off and the channel is armed (strict less-than). Set it under the controller's minimum crack; a box that never commands fully closed never evaluates this rule, and yReheatOk says so."
    cxf: rhtOff.t
  sample_period:
    default: 60.0
    unit: s
    description: "The CUSUM clock — both unit delays advance on this grid, one increment per period. SET IT TO THE HOST'S TICK INTERVAL and set both paths together. 60 s matches the source's own 1-minute data (§5.2)."
    cxf: [sPrev.samplePeriod, tPrev.samplePeriod]
  exclusion_time:
    default: 3600.0
    unit: s
    description: "How long after each occupied-period start the accumulators stay held at zero — the source's first hour (§5.1.3), taken literally. delayOnInit = true also covers a controller restart."
    cxf: occGate.delayTime
energy_impact:
  affected_subsystem: VAV reheat coil passing heat while commanded off — heating energy added, then removed again by the cooling plant
  savings_range: "not estimated in-rule; the rise itself is the proxy — dT × airflow prices the leak once the host multiplies in the box's flow, which this rule deliberately does not consume"
  climate_sensitivity: "heating-plant-season biased: the leak needs a hot hydronic loop (or a live electric element) behind the valve, so plants that isolate heat in summer bound the exposure"
  runtime_estimation: "none in-rule — PROXY_ESTIMATION. This card supplies leak-hours and the rise magnitude's persistence; VAV-0003 carries the priced version of the same waste when the valve is driven rather than passing"
emissions:
  scope: "1|2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:b1d17eced97fea52d58a97a364eee7bf"
  date: 2026-10-05
---

## Description

A reheat valve that will not quite close is the quietest fault a VAV box can
have: the zone stays comfortable, the damper compensates with more cold primary
air, and the only witness is a discharge temperature a degree or two above the
supply air feeding the box. That rise is real heat, paid for twice — once at
the boiler and again at the chiller that removes it. No single reading is worth
an alarm, because fan heat and duct gain put a small legitimate rise across
every box; what convicts the coil is a rise persistently above that baseline,
every minute, with the valve commanded shut. A cumulative sum chart is built
for exactly that shape of evidence.

## Detection Logic

```
dTerror  = vav_dat − sat                       rise across the box, °C

z_i      = (dTerror_i − error_mean) / error_sigma

armed_i  = occ_scheduled held true for exclusion_time
           AND rht_vlv_cmd < reheat_closed_threshold

S_i      = armed_i ? max(0,  z_i − slack_k + S_{i−1}) : 0
T_i      = armed_i ? max(0, −z_i − slack_k + T_{i−1}) : 0

yHigh    = S_i > alarm_limit_h        yLow = T_i > alarm_limit_h
yFault   = yHigh OR yLow
```

Block graph (`rule.cxf.jsonld`):

![VAV-0009 block graph](diagram.svg)

The recursion, the occupancy arming and the feedback-through-`UnitDelay`
topology are VAV-0008's, documented there; this card adds one gate. A single
`Logical.And` of the occupancy arm and the reheat-off comparison drives both
reset switches, so the accumulators hold state only while the coil is commanded
off — the one condition under which a discharge rise means anything. When the
coil is commanded open, the state is cleared, not frozen (see Deviations).
Both threshold comparisons are strict, and there is no alarm delay: the
accumulation is the persistence test.

## Possible Diagnoses

§5.1.3 attributes this channel to a leaking valve or element; the gate and the
direction flags narrow it further.

1. **Reheat valve passing** — worn seat, debris, or actuator not driving fully
   closed. The classic `yHigh` finding: rise within the source's measured
   3.7–11.9 °F leak band, coil commanded shut. Verify at the coil: pipe surface
   temperature downstream of the valve tells the truth in one visit.
2. **Electric reheat element held on** — welded contactor or failed SCR, the
   electric-box equivalent of a passing valve, same signature and a bigger
   safety question. FCU-0002 is the fan-coil sibling.
3. **Valve driven open by override or bad sequence** — the command reads closed
   at the BAS but a local override or miswired output holds it open. The rule
   cannot tell this from a leak; the work order can.
4. **Instrumentation, not the coil** (`yLow`, or a `yHigh` that survives valve
   isolation) — wrong AHU loop bound as `sat`, discharge sensor drifted or
   swapped with another box. A persistent negative rise is physically
   impossible from a leak, which is why the T chart's finding is named as
   instrumentation rather than folded into the coil story.

## Energy Impact

CRITICAL_WASTE, MEDIUM confidence, PROXY_ESTIMATION. Leak heat is bought at the
plant and then removed by cooling the same air back down — the same
double-payment as VAV-0003, seen from the temperature side instead of the
command side. The rule deliberately consumes no airflow point, so it prices
nothing in-rule; the proxy is `(dTerror − error_mean) × airflow` for a host
that wants numbers, and the leak-hours count alone ranks boxes for a valve
walk-down. Confidence is MEDIUM on the sibling cards' terms: the method is
validated on physically injected faults (§5.2), the shipped parameters are one
simulated box plus the source's campaign. Heating-season biased, since the
leak needs a live heat source behind the valve.

## Emissions Impact

Scope 1 or 2 by heat source, QUALITATIVE_EMISSIONS. A passing hydronic valve
spends boiler fuel (Scope 1 where gas) plus the chiller electricity that
removes it (Scope 2); an electric element is Scope 2 twice. The double-payment
structure means abating this fault removes emissions on both sides at once,
which is why it ranks above its comfort impact — the zone usually feels fine.

## Deviations

- **Reheat-on clears the accumulators rather than holding them.** The source
  computes dTerror only while the coil is off and says nothing about what the
  sums do meanwhile. Holding would carry a stale sum across a multi-hour
  heating call and alarm on the first armed tick after it; clearing costs the
  case where a slow leak is interleaved with regular legitimate reheat — but a
  coil regularly commanded open is VAV-0003's jurisdiction, not a coil
  passing unseen. The vectors pin the choice (`reheat_command_resets_accumulator`).
- **One `Logical.And` gates both charts through the shared reset switches,**
  collapsing the source's occupied-only rule and reheat-off rule into a single
  arm condition. The two evaluability flags stay separate outputs precisely
  because the host cannot otherwise tell which gate silenced the rule — a box
  heating all day publishes `yReheatOk = false` for hours and that is normal.
- **`error_mean`/`error_sigma` are measured, not floored — the contrast with
  VAV-0008.** Fan heat and duct gain give this channel a real healthy
  baseline; the committed vavcal method measured 0.44/0.39 °C across the
  OfficeMedium-4004 zones on reheat-off occupied ticks. They remain per-box
  commissioning values (§5.1.5): a long duct run or a series fan moves the
  baseline, and error_mean is where that lives.
- **`slack_k = 3.0` and `alarm_limit_h = 100` come from Table 5's Iowa
  campaign, not Figure 9's illustration** — the one channel in the trio whose
  defaults are measured. Still commissioning parameters; the campaign's boxes
  are not your boxes.
- **The T chart is kept and its finding renamed.** A leak cannot produce a
  negative rise, so `yLow` names instrumentation (wrong `sat` binding, sensor
  drift or swap) instead of pretending to be a coil finding. Dropping the
  chart would have been cheaper in blocks; keeping the source's two-sided
  recursion buys a free wrong-loop detector on the channel most exposed to a
  binding mistake.
- **`sat` stands in for entering-air temperature per §5.1.4's own workaround.**
  The bias of that approximation was validated below 0.005 °C by the committed
  harness method — in simulation, on one building; the real-world residual is
  duct gain, which `error_mean` absorbs by design.
- **`rht_vlv_cmd` is compared strictly below 1 % (`Reals.LessThreshold`)** —
  command, not position, because position feedback is rare on terminal units
  and a lying command is diagnosis 3. The threshold is a parameter so sites
  whose controllers never write a clean zero can still arm the rule.
- **The shared house choices are inherited, not re-argued:** in-graph reset
  driving the published sum and the delay input together, `delayOnInit = true`
  serving a fresh exclusion hour on restart, `UnitDelay` y_start seeding
  costless at zero, strict comparisons, no alarm delay, constants over
  parameterized gains. VAV-0008's Deviations carry the full arguments.
- **Severity 3, `category: CRITICAL_WASTE`, `confidence: MEDIUM` and the name
  are library-authored.** CRITICAL_WASTE follows VAV-0003 — same waste, same
  double payment — rather than the trio's COMFORT_ENERGY, because the zone is
  typically comfortable while this fault runs. `g36: null` on the same grounds
  as the siblings.

## Notes

The trio splits by error signal: VAV-0007 accumulates airflow tracking
error, VAV-0008 zone temperature error, this card the reheat-off discharge
rise. This is the only channel that needs `vav_dat`; a box without it runs the
other two as the source's reduced VPACC and loses only this coil's coverage.

VAV-0003 and this card meet at the same valve from opposite sides: 052
convicts a valve *commanded* open while the zone is satisfied, this card a
valve *passing* while commanded shut. Neither suppresses the other — a failed
actuator can produce both in one afternoon, and seeing both is the diagnosis.
