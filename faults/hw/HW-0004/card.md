---
schema: cxf-library/fault-card/v1
id: HW-0004
name: HW loop low delta-T
equipment: hw
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "PNNL-27338 §4.6 (low hot-water loop delta-T, pp. 4.19-4.21) — algorithm, 20 °F design / 10 °F trip pair, and its own note that the test carries no load gate"
  - "PNNL-27338 §4.4 (pp. 4.12-4.13) — the 35% pump-speed line this report uses to mean a lightly loaded HW loop, adopted here as the evaluability floor"
  - "PNNL-27338 (Katipamula et al. 2018) — adapted via an internal paraphrased deep-read digest, not distributed (paraphrased algorithm digest; candidate 8)"
  - "Sibling precedent: CHW-0004 (graph shape, two-parameter trip line, yLoadOk evaluability output), VFD-0002 (assembled limit)"
  - "Library extension: the HVAC FDD Reference v1.0 ch.14 specifies only HW-0001..052 — see faults/hw/README.md"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0002, HW-0003, HW-0005, HW-0010, CHW-0004, AHU-0015, VAV-0003]
playbooks: [low-delta-t, hot-water-plant-faults]
operating_states: "Heating plant producing — boilers firing or enabled and holding a supply setpoint — with the distribution pumps circulating above min_pump_speed_for_eval. The rule's yLoadOk covers the pump half of that state; the producing half is the host's to enforce, and the rule is wrong without it (see Deviations)."
preconditions: "hws_temp and hwr_temp must describe the same hydraulic loop at the same moment, and must be the loop the coils are on. On a primary/secondary plant the boiler's own leaving and entering temperatures see primary flow and read a healthy delta-T while the secondary loop that actually serves the coils short-circuits through the decoupler — which is the fault. Bind the secondary supply and return headers on a decoupled plant and the boiler connections on a variable-primary one, and bind hw_pump_vfd_speed from the pumps that move that same water. Both temperatures must be in °C (the rule converts nothing), and design_delta_t must be this loop's design value read off the plant's drawings, not the shipped 11.0 K, before any verdict means anything. Sensor placement and calibration decide the finding: a supply sensor reading low or a return sensor reading high biases delta-T toward the alarm, nothing in the rule can tell a swapped sensor pair from a genuine collapse, and a 0.5 K offset is 9% of a 5.5 K trip line. The plant must actually be making heat: a loop circulating with the boilers off equalises supply and return and alarms permanently (pinned by loop_circulating_with_no_heat_input), so gate host-side on the boiler or plant-enable status — HW-0003 is the rule that owns that condition. hw_pump_vfd_speed must be a speed feedback from a variable-flow distribution system: a constant-speed pump reads full speed always and the load gate protects nothing, and a drive pinned at a minimum speed above 35% does the same. Evaluability is signalled in-rule by yLoadOk; when it is false the verdict is NO_EVAL, not a healthy loop."
points:
  - hws_temp
  - hwr_temp
  - hw_pump_vfd_speed
outputs:
  - name: yFault
    description: True while the hot water delta-T has stayed below design_delta_t × low_dt_fraction with the distribution pumps above min_pump_speed_for_eval, continuously for at least alarm_delay
  - name: yLoadOk
    description: Evaluability signal — true when hw_pump_vfd_speed is above min_pump_speed_for_eval, the speed below which so little water is moving that delta-T says nothing about the distribution system. False means NO_EVAL and the host must ignore yFault
