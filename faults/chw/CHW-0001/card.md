---
schema: cxf-library/fault-card/v1
id: CHW-0001
name: Chiller efficiency (kW/ton) degradation
equipment: chw
status: verified
phase: 2
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: HIGH
estimation_method: BASELINE_COMPARISON
source:
  - "HVAC FDD Reference v1.0 §13 (ch. 'Chilled Water Plants', pdf pp. 118-119), CHW-0001"
  - "PNNL retuning; PNNL EEM-26"
  - "Chen et al. 2024"
g36: null
clusters: [CLU-06]
suppresses: []
suppressed_by: []
related: [CHW-0002, CHW-0004, CHW-0005, CHW-0007, CHW-0009, HP-0001]
playbooks: [chiller-efficiency]
operating_states: "chiller running and loaded above min_chiller_load, past the start transient — one rule instance per chiller, each carrying that machine's fitted coefficients"
preconditions: "The host owns the baseline. It runs the learning_period_days (30 d) Ridge regression of kW/ton against load, CWST and CHWST for THIS machine, confirms the fit is good enough to hold the plant to, and writes the four coefficients in with set_param; until it has, the rule is comparing against the shipped placeholders and means nothing (see Deviations). chiller_tons MUST be bound in refrigeration tons — the point dictionary flags this as its main hazard, because kW/ton and the fitted coefficients go wrong together and silently if the host feeds kW thermal instead. On most plants chiller_tons is a host-computed virtual point (flow × delta-T × cp), so its provenance is part of the baseline precondition rather than separate from it: a tons figure derived from a drifting flow meter moves the quotient and the fit together. The chiller must also have settled after a start or a capacity step before the quotient means anything; a machine still pulling down reads degraded on physics. The regressors must lie inside the range the fit was taken over — the graph extrapolates the plane forever and knows nothing about where the fit stops being physical. Evaluability is signalled in-rule by two outputs: yLoadOk (the reference's min_chiller_load gate) and yTonsOk (the divide guard). When either is false the verdict is NO_EVAL, not healthy."
points:
  - chiller_kw
  - chiller_tons
  - chiller_load
  - cwst
  - chwst
outputs:
  - name: yFault
    description: True while the measured kW/ton has stayed above degradation_ratio_threshold × the fitted baseline for the current load and water temperatures, continuously for at least alarm_delay
  - name: yLoadOk
    description: Evaluability signal — true when chiller_load is above min_chiller_load, the load below which kW/ton is not comparable to the baseline. False means NO_EVAL and the host must ignore yFault
  - name: yTonsOk
    description: Evaluability signal — true when chiller_tons is above min_chiller_tons, the floor below which the kW/ton quotient is meaningless or undefined. False means NO_EVAL and the host must ignore yFault
params:
  kw_per_ton_load_coeff:
    default: 0.0025
    unit: "kW/ton per %"
    description: "Load coefficient of the host-fitted kW/ton baseline. PER-MACHINE SITE CONFIGURATION — the reference supplies a Ridge regression, not a number, and the shipped value is a placeholder describing a generic water-cooled centrifugal machine. Meaningful only together with the other three coefficients."
    cxf: loadTerm.k
  kw_per_ton_cwst_coeff:
    default: 0.01
    unit: "kW/ton per °C"
    description: "Condenser water supply temperature coefficient of the same fit — the condenser-lift term, positive because warmer condenser water costs more kW per ton. PER-MACHINE SITE CONFIGURATION on the same terms."
    cxf: cwstTerm.k
  kw_per_ton_chwst_coeff:
    default: -0.015
    unit: "kW/ton per °C"
    description: "Chilled water supply temperature coefficient of the same fit. Inherently negative for a normal machine — raising the evaporator temperature reduces lift and improves kW/ton — which is why this parameter is signed (see Deviations). PER-MACHINE SITE CONFIGURATION."
    cxf: chwstTerm.k
  kw_per_ton_intercept:
    default: 0.25
    unit: kW/ton
    description: "Intercept of the same fit: the expected kW/ton at zero load, 0 °C condenser water and 0 °C chilled water, which is a fitting artefact rather than an operating point. PER-MACHINE SITE CONFIGURATION; the four coefficients are only meaningful as a set."
    cxf: expected.p
  degradation_ratio_threshold:
    default: 1.1
    unit: "1"
    description: "Multiple of the baseline kW/ton the machine must stay below. 1.1 is the reference's 10% degradation_threshold written as a multiplier: fault when measured > 1.1 × expected. It is a multiplier, not a percentage — writing 10 here silences the rule (see Deviations)."
    cxf: allowed.k
  min_chiller_load:
    default: 30.0
    unit: "%"
    description: "Chiller load below which kW/ton is not evaluated. The reference's min_chiller_load: a lightly loaded chiller is inefficient on physics, and the baseline was not fitted down there."
    cxf: loadOk.t
  min_chiller_tons:
    default: 10.0
    unit: tons
    description: "Cooling output below which the kW/ton quotient is not evaluated. Guards the division — at zero tons the quotient is infinite or NaN. PER-MACHINE SITE CONFIGURATION: set it from the machine's real minimum output, not from zero (see Deviations). The shipped 10 tons suits a mid-size machine and is arbitrary on a 3,000-ton one."
    cxf: tonsOk.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous degradation required before the alarm asserts (60 min). The reference's AlarmDelay, renamed to the library's convention"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Chiller compressor energy
  savings_range: "5-15% chiller energy; 0.1 kW/ton above baseline ≈ 15% excess"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "waste_kw = chiller_kw × (1 − expected_kw_per_ton / measured_kw_per_ton) — the reference's formula. The graph computes both quantities but publishes neither as a boundary output, so the host recomputes the ratio from the same points and parameters"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:86abcc0abe758ffcc575e7dff93afc03"
  date: 2026-10-05
