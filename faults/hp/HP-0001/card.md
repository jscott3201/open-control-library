---
schema: cxf-library/fault-card/v1
id: HP-0001
name: COP degradation vs baseline
equipment: hp
status: verified
phase: 2
method: statistical
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: BASELINE_COMPARISON
source:
  - "HVAC FDD Reference v1.0 §11, HP-0001"
  - "Barandier 2023"
  - "Barandier & Mendes 2024"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HP-0002, HP-0003, HP-0004, HP-0005, RTU-0002, HP-0007, HP-0008]
playbooks: [heat-pump-faults]
operating_states: "heating or cooling, evaluated separately — one rule instance per mode, each carrying that mode's fitted line"
preconditions: "The compressor must have run for min_runtime_for_eval (15 min) at its current capacity before the quotient means anything; a unit still pulling down after a start, or coming out of a defrost cycle, reads degraded on physics rather than on fault. The host owns the baseline: it runs the learning_period_days (14 d) regression of COP against oat for THIS mode, confirms R² > 0.6, and writes the result into cop_baseline_slope and cop_baseline_intercept with set_param. Until it has done so the rule is comparing against the shipped placeholders and means nothing (see Deviations). oat must also lie inside the range the line was fitted over — the graph extrapolates the line forever and says nothing about where the fit stops being physical. thermal_power is almost always a host-computed virtual point; its provenance is part of the R² precondition, not separate from it. Compressor evaluability is signalled in-rule by yPowerOk; when it is false the verdict is NO_EVAL, not healthy."
points:
  - thermal_power
  - elec_power
  - oat
outputs:
  - name: yFault
    description: True while the measured COP has stayed below cop_ratio_threshold × the fitted baseline for the current outdoor temperature, continuously for at least alarm_delay
  - name: yPowerOk
    description: Evaluability signal — true when elec_power is above elec_power_min, the floor below which the COP quotient is meaningless; false means NO_EVAL and the host must ignore yFault