params:
  design_delta_t:
    default: 11.0
    unit: "°C"
    description: "Design hot water delta-T. PER-LOOP SITE CONFIGURATION — read it off the plant's design documents. The shipped 11.0 K is a placeholder near the 20 °F (11.1 K) design PNNL-27338 §4.6 assumes; radiant and low-temperature hydronic loops are commonly designed nearer 5-8 K, and condensing-boiler retrofits are often pushed to 15 K or more to hold the return temperature down, and none of those are served by the shipped value."
    cxf: designDt.k
  low_dt_fraction:
    default: 0.5
    unit: "1"
    description: "Fraction of design delta-T below which the loop is faulted. 0.5 reproduces PNNL-27338 §4.6's own pair, whose 10 °F trip is exactly half its 20 °F design. Kept as its own parameter rather than folded into the trip line so a site can retune the tolerance and the design value independently — see Deviations."
    cxf: lowDtLimit.k
  min_pump_speed_for_eval:
    default: 35.0
    unit: "%"
    description: "Distribution pump speed below which delta-T is not evaluated. ADOPTED — §4.6 gates on nothing at all; 35% is the speed §4.4 treats as a lightly loaded HW loop, and it sits below the 45% HW-0005 calls working hard."
    cxf: loadOk.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous low delta-T at flow required before the alarm asserts (60 min). ADOPTED from CHW-0004 — PNNL-27338 specifies a 15-60 min averaging window per §1.2, not an alarm persistence."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: HW distribution pump energy, boiler staging, and condensing-mode efficiency
  savings_range: "5-15% pump energy (CHW-0004's published range, carried across — PNNL-27338 publishes no savings figure for §4.6); on a condensing plant the boiler-side loss is larger than the pump-side one"
  climate_sensitivity: heating-dominant
  runtime_estimation: "excess_pump_kw ≈ hw_pump_kw × (design_dt − actual_dt) / design_dt — CHW-0004's estimator on the heating loop's numbers. hw_pump_kw is neither a point of this rule nor of the HW dictionary, so the host supplies it; actual_dt is the difference the graph already computes. Where the plant is condensing, add the efficiency term: return water above roughly 55 °C stops condensation and costs 5-10 points of boiler efficiency, which is a fuel number rather than a pump number."
emissions:
  scope: "1+2"
  method: PROXY_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, one January week, plant mode at 60 s"
    scenarios: 1
    failures: 0
    notes: "single RunPeriod with timeline/cadence validation; gated only where the boiler is active and hw_pump_vfd_speed remains a flow-fraction proxy. July had no evaluable boiler-active window and is not counted"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:022b2acd6415b5ce7aaba663fa8c3c49"
  date: 2026-10-05
---

## Description

A heating loop is sized on a temperature difference. Design the coils for 11 K
between supply and return and the pumps move enough water to carry the peak
load; let that difference fall to 4 K and the same load needs nearly three times
the flow, so the pumps speed up and a plant carrying the building on one boiler
starts a second. On a heating loop low delta-T means the return comes back **too
hot**, and a condensing boiler needs return water below roughly 55 °C to
condense at all — a loop whose return has crept up has not only doubled its
pumping, it has moved the boiler out of the regime the plant was bought for.
Nothing looks broken: zones hold setpoint and the boiler makes supply
temperature. It is measured at the loop because that is where a bypassed
three-way valve here and an oversized control valve there add up. This rule is a
library extension — the reference's ch.14 stops at HW-0003 — built from
PNNL-27338 §4.6 and CHW-0004's parameter shapes.

## Detection Logic