---

## Description

Kilowatts per ton is the chiller trade's efficiency number, and on its own it
diagnoses nothing: the machine that turns in 0.45 kW/ton on a mild morning with
cold condenser water turns in 0.75 on a design afternoon, and both can be
perfectly healthy. A fixed threshold alarms all summer or never alarms at all.
What is stable is the relationship — for a given machine kW/ton is close to a
plane in three variables (load, condenser water temperature, chilled water
temperature). The host fits that plane (the reference specifies a Ridge
regression retrained periodically over 30 days) and writes the four coefficients
in as parameters; the graph asks whether today's reading is more than 10% above
what the plane predicts. The fault it finds best is the one nobody notices —
condenser tube fouling, which develops over months and never trips the chiller's
own panel. The reference's yardstick: 0.1 kW/ton above baseline is roughly 15%
excess chiller energy.

## Detection Logic

```
measured_kw_per_ton = chiller_kw / chiller_tons
expected_kw_per_ton = kw_per_ton_load_coeff  × chiller_load
                    + kw_per_ton_cwst_coeff  × cwst
                    + kw_per_ton_chwst_coeff × chwst
                    + kw_per_ton_intercept
allowed_kw_per_ton  = degradation_ratio_threshold × expected_kw_per_ton

yLoadOk = chiller_load > min_chiller_load       (false ⇒ host reports NO_EVAL)
yTonsOk = chiller_tons > min_chiller_tons       (false ⇒ host reports NO_EVAL)
yFault  = measured_kw_per_ton > allowed_kw_per_ton AND yLoadOk AND yTonsOk,
          sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![CHW-0001 block graph](diagram.svg)

`loadTerm`, `cwstTerm`, `chwstTerm`, the two `Reals.Add` blocks and `expected`
are the fitted plane — the whole statistical content of the rule at runtime.
`allowed` scales it by the tolerance, so the comparison is against a second plane
parallel to the first rather than against a number: at the shipped placeholders a
machine at 50% load with 30 °C condenser water making 6 °C chilled water is
expected at 0.585 kW/ton and allowed 0.6435, and identical power and tonnage
give opposite verdicts once the regressors move.

`kwPerTon` is the only division and its denominator goes to zero whenever the
machine unloads or the tons calculation fails; with tons at zero the quotient is
infinite and the comparison is true, so without `tonsOk` a dead flow meter would
produce a permanent alarm. `tonsOk` and `loadOk` each drive a boundary output and
a gate, so a machine below either floor holds `yFault` down and the host reads
the silence as "not evaluated". The load floor is about comparability rather than
arithmetic: at 20% load a chiller is inefficient because it is a chiller at 20%
load.

The comparison at the allowance is strict, so a machine exactly on the allowed
plane reads healthy. `persist` requires 60 continuous minutes — enough to ride
out a capacity step, a condenser water reset or a stage change — and carries
`delayOnInit = true`.

## Possible Diagnoses

Transcribed from the reference's CHW-0001 card:

1. Condenser tube fouling — scale, biofilm or silt from an open tower loop. The
   most common cause and the one this rule is really for; the condenser approach
   widens long before anything else complains
2. Evaporator tube fouling — rarer, the loop being closed, and usually a water
   treatment or an opened-for-work story
3. Low refrigerant charge — degrades kW/ton across the whole operating range
   without producing a single reading that looks wrong on its own
4. Compressor degradation: worn bearings, damaged impellers, failing unloaders —
   elevated draw for the same delivered capacity
5. Non-condensable gases raising condensing pressure; on a low-pressure machine
   the purge unit's runtime usually tells the story first

The discriminator between 1 and 2 is the approach temperature on each side,
which this rule does not read — a plant that trends both approaches alongside
this alarm separates the two causes for free. The chapter's introduction
promises approach-temperature analysis, but none of its specified rules performs
it and neither does this library.

## Energy Impact

EFFICIENCY_LOSS, HIGH confidence, BASELINE_COMPARISON. The estimator is the
ratio the rule already computes:
`waste_kw = chiller_kw × (1 − expected_kw_per_ton / measured_kw_per_ton)` — a
machine drawing 358 kW at 0.70 kW/ton against a 0.585 baseline spends about
59 kW on the degradation. The reference's range is 5–15% of chiller energy, and
its 0.1 kW/ton ≈ 15% yardstick is the one to quote to an operator because it is
in the units the plant's trend screen already shows. HIGH confidence carries the
caveat every self-learned baseline has: the fit is this machine's own recent
history, so a chiller with fouled tubes at commissioning learns a fouled
baseline and reads healthy forever. Cooling-dominant, and largest on the hottest
days.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, HIGH confidence; the reference's typical range is
1,000–10,000 kg CO₂e/yr on a marginal operating emissions rate basis. All of it
is purchased electricity at the compressor. The range is wide because it spans a
small machine with a slight charge loss and a large one with fouled tubes, and
because the marginal rate in the hours a chiller runs hardest is well above the
annual average — a cooling peak is when the dirtiest generator on the system is
dispatched.

## Deviations

- **All four baseline coefficients ship as documented PLACEHOLDERS.** The
  reference specifies a model, not numbers
  (`baseline_model.predict([chiller_load, cwst, chwst])`, sklearn Ridge,
  `learning_period_days = 30`), and this library's split puts the fitting in the
  host and the fitted plane in the graph as `set_param` targets. The shipped set
  describes a generic water-cooled centrifugal machine and exists so the
  document is runnable as delivered. **They are not site values, and a wrong set
  fails silently in both directions** — fit the plane 15% high and nothing ever
  alarms, fit it low and every hour does. Precedent: HP-0001's COP line,
  VAV-0001's `ventilation_requirement`.
- **The reference publishes no fit-quality bar for this fault, and this card
  does not invent one.** HP-0001's chapter specifies `R² > 0.6`; chapter 13
  specifies only "retrained periodically" over 30 days, so the precondition asks
  the host to confirm the fit is good enough to hold the plant to. A site
  adopting 0.6 by analogy is making a defensible choice, not following the
  reference.
- **`kw_per_ton_chwst_coeff` is negative — the documented exception to the
  library's no-negative-parameters convention.** A regression coefficient is
  inherently signed (raising the evaporator temperature reduces lift and
  improves kW/ton), and a host re-fitting on odd data could get any sign on any
  of the three; forcing signs into the topology would mean a different graph per
  machine. Same reasoning as HP-0001's `cop_baseline_slope`.
- **The degradation test is a multiplier, not the reference's fraction.** The
  reference writes `(measured − expected)/expected > degradation_threshold` at
  10%; this rule computes `measured > 1.1 × expected`, the same predicate for
  any positive `expected`, and it avoids a second division by a fitted plane
  that can cross zero. The units consequence bites:
  `degradation_ratio_threshold` is a multiplier (1.1), not a percentage (10),
  and writing 10 silences the rule because no machine draws ten times its
  baseline. HP-0001 carries the mirror image, where the same mistake alarms
  permanently.
- **`min_chiller_tons` and `yTonsOk` are adopted, not transcribed.** The
  reference's only evaluability gate is `min_chiller_load`, but the graph
  divides by a live signal, and per SCHEMA.md a test computable from the rule's
  own inputs belongs in the graph as a boundary output. Same stance as
  HP-0001's `yPowerOk`.
- **The tonnage floor guards zero, not a wrong reading.** A tons signal that has
  collapsed to a small non-zero value — a flow meter reading 5% of actual, a
  delta-T using a failed sensor — produces a believable quotient and a false
  alarm on a healthy machine. Set `min_chiller_tons` from the machine's real
  minimum output rather than from zero, and read `yFault` beside the plant's own
  tonnage trend.
- **The tons unit hazard is the biggest single way to deploy this rule wrong.**
  The kW/ton convention uses refrigeration tons while most BAS trend kW thermal
  or derive tons from flow × delta-T; feeding kW thermal scales the quotient by
  3.517, and because the host fits the coefficients against the same wrong
  signal the baseline scales with it — the rule keeps working while every number
  in it is meaningless to a human reading the alarm. Nothing in the graph can
  detect this; it is a binding-time check.
- **Strict `>` at the allowance and at both floors.** CDL `Reals` has no
  `GreaterEqual`, and the reference's degradation test is strict too. The load
  floor is a different case — `min_chiller_load` appears only in the reference's
  tunables table, never in its equation — so the operator is this card's choice
  and strict is the conservative one: exactly at a floor is NO_EVAL, and a
  machine exactly on the allowed plane reads healthy.
- **`learning_period_days` (30 d) stays a host precondition, and re-fitting is
  the dangerous part.** The fit happens offline, outside anything the graph can
  see. Whatever schedules it must refuse to re-fit while this fault is active:
  re-fitting a degraded machine bakes the degradation in as the new normal, and
  it matters more here than on HP-0001 because the reference asks for periodic
  retraining rather than a one-time fit.
- **The fitted plane is extrapolated without limit.** Nothing in the graph knows
  the range the regression was fitted over. Far enough outside it — and with the
  negative CHWST coefficient — `expected` can be driven non-positive, at which
  point every measured quotient exceeds `allowed` and the rule alarms
  permanently rather than going quiet (the opposite failure from HP-0001's,
  because the comparison runs the other way). The block set has no domain guard,
  so it is a precondition; a host can clamp the regressors with `Reals.Limiter`
  upstream.
- **One instance per chiller.** The reference's points are per-machine and so is
  the fit; a plant with three chillers runs three instances with three
  coefficient sets. The library has no plant-level aggregate rule, and averaging
  machines would hide exactly the one that is degraded.
- **`method: statistical` describes the baseline's provenance, not the
  runtime.** The graph performs one division, four multiplies, three adds, three
  comparisons and a delay; the classification is honest because the coefficients
  come from a regression. HP-0001 and RTU-0002 carry the same note.
- **`AlarmDelay` is renamed `alarm_delay`**, matching every other card and
  unchanged at 60 min. `persist.delayOnInit = true` (CDL default `false`), the
  library's standing choice: a machine already above its plane at controller
  restart waits out the full hour.
- **The reference publishes no test vectors for this fault**, so every scenario
  in `vectors.json` is authored from the equation and replayed against the
  pinned engine rev.
- **The chapter's Notes line for this fault is truncated in the source
  extract.** It reads "Chen et al. (2024) showed data-driven chiller FDD
  degrades under" and stops. The missing clause is presumably about
  generalisation to unseen conditions, but this card does not transcribe what it
  cannot read: `source` cites the paper and the extrapolation deviation states
  the blind spot in terms this rule supports.
- **`clusters: [CLU-06]` is the existing cluster set's membership, not this
  card's authorship** — `clusters/clusters.json` already names this fault as the
  trigger of "Chilled Water Plant Inefficiency" with CHW-0002, 052 and 053 as
  members, and its `playbook` slug resolves to the reference's transcribed
  `playbooks/chiller-efficiency.md`.
- **`playbooks` cites `chiller-efficiency`,** the reference's own playbook for
  this fault; its Applies-To row names CHW-0001 and CLU-06 directly.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph, per the library's design
  stance. Severity 3, `method: statistical`, `confidence: HIGH` and the fault
  name are the reference's chapter 13 card.

## Notes

Read `yLoadOk` and `yTonsOk` before `yFault`. A cleaned condenser, a chiller
that unloaded and a dead tonnage signal all drop `yFault` on the same tick and
mean completely different things, so a host that treats the falling edge as a
repair will close this fault every time the plant stages down.

Pull the condenser approach temperature first: it is free, it is already on the
chiller's panel, and a widening approach with a rising kW/ton is tube fouling
with enough confidence to schedule a cleaning. A normal approach on both sides
puts the loss inside the machine — charge, compressor or non-condensables — and
purge runtime is the cheapest test for the last of those.

Check CHW-0004 when both fire: low delta-T forces early staging, so a plant
running two machines where one would do has a kW/ton problem whose cause is in
neither machine. And raising the chilled water setpoint moves this rule's
expected line *down*, so a site switching CHW-0002's reset on mid-baseline
should re-fit rather than let the old plane judge the new regime.
