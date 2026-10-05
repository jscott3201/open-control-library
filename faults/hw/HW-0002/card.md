---
schema: cxf-library/fault-card/v1
id: HW-0002
name: Boiler efficiency degradation
equipment: hw
status: verified
phase: 2
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: HIGH
estimation_method: BASELINE_COMPARISON
source:
  - "HVAC FDD Reference v1.0 §14 (ch. 'Hot Water Plants', pdf pp. 125-126), HW-0002"
  - "Meng et al. 2021"
  - "Shohet et al. 2020"
  - "PNNL-13890 (O&M best practices)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0001, HW-0003, HW-0010, HW-0012, HP-0001]
playbooks: [hot-water-plant-faults]
operating_states: "boiler firing and settled at its current fire — one rule instance per boiler, each carrying that boiler's fitted line"
preconditions: "fuel_power and thermal_power must be computed on the same heating-value convention the baseline was fitted on. The point dictionary's warning on fuel_power is the one that bites hardest here: fuel input derived from flow × HHV and fuel input derived from flow × LHV differ by roughly 10% for natural gas, so an efficiency computed one way against a line fitted the other way is wrong by about five times this rule's whole threshold. Pick one convention per site, record it, and refit if it ever changes. The host owns the baseline: it runs the learning_period_days (14 d) regression of efficiency against boiler_firing_rate for THIS boiler and writes the result into eff_baseline_slope and eff_baseline_intercept with set_param. Until it has, the rule is comparing against shipped placeholders and means nothing (see Deviations). boiler_firing_rate must also lie inside the range the line was fitted over — the graph extrapolates forever and knows nothing about where the fit stops being physical. thermal_power is almost always a host-computed virtual point (HW flow × ΔT × cp); its provenance is part of the baseline's validity, not separate from it, and a flow meter that reads 5% high moves measured efficiency by five points on its own. The boiler must be firing and settled: the minutes after a light-off are spent heating the vessel rather than the water, and they read as degraded on physics. Evaluability is signalled in-rule by yFuelOk; when it is false the verdict is NO_EVAL, not healthy."
points:
  - thermal_power
  - fuel_power
  - boiler_firing_rate
outputs:
  - name: yFault
    description: True while the measured efficiency has stayed more than efficiency_threshold below the fitted baseline for the current firing rate, continuously for at least alarm_delay
  - name: yFuelOk
    description: Evaluability signal — true when fuel_power is above fuel_power_min, the floor below which the efficiency quotient is meaningless. False means NO_EVAL and the host must ignore yFault
params:
  eff_baseline_slope:
    default: 0.0006
    unit: "1/%"
    description: "Slope of the host-fitted efficiency-vs-firing-rate regression, in efficiency fraction per percent of fire. PER-BOILER SITE CONFIGURATION — the reference supplies a learned model, not a number, and the shipped 0.0006 is a placeholder for a conventional non-condensing boiler. Inherently signed: a condensing boiler's fit is negative, because its efficiency is highest at low fire"
    cxf: fireSlope.k
  eff_baseline_intercept:
    default: 0.78
    unit: "1"
    description: Intercept of the same regression — the expected efficiency extrapolated to 0% fire, which is not an operating point but is what a straight line needs. PER-BOILER SITE CONFIGURATION on the same terms as the slope; the pair is only meaningful together
    cxf: expected.p
  efficiency_threshold:
    default: 0.05
    unit: "1"
    description: "Shortfall below the fitted line that counts as degradation, in efficiency FRACTION — 0.05 is the reference's 5 efficiency points. This is an absolute difference, not a relative one: writing 5 into it (percent) silences the rule permanently"
    cxf: effLow.t
  fuel_power_min:
    default: 5.0
    unit: kW
    description: "Fuel input below which the efficiency quotient is not evaluated. Guards the division — at zero fuel the quotient is NaN, and at a standing pilot it reads as total degradation. PER-BOILER SITE CONFIGURATION: set it above the pilot and purge flow and below the smallest genuine firing input"
    cxf: fuelOk.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: Continuous shortfall required before the alarm asserts (60 min). ADOPTED, not transcribed — the reference's tunables line for this card truncates before reaching it (see Deviations)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Boiler fuel consumption
  savings_range: 5-15% fuel; PNNL-13890 case study puts $730/yr on a 300-hp boiler
  climate_sensitivity: heating-dominant
  runtime_estimation: "waste_kw = fuel_power × (1 − measured_eff / expected_eff) — the reference's formula verbatim. Both terms are already in the graph; the host reconstructs the quotient from the same three points"