```
delta_t   = hws_temp − hwr_temp                     (supply minus return: the heating sign)
low_limit = design_delta_t × low_dt_fraction        (11.0 × 0.5 = 5.5 K)

yLoadOk = hw_pump_vfd_speed > min_pump_speed_for_eval   (false ⇒ host reports NO_EVAL)
yFault  = delta_t < low_limit AND yLoadOk,
          sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HW-0004 block graph](diagram.svg)

`deltaT` is CHW-0004's block with the inputs the other way round — supply on
`u1`, return on `u2` — and that ordering is the whole heating-side inversion. It
is the line to check first: a graph that subtracts in the cooling direction
reports a permanent fault on a healthy loop and looks exactly like a rule that
works. `designDt` and `lowDtLimit` assemble the trip line in the graph rather
than shipping a pre-multiplied 5.5, so both numbers stay independent `set_param`
targets; they are retuned for different reasons.

`lowDt` is strict, so a loop sitting exactly on the trip line reads healthy, and
the boundary is bit-exact: 11.0 halves to exactly 5.5, which a pair of realistic
temperatures can reach (65.5 − 60.0). `loadOk` is the evaluability story —
PNNL-27338 §4.6 gates on nothing, and without a gate every night setback and
mild afternoon on a reset loop reads as a fault. Pump speed is the only
load-shaped signal the HW dictionary carries; Deviations records what that
substitution costs. `persist` requires 60 continuous minutes and carries
`delayOnInit = true`; low delta-T is a loop condition, not an event.

## Possible Diagnoses

Library-authored — PNNL-27338 §4.6 specifies a threshold test, not causes, so
this is the heating-side reading of CHW-0004's list plus the hot-loop cases:

1. Three-way valves bypassing at coils, unit heaters and cabinet heaters — the
   classic cause, worst when the building is warm and the bypass port widest
2. Oversized two-way control valves — no authority left below 20% open, so the
   valve sits nearly shut and still overflows its coil
3. Zone valves failing open, leaking by, or left in hand
4. Loop bypasses and balancing valves left open from an unfinished
   commissioning, or reverse flow through a primary/secondary decoupler
5. Coils that cannot transfer their duty — water-side fouling, or air trapped at
   an unvented high point, which has no chilled-water analog
6. Distribution pressure above what the loop needs, forcing flow through valves
   already throttling. HW-0005 detects that directly and is the one to fix first
7. A supply temperature reset pushed too far down — cooler water makes every
   valve open further for the same duty; the fix is the reset schedule
8. A loop oversized for the building it ended up serving — the case with no
   repair, where the delta-T is telling the truth

Causes 1 through 5 are local defects this loop-level rule aggregates: forty
heating coils can reach the trip line with four misbehaving and thirty-six fine.

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, PROXY_ESTIMATION. The pump term is
CHW-0004's estimator on heating values —
`excess_pump_kw ≈ hw_pump_kw × (design_dt − actual_dt) / design_dt` — so a loop
at 5 K on an 11 K design spends more than half its pump energy moving water that
comes back too hot to be worth having moved. PNNL-27338 publishes no savings
range for §4.6, so the 5-15% in `savings_range` is the chilled-water figure and
is the weaker half of the claim; the stronger half is the boiler, where a return
above the condensing threshold forfeits 5-10 efficiency points, or on
non-condensing plant costs the staging and cycling HW-0001 measures.
Confidence is MEDIUM because the finding is loop-level and the repair is not.

## Emissions Impact

Scope 1 + 2, PROXY_EMISSIONS, MEDIUM confidence. The split matters more than the
total: pump energy is purchased electricity on a marginal operating emissions
rate, while the condensing-mode and staging penalties are fuel burned on site on
a static factor. A site that has decarbonised its electricity still owns the
whole Scope 1 half, and on a condensing plant that half is the larger one. No
published emissions range exists for this fault — the sibling CHW rule's figure
is a cooling-side pump-and-staging number and does not transfer — so the
estimate is left to the host's own fuel and electricity factors.

## Deviations

- **This rule is a library extension, not a transcription.** The reference's
  ch.14 specifies HW-0001, 051 and 052 and stops; `faults/hw/README.md` frames
  FC-053 through 057 as library-authored rules grounded in PNNL-27338 §4. The
  name, severity 3 and `method: rule` are that index's; the graph, parameters,
  diagnoses and energy claim are authored here, with every number the report
  does not fix marked ADOPTED in `params`.
- **The delta-T is inverted relative to CHW-0004, and the inversion is the
  whole port.** Same block and trip line, opposite operand order, because on a
  heating loop the supply is the hot side. Clinically it means the *return* is
  too hot, which is why this card carries a condensing-boiler argument its
  sibling has no reason to make; for deployment it means getting it wrong fails
  silently, with every healthy loop reporting a permanent fault.
- **`design_delta_t = 11.0 K` is ADOPTED, and the 0.1 K between it and
  PNNL-27338 is deliberate.** §4.6 writes a 20 °F (11.1 K) design with the trip
  at half. The parameter is a per-loop site value that must be read off the
  drawings anyway, so a tenth of a kelvin of transcription fidelity buys
  nothing — and 11.0 halves to exactly 5.5, whose trailing mantissa zeros let a
  realistic temperature pair land on the line and make the strict comparison
  testable rather than merely asserted. At 11.1 the line falls at 5.55, which no
  difference of two doubles in the 32-128 binade reaches. The shipped line sits
  0.06 K below PNNL's, an order of magnitude inside any sensor's error.
- **`low_dt_fraction = 0.5` is PNNL-27338's ratio, factored into its own
  parameter.** §4.6 subtracts against a fixed 10 °F; this rule reconstructs the
  same line as `design × fraction` so a site with a 20 K condensing-retrofit
  design can retune one without recomputing the other. Three blocks instead of
  one buys that. Precedent: CHW-0004 and VFD-0002's assembled speed floor.
- **The load gate is entirely adopted — §4.6 has none,** and its own prose names
  low demand as a confound without encoding a fix. Shipping that literally would
  alarm through every night setback and mild shoulder-season afternoon on loops
  whose delta-T is small for the correct reason. The gate is the CHW sibling's
  design imported wholesale and is the largest departure from the cited
  algorithm.
- **`hw_pump_vfd_speed` substitutes for CHW-0004's `chiller_load`, and the
  substitution is not free.** The HW dictionary has no load analog —
  `boiler_firing_rate` describes what the boiler is doing, not how much water is
  moving — and PNNL-27338 itself uses pump speed as its HW load heuristic in
  §4.2 and §4.4. The costs: the proxy is partly endogenous (low delta-T raises
  flow, which raises pump speed), so the gate excludes *quiet* loops rather than
  lightly loaded ones; a constant-speed pump or a drive with a minimum above 35%
  never falls below the floor, so the gate protects nothing; and a drive that
  latches its last command reads true on a dead loop. All three are
  `preconditions` text, because none is separable inside the rule.
- **`min_pump_speed_for_eval = 35%` is ADOPTED from §4.4, not ported from the
  CHW sibling,** whose 40% is a chiller load and does not convert. 35% is the
  number PNNL-27338 uses for a hot water loop with little demand, and it sits
  below the 45% §4.2 calls working hard, so this rule and HW-0005 read the
  speed axis consistently: a loop between 35% and 45% is evaluable here and
  uninteresting there.
- **Strict `<` at the trip line and strict `>` at the load floor.** CDL `Reals`
  has no `LessEqual` or `GreaterEqual` in any case, and PNNL's own arithmetic is
  strict. A loop at exactly 5.5 K reads healthy and a pump at exactly 35.0%
  reads NO_EVAL; both disagreements are measure-zero and both err toward silence.
- **`yLoadOk` is an evaluability output, not an echo of an input.** It is a
  boundary input compared against a parameter, which is what SCHEMA.md asks such
  an output to be; it adds no logic and changes no verdict, and it is the only
  thing that lets a host tell "delta-T is fine" from "the loop was too quiet to
  ask". Same stance as CHW-0004's `yLoadOk` and HP-0001's `yPowerOk`.
- **Persistence stands in for PNNL's window average.** Every AIRCx algorithm
  averages a 15-60 minute window and compares the average (§1.2); this rule
  consumes instantaneous points and requires the condition continuously, so a
  delta-T alternating above and below the line every 20 minutes never alarms
  even though a loop spending half its day low is a genuine finding. A steady
  syndrome — a bypassing or oversized valve — reads the same either way.
- **There is no boiler-on conjunct, and that is a real blind spot.** A loop
  circulating with no heat input equalises, so delta-T goes to zero and the rule
  alarms at full confidence on a plant with no distribution defect. Adding
  `boiler_status` was rejected: it rebuilds HW-0003's plant-level disjunction
  inside a distribution rule, silences the rule during the off-cycles of a plant
  that is firing normally, and trades a documented false positive for an
  undocumented false negative. The honest placement is `operating_states` plus a
  host gate.
- **Nothing guards against an inverted delta-T either.** A swapped sensor pair,
  or a pair mounted on the wrong side of a decoupler, gives a negative delta-T
  that is below any positive trip line and alarms permanently. A limiter or a
  second comparison against zero could suppress it, but a genuinely negative
  delta-T is also what reverse flow through a decoupler looks like, so the guard
  would hide a real hydraulic fault to hide a wiring one. Commissioning check:
  swap the leads and watch the sign, once, before trusting the rule.
- **The rule is blind to which coil is responsible, and to how many.** That is
  the design rather than a simplification — the individual defects are usually
  too small to detect one at a time, which is why the syndrome is measured in
  the return header. Air-side companions AHU-0015, FCU-0005 and VAV-0003
  are `related`, none is wired.
- **`alarm_delay = 3600 s` is adopted from CHW-0004.** PNNL-27338 specifies a
  data window and a minimum sample count, not an alarm persistence. An hour
  matches the sibling and HW-0003, and a heating loop's thermal mass makes
  anything shorter noise.
- `persist.delayOnInit = true` (CDL default `false`), the library's standing
  choice: a loop already below the line at controller restart waits out the full
  hour rather than alarming on the first tick.
- **`playbooks` cites two, and neither Applies-To row names this card yet.**
  `low-delta-t` is written around the chilled-water plant but already claims
  PNNL-27338 covers both CHW and HW, and its Step 1 arithmetic transfers once
  the subtraction is read in the heating direction; `hot-water-plant-faults`
  names low loop delta-T in its energy row. Both Applies-To rows are the index
  owner's edit, the same sequencing CHW-0004 recorded.
- **`clusters: []`.** `clusters/clusters.json` has no hot water cluster; CLU-06
  is chilled water by name and membership. A hot-water plant syndrome (this
  rule, HW-0005, 055 and 056 all describe one plant giving away pump and fuel
  energy) is a reasonable future cluster and the cluster owner's edit.
- **`suppresses` and `suppressed_by` are both empty.** HW-0005 is the closest
  candidate — distribution pressure above what the loop needs is diagnosis 6 —
  but it is a cause of low delta-T rather than a reason to disbelieve it, and
  both findings stay true and separately actionable. Suppression edges must also
  be declared on both cards, and HW-0005 ships the matching empty pair.
- **No published test vectors exist for this algorithm.** PNNL-27338 §4.6
  specifies thresholds, not cases, so every scenario in `vectors.json` is
  authored from the equation and replayed against the pinned engine rev.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph, per the library's design
  stance.

## Notes

Read `yLoadOk` before `yFault`. A loop that is off, or coasting through a mild
afternoon at 25% pump speed, holds `yLoadOk` false for hours, and every
`yFault = false` underneath means "not evaluated" rather than "delta-T is fine".

Check whether the plant is condensing before deciding what the finding is worth:
there the return temperature is an input to the efficiency curve and the fuel
penalty dominates the pump penalty, while on a non-condensing plant the same
alarm is a pumping and staging finding that can be scheduled rather than chased.

Trend delta-T against outdoor air for a week before sending anyone. A delta-T
that degrades as the weather warms points at bypasses and valves with no
authority left at low load; one that is flat and low across the range points at
oversized valves, an over-aggressive reset, or an oversized loop. Vent the high
points, then check the largest coils — the syndrome is a sum and the big air
handlers dominate it. Where HW-0005 also fires, treat the pressure finding as
the trigger and re-check delta-T after the DP setpoint comes down.