params:
  cop_baseline_slope:
    default: 0.08
    unit: "1/°C"
    description: Slope of the host-fitted COP-vs-oat regression. PER-UNIT, PER-MODE SITE CONFIGURATION — the reference supplies a learned model, not a number, and the shipped 0.08 is a placeholder for a generic air-source heat pump in heating. Inherently signed; a cooling-mode fit is negative.
    cxf: oatSlope.k
  cop_baseline_intercept:
    default: 2.7
    unit: "1"
    description: Intercept of the same regression — the expected COP at oat = 0 °C. PER-UNIT, PER-MODE SITE CONFIGURATION on the same terms as the slope; the pair is only meaningful together.
    cxf: expected.p
  cop_ratio_threshold:
    default: 0.85
    unit: "1"
    description: "Fraction of the baseline COP the unit must stay above. 0.85 is the reference's 15% degradation threshold written as a ratio: fault when measured < 0.85 × expected."
    cxf: allowed.k
  elec_power_min:
    default: 0.5
    unit: kW
    description: Compressor draw below which the COP quotient is not evaluated. Guards the division — at zero draw the quotient is NaN, and at a standby trickle it reads as total degradation. Retune to the smallest real compressor draw the unit produces at minimum capacity.
    cxf: pwrOk.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: Continuous degradation required before the alarm asserts (60 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Heat pump compressor energy
  savings_range: 5-25% compressor energy; refrigerant undercharge is the most frequent cause (Barandier 2023)
  climate_sensitivity: both
  runtime_estimation: "waste_kw = elec_power × (1 − measured_cop / expected_cop) — the share of the compressor's current draw that buys nothing, since a unit at 80% of its baseline COP spends 20% of its input on the degradation"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:4e34c94f444594a4a9ccdce25c4eebef"
  date: 2026-10-05
---

## Description

A heat pump has no COP it is supposed to hold — the same machine returns four
units of heat per unit of electricity on a mild afternoon and two on a cold
night, and neither number is a fault. What it has, per unit and per mode, is a
line: COP against outdoor air temperature, fitted by the host over two weeks of
normal operation. This rule evaluates that line, alarming when measured COP
stays below 85% of what the line predicts at today's `oat`. Everything
statistical happens before the first tick. Barandier (2023) found refrigerant
undercharge the most frequent heat pump fault, and it is what this rule sees
best: COP falls across the whole operating range while no single reading looks
wrong on its own.

## Detection Logic

```
measured_cop = thermal_power / elec_power
expected_cop = cop_baseline_slope × oat + cop_baseline_intercept
allowed_cop  = cop_ratio_threshold × expected_cop

yPowerOk = elec_power > elec_power_min          (false ⇒ host reports NO_EVAL)
yFault   = measured_cop < allowed_cop AND yPowerOk,
           sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HP-0001 block graph](diagram.svg)

`oatSlope` and `expected` are the fitted line; `allowed` scales it by the
tolerance, so the test is against a second line parallel to the first rather
than against a number.

`cop` is the rule's only division and its denominator goes to zero every time
the compressor stops; `pwrOk` guards it. With both meters at zero the quotient
is NaN; with a standby trickle and no heat output it is a clean, believable
0.0 — the more dangerous case, since nothing downstream would flag it. `pwrOk`
drives both the boundary output `yPowerOk` and `gate`, so a unit below the floor
holds `yFault` down and the host reads the silence as NO_EVAL, not healthy.

The comparison is strict, so a unit exactly on the allowed line reads healthy.
`persist` requires 60 continuous minutes of shortfall — long enough to ride out
a defrost cycle, a capacity step, or a load transient — and `delayOnInit = true`
holds that window across a controller restart.

## Possible Diagnoses

1. Refrigerant undercharge — the most common heat pump fault (Barandier 2023);
   check subcooling and superheat at the service ports first
2. Refrigerant overcharge, which degrades COP the same way and is less common
3. Condenser or evaporator coil fouling — the pathology RTU-0002 detects from
   the air side; rule it out with a filter change before opening anything up
4. Compressor degradation — worn valves or bearings raise amp draw against
   nameplate for the same delivered capacity
5. Non-condensable gases in the refrigerant circuit, usually from a service
   procedure that skipped or shortened the evacuation

## Energy Impact

EFFICIENCY_LOSS, MEDIUM confidence, BASELINE_COMPARISON.
`waste_kw = elec_power × (1 − measured_cop / expected_cop)` — the share of the
compressor's draw that buys nothing. Range 5–25% of compressor energy
(Barandier 2023), spanning a small charge loss at one end and a badly fouled
coil or failing compressor at the other. MEDIUM for a structural reason: the
baseline is the unit's own recent behavior, so the rule measures degradation
*since the learning period* — a heat pump commissioned undercharged learns an
undercharged baseline and reads healthy forever.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, MEDIUM confidence; typically 300–2,500 kg CO₂e/yr for
a commercial packaged heat pump. All of it is compressor electricity, so the
avoided-emissions basis is the marginal operating emissions rate (MOER).
Degradation costs most at the extremes of the outdoor temperature range, when
the unit runs longest and the grid is dirtiest, so a charge correction is worth
more than the annual average kWh figure suggests.

## Deviations

- **`cop_baseline_slope` and `cop_baseline_intercept` ship as placeholders, not
  site values.** The reference specifies a regression the host fits (14 days,
  R² > 0.6); the fitting lives in the host, the fitted line in the graph as two
  `set_param` targets. The shipped 0.08 /°C and 2.7 describe a generic air-source
  heat pump in heating so the card is runnable as delivered — a wrong pair fails
  silently: fit high and every hour alarms, fit low and nothing ever does.
- **A negative `cop_baseline_slope` is the documented exception to the library's
  no-negative-parameters rule.** A regression slope is inherently signed —
  positive for a heating fit, negative for a cooling fit — and one instance must
  accept either without being rewired; pushing the sign into the topology would
  mean two graphs for one fault.
- **The degradation test is a ratio, not the reference's fraction.** The
  reference writes `(expected_cop − measured_cop) / expected_cop > 0.15`; this
  computes `measured_cop < 0.85 × expected_cop`, the same predicate for any
  positive `expected_cop` and without a second division by a fitted line. Note
  the units: `cop_ratio_threshold` is the fraction retained (0.85), not the
  percentage lost — writing `15` into it makes the rule alarm permanently.
- **`elec_power_min` and `yPowerOk` are adopted, not transcribed.** The reference
  names no power floor; its evaluability gate is `min_runtime_for_eval`, which
  the graph cannot see, and per SCHEMA.md a test computable from the rule's own
  inputs belongs in the graph as a boundary output. The 0.5 kW default suits a
  small commercial packaged unit — raise it for a large compressor, lower it for
  a variable-speed unit that genuinely modulates below it.
- **`min_runtime_for_eval` (15 min) and `learning_period_days` (14 d) stay host
  preconditions.** Both gate on things outside the graph's view: time since a
  capacity transition, and an offline fitting run. The 60-minute `alarm_delay`
  covers post-start pull-down in steady operation but does not substitute — a
  unit that takes 20 minutes to settle spends a third of the window degraded.
- **The fitted line is extrapolated without limit.** Nothing in the graph knows
  the temperature range the regression covered; far enough out a cooling-mode
  line goes negative, `allowed` with it, every positive measured COP clears and
  the rule goes quiet. No block expresses a domain guard, so it is a frontmatter
  precondition; a host can clamp `oat` with `Reals.Limiter` upstream.
- **Mode separation is instance-level.** The reference evaluates heating and
  cooling separately and this rule has no mode input: one line, one alarm. A
  reversible unit runs two instances with two fitted pairs, and the host enables
  whichever matches the mode currently commanded.
- **Strict `<` at the allowance, where the playbook reads inclusively.** The heat
  pump playbook's step 1.a calls a 15% or greater drop below the baseline curve
  degradation, but CDL Reals has no `LessEqual`, so the strict form is the
  expressible one and a unit sitting exactly on the allowed line reads healthy.
  Both sides of the boundary are pinned; the disagreement is measure-zero.
- **`method: statistical` describes the baseline's provenance, not the runtime.**
  The graph performs one division, one multiply-add, one scale and one
  comparison. The coefficients come from a regression and the reference's
  classification is recorded rather than relabelled; RTU-0002 carries the same
  note.
- `persist.delayOnInit = true` (CDL default is `false`), the library's standing
  choice: a unit already below its line at controller start waits out the full
  hour rather than alarming on the first tick.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph. Severity 3 and
  `method: statistical` are the reference's chapter 11 card; its §5.8.4 index
  carries no severity column.
- The reference publishes no test vectors, so every scenario in `vectors.json`
  is authored from the equation and replayed against the pinned engine rev.

## Notes

Read `yPowerOk` before `yFault`. A repair and a compressor stop are
indistinguishable in `yFault` alone — both drop the alarm — so a host that
treats the falling edge as a fix will close this fault every time the unit
finishes a cycle. The
[heat-pump-faults](../../../playbooks/heat-pump-faults.md) playbook orders the
on-site work by prevalence and calls the fault resolved when COP returns to
within 10% of the baseline, tighter than the 15% this rule alarms at. Do not
re-fit the baseline while the fault is active: fitting from a degraded unit's
own history bakes the fault in as the new normal.