emissions:
  scope: "1"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:2b1b4b0297999b62251fc5113d6cacf8"
  date: 2026-10-05
---

## Description

A boiler's efficiency is not a constant and is not supposed to be. The same
burner returns more of its fuel as useful heat at full fire than at minimum
fire, where jacket and standby losses are spread across a smaller output — or
less, if the boiler condenses, because a cooler flue and a wetter heat exchanger
are what low fire produces. Which way the curve runs is a fact about the
machine, so no fixed efficiency threshold would either stay quiet all winter or
ever alarm. What there is, per boiler, is a line: the host fits efficiency
against firing rate over fourteen days and writes the slope and intercept in as
parameters, and the graph asks whether today's fire is within five efficiency
points of what that line predicts. Five points is a lot of gas, and none of the
causes announce themselves — scale, soot and a rich burner all leave the boiler
making its setpoint on more fuel.

## Detection Logic

```
measured_eff = thermal_power / fuel_power
expected_eff = eff_baseline_slope × boiler_firing_rate + eff_baseline_intercept

yFuelOk = fuel_power > fuel_power_min            (false ⇒ host reports NO_EVAL)
yFault  = (expected_eff − measured_eff) > efficiency_threshold AND yFuelOk,
          sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HW-0002 block graph](diagram.svg)

`fireSlope` and `expected` are the fitted line and the whole statistical content
of the rule at runtime; `shortfall` and `effLow` write the reference's equation
out unchanged, as an absolute difference in efficiency points rather than a
ratio. At the shipped placeholders the line predicts 0.792 at 20% fire, 0.810 at
50% and 0.840 at full fire, with the alarm five points under each — identical
meter readings are healthy at low fire and faulted at high fire, decided
entirely by `boiler_firing_rate`.

`eff` is the only division in the rule and its denominator goes to zero every
time the burner stops. With both meters at zero the quotient is NaN and every
comparison against it is false; with a standing pilot and no useful output it is
a clean, believable 0.0, which is the more dangerous of the two. `fuelOk` drives
both the boundary output `yFuelOk` and the second input of `gate`, so a boiler
below the floor holds `yFault` down and the host reads the silence as "not
evaluated" rather than "healthy". `persist` requires 60 continuous minutes of
shortfall — long enough to ride out a light-off, a firing-rate step or a
return-temperature swing — and carries `delayOnInit = true`.

## Possible Diagnoses

Transcribed from the reference's HW-0002 card:

1. Fouled heat exchanger or water-side scale — a millimetre costs several
   efficiency points and develops slowly enough for a fitted line to catch it
2. Burner misalignment or fouling — soot does the same from the fire side, and a
   sooted burner is usually a badly adjusted one
3. Incorrect fuel/air ratio — excess air carries heat up the stack, too little
   leaves fuel unburned; only a combustion analyser separates them
4. Flue gas recirculation problem — an FGR damper out of position moves the
   NOx/efficiency trade the burner was commissioned on
5. Refractory degradation — cracked refractory lets heat into the jacket instead
   of the water, usually found only when someone opens the front

## Energy Impact

EFFICIENCY_LOSS, HIGH confidence, BASELINE_COMPARISON. The estimator is the
quotient the rule already computes:
`waste_kw = fuel_power × (1 − measured_eff / expected_eff)` — a boiler burning
1000 kW at 0.74 against an 0.81 baseline wastes about 86 kW of gas. The
reference's range is 5–15% of fuel; PNNL-13890's case study puts $730/yr on a
300-hp boiler. HIGH confidence holds because both terms are metered rather than
inferred, with two caveats that do not change it: the baseline is the boiler's
own recent behaviour, so degradation present when the line was fitted is
invisible, and `thermal_power` is usually derived, so its accuracy is the flow
meter's. Heating-dominant by construction.

## Emissions Impact

Scope 1, PROXY_EMISSIONS, HIGH confidence; the reference's typical range is
1,000–10,000 kg CO₂e/yr against a static 0.181 kg CO₂e/kWh natural gas factor.
This is combustion at the building, so there is no grid to hedge against: the
avoided-emissions basis is the static Scope 1 factor and the saving is the same
whatever hour the boiler runs, which makes the emissions arithmetic the energy
arithmetic times a constant.

## Deviations

- **The comparison is an absolute difference in efficiency points, as the
  reference writes it**, and it diverges from the sibling cards on purpose:
  CHW-0001 tests a *relative* loss and HP-0001 multiplies its ratio through,
  both to avoid dividing by a fitted line, and neither move is needed here. The
  consequence at deployment: an absolute threshold is a larger relative
  tolerance the lower the baseline sits — five points is 6.1% of an 0.82
  baseline and 5.6% of an 0.90 one — so this rule is the more forgiving of the
  two, and most forgiving where there is least efficiency to spare.
- **`efficiency_threshold` carries a fraction, not a percentage, and the failure
  mode is silence.** The reference prints "5%"; the graph compares two
  dimensionless quotients, so the parameter is 0.05. Writing `5` asks for a
  500-point shortfall and the rule goes quiet forever with no error.
  HP-0001's `cop_ratio_threshold` has the mirror-image trap.
- **`eff_baseline_slope` and `eff_baseline_intercept` ship as documented
  placeholders.** The reference specifies a model, not numbers
  (`baseline_model.predict(boiler_firing_rate)`, fitted over
  `learning_period_days`), and this library's split puts the fitting in the host
  and the fitted line in the graph as `set_param` targets. The shipped
  0.0006 /% and 0.78 describe a conventional non-condensing boiler and exist so
  the document is runnable as delivered. **They are not site values, and a wrong
  pair fails silently in both directions** — fitted five points high, every hour
  alarms; fitted low, nothing ever does. Precedent: HP-0001's COP line,
  VAV-0001's `ventilation_requirement`.
- **The slope may be negative — the documented exception to the library's
  no-negative-parameters convention.** A regression slope is inherently signed
  and here the sign is a fact about the boiler type: conventional efficiency
  rises with fire, condensing falls, and one rule instance must accept either
  without rewiring. HP-0001 carries the identical exception.
- **`alarm_delay` is adopted, not transcribed, because the source line
  truncates.** The reference's tunables line ends at
  "`efficiency_threshold = 5%, learning_period_days = 14,`" and whatever followed
  did not survive the extract. The 60 minutes comes from the two nearest
  authorities — CHW-0001's `AlarmDelay = 60 min` on the same
  fitted-baseline shape, and HP-0001 — and is the shortest delay that reliably
  outlasts a light-off transient on a large boiler.
- **`fuel_power_min` and `yFuelOk` are adopted, not transcribed.** The reference
  names no fuel floor and no evaluability gate, but the graph divides by a live
  signal, and per SCHEMA.md a test computable from the rule's own inputs belongs
  in the graph as a boundary output rather than as prose. `yFuelOk` is not an
  echo of `fuel_power` — it is the comparison the division needs. The 5.0 kW
  default sits above a standing pilot on a small commercial boiler and is
  arbitrary on a 3 MW firetube, where minimum fire alone is hundreds of kW.
- **The heating-value convention is a precondition the rule cannot check.** HHV
  and LHV differ by about 10% for natural gas, so an efficiency computed one way
  against a line fitted the other is off by roughly eight efficiency points —
  more than the whole threshold — and a healthy boiler reads as permanently
  degraded. Nothing in three signals reveals which convention produced them, so
  it lives in `preconditions`. It is the single most likely way to deploy this
  rule wrongly.
- **The nominal boundary case is a FAULT, and that is arithmetic rather than a
  choice.** A five-point shortfall cannot be represented exactly at these
  magnitudes: `0.05` is an odd multiple of 2⁻⁵⁶ while every double in [0.5, 1)
  is a multiple of 2⁻⁵³, so no difference of two efficiencies in that range can
  equal the threshold. 0.760 against 0.810 evaluates to 0.050000000000000044 and
  trips the strict `>`; one ulp lower clears. That is as tightly as the boundary
  can be bracketed, and the card says so rather than implying a precision it
  does not have.
- **The fitted line is extrapolated without limit.** Nothing in the graph knows
  the firing-rate range the regression covered, so a far enough fire drifts the
  expected efficiency into values the fit never supported. The block set has no
  domain guard, so it is a frontmatter precondition; a host that wants it
  enforced can clamp `boiler_firing_rate` with `Reals.Limiter` upstream.
  HP-0001 documents the same open end.
- **One regressor, which is the reference's choice and a real blind spot.** For
  a condensing boiler the dominant variable is return water temperature — the
  same burner at the same fire condenses at 40 °C return and does not at 60 °C,
  several efficiency points apart — so a plant whose return temperature tracks
  the weather shows scatter this line cannot explain. The point dictionary
  carries no HW return temperature and the reference's Required Points list has
  three entries; a second regressor would be a different card. Condensing sites
  should widen `efficiency_threshold` or restrict the evaluated operating states.
- **`learning_period_days` (14 d) stays a host precondition.** It gates a
  fitting run that happens offline, outside any tick, and nothing in the block
  graph could observe it.
- **`method: statistical` describes the baseline's provenance, not the
  runtime.** The graph performs one division, one multiply-add, one subtraction
  and two comparisons; the classification is honest because the coefficients
  come from a regression. HP-0001 and RTU-0002 carry the same note.
- `persist.delayOnInit = true` (CDL default `false`), the library's standing
  choice: a boiler already below its line at controller restart waits out the
  full hour rather than alarming on the first tick.
- **`clusters: []`.** `clusters/clusters.json` defines no cluster containing a
  hot water plant rule, and this card does not edit the cluster set. CLU-06 is
  the chilled-water analogue this fault would head on the heating side if such a
  cluster existed.
- **No test vectors are transcribed, because the reference publishes none.** All
  fifteen scenarios in `vectors.json` are authored from the equation and
  replayed against the pinned engine rev.
- Severity 3, phase 2, `method: statistical`, `confidence: HIGH` and the 5%
  threshold are the reference's chapter 14 card. `g36: null` — research-derived
  (Meng et al. 2021; Shohet et al. 2020), not a G36 clause.

## Notes

Read `yFuelOk` before `yFault`. The two outputs are what separate a repair from
a burner that simply stopped: a host treating the falling edge of `yFault` as a
fix will close this fault every night the boiler shuts down.

Whatever schedules the learning run must refuse to re-fit while this fault is
active. The line comes from the boiler's own history, so re-fitting after
degradation has developed bakes it in as the new normal and the rule reports
healthy on a machine everyone agrees is wasting gas.

Work the diagnoses in the order a combustion analyser can see them: stack
temperature and oxygen at high and low fire separate the fuel/air ratio (3) and
FGR (4) from the heat-transfer causes (1, 2, 5) in about twenty minutes.
HW-0001 reads the same boiler from the cycling side and is worth checking
first — a plant tripping both may have one problem.
