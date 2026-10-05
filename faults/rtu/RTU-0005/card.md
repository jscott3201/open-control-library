---
schema: cxf-library/fault-card/v1
id: RTU-0005
name: Excess outdoor air intake
equipment: rtu
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §11, RTU-0005"
  - "PNNL-23790 (RTU AFDD4/AFDD5)"
  - "PNNL EEM-17 (demand control ventilation)"
  - "PNNL EEM-23 (RTU advanced controls)"
g36: null
clusters: []
suppresses: []
suppressed_by: [AHU-0028]
related: [AHU-0021, AHU-0030, RTU-0006, RTU-0004, RTU-0010]
playbooks: [economizer-failure]
operating_states: "occupied operation with the economizer locked out (host-gated); the reference's economizer_should_be_inactive(oat, mode) term lives in preconditions, not in the graph"
preconditions: "Supply fan running. The host must not evaluate while the economizer is legitimately open — drawing more than the design minimum is the point of economizing, and these three temperatures cannot tell that apart from a damper that never closed. MAT must pass its integrity gate (AHU-0028, see suppressed_by): the fraction is a ratio of temperature differences, so a biased mixed-air reading moves it directly. The temperature-difference gate is signalled in-rule by yTempDeltaOk; when it is false the verdict is NO_EVAL, not healthy."
points:
  - oat
  - rat
  - mat
outputs:
  - name: yFault
    description: True while the outdoor air fraction has stayed more than oa_excess_margin above design_min_oa_fraction for at least alarm_delay, with the temperature difference large enough to evaluate
  - name: yTempDeltaOk
    description: Evaluability signal — true when |oat − rat| exceeds min_delta; false means NO_EVAL and the host must ignore yFault
params:
  design_min_oa_fraction:
    default: 0.15
    unit: "1"
    description: Design minimum outdoor air fraction the unit should hold when it is not economizing (0–1)
    cxf: designConst.k
  oa_excess_margin:
    default: 0.15
    unit: "1"
    description: Tolerance above the design minimum before the excess counts as a fault
    cxf: marginHigh.t
  min_delta:
    default: 6.0
    unit: "°C"
    description: Minimum |oat − rat| for the fraction to be meaningful; below it the rule is not evaluable
    cxf: deltaOk.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: RTU thermal energy spent conditioning excess ventilation air
  savings_range: 2-10% of unit thermal energy (HVAC FDD Reference §11; PNNL EEM-17, EEM-23)
  climate_sensitivity: heating-dominant
  runtime_estimation: "excess_oa_kw = (actual_oaf − design_min_oa_fraction) × airflow × cp × |oat − rat|"
emissions:
  scope: "1+2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:403a34cdcdb48f2f6c7f4827a80b4ab9"
  date: 2026-10-05
---

## Description

The unit is drawing well over its design minimum outdoor air at an hour when it
has no business economizing. Every extra cubic metre arrives at outdoor
temperature and has to be dragged to supply temperature by the gas heat or the
compressors, and none of it buys ventilation the code did not already have.
Nothing about it is uncomfortable — the space stays on setpoint, the unit simply
runs harder — so the defect survives until someone reads the fuel bill. Packaged
units make it common: economizer, minimum-position setting and dampers all live
in one weather-exposed cabinet, and Cowan's 2004 survey found at least one
economizer fault on 54% of units. The outdoor air fraction is inferred from the
mixing-box energy balance rather than measured, which is what makes the
diagnostic cheap — three temperatures, no airflow station — and also what makes
it conditional, hence the explicit evaluability output.

## Detection Logic

```
oaf          = (mat − rat) / (oat − rat)
yTempDeltaOk = |oat − rat| > min_delta                  (false ⇒ host reports NO_EVAL)
yFault       = (oaf − design_min_oa_fraction > oa_excess_margin) AND yTempDeltaOk,
               sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![RTU-0005 block graph](diagram.svg)

The fraction core is AHU-0021's, unchanged, bound to the RTU point dictionary.
`gate` is what makes the unguarded division safe: CDL `Divide` follows IEEE-754,
so `oat = rat` yields ±∞ or NaN and a near-zero denominator amplifies ordinary
sensor noise into a fraction of any magnitude. NaN compares false everywhere, and
±∞ or a noise-inflated finite fraction can raise `marginHigh` but cannot pass
`gate`, because a denominator small enough to misbehave is by construction one
below `min_delta`. Garbage arithmetic can only make the rule report itself
unevaluable. `yTempDeltaOk` leaves the block as well as feeding the gate, so a
host that reads it learns the difference between "not faulted" and "cannot
tell". Both comparisons are strict: a fraction sitting exactly at
`design_min_oa_fraction + oa_excess_margin` is not a fault, and a temperature
difference of exactly `min_delta` is not evaluable. The quotient is signed
consistently on both sides of the year — winter makes both differences negative,
summer both positive — so no seasonal branch is needed. `persist` requires 30
continuous minutes, riding out a damper stroke and the mixing transient after a
stage change; recovery is immediate, and `delayOnInit = true` holds the window
across a restart.

## Possible Diagnoses

1. OA damper minimum position set too high
2. OA damper not closing to the commanded minimum
3. Damper blade seals deteriorated
4. Economizer lockout not engaging

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, DIRECT_MEASUREMENT. The waste is computable
from live data: `excess_oa_kw = (actual_oaf − design_min_oa_fraction) × airflow
× cp × |oat − rat|`, with the excess fraction already on the wire as
`oaf − designConst.k`. Correcting minimum ventilation is worth 2–10% of the
unit's thermal energy, the upper half of that range in heating-dominant climates
where the outdoor-to-return difference is largest for months at a time. The
reference maps the fault to PNNL EEM-17 (demand control ventilation) and EEM-23
(RTU advanced controls); this rule screens both, since a unit that cannot hold
its design fraction will not benefit from either until the mechanical problem is
fixed.

## Emissions Impact

Scope 1 + 2, DIRECT_EMISSIONS, HIGH confidence; typically 300–2,500 kg CO₂e/yr
for the excess ventilation load. The split follows the season and the unit:
winter excess burns scope 1 gas at the furnace section, summer excess draws
scope 2 electricity at the compressors, and on an all-electric packaged unit the
whole exchange collapses to scope 2. Avoided-emissions basis: marginal operating
emissions rate (MOER).

## Deviations

- **`min_delta` default adopted, not transcribed.** The reference states the
  fraction is computed only when `|OAT − RAT| > min_delta` but omits the
  parameter from its tunables table. This card adopts 6.0 °C, matching
  AHU-0021, AHU-0030 and RTU-0006 so every rule running this quotient
  agrees on when it is meaningful (PNNL-27338 uses 5 °F for the same
  computation). A site that retunes one should retune all of them.
- **The reference's `economizer_should_be_inactive(oat, mode)` term is not in
  the block graph.** Economizer state is an operating state, not a measurement,
  and this library keeps state gating host-side (precedent: AHU-0021, whose
  reference card carries the same term). A host that evaluates this rule while
  the unit is economizing gets a sustained fault, and it is the host's bug;
  `mode` is not a canonical RTU point in any case.
- **Design fraction as a constant, excess as a threshold.** The reference writes
  `oa_fraction > (design_min_oa_fraction + oa_excess_margin)`, which implemented
  literally would sum the two tunables into one threshold and stop a host
  retuning either alone. Feeding the design fraction in as
  `Reals.Sources.Constant.k` keeps both as independent `set_param` paths with no
  sign flips. Algebraically identical.
- **Evaluability is an output, not just a precondition.** The `min_delta` test
  is computable from this rule's own inputs, so SCHEMA.md requires exposing it
  as `yTempDeltaOk`. False `yFault` under false `yTempDeltaOk` means "unknown",
  not "healthy", and the host must treat it that way.
- **Both comparisons are strict** (`>`). The reference does not specify boundary
  behavior and CDL `Reals` offers no `GreaterEqual`, so the choice is made
  rather than inherited; the disagreement with an inclusive reading has measure
  zero on a real temperature signal and errs toward silence.
- The reference publishes no worked vectors for this fault, so every scenario in
  `vectors.json` is authored from the equation, following AHU-0021's suite
  shape.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: an excess already present at load waits out the full 30
  minutes instead of alarming on the first tick after a restart.

## Notes

`suppressed_by: [AHU-0028]` is transcribed from the reference and points
across equipment families on purpose: AHU-0028's graph consumes nothing but
`mat`, `oat` and `rat`, so the host instantiates it against this RTU's own three
points. Deploy the pair together — a MAT sensor reading 4.5 °C low in −5 °C
weather turns a compliant 0.15 fraction into 0.32 and manufactures this fault
out of nothing, and on a milder day with a 10 °C spread under 2 °C of bias does
the same.

RTU-0006 is this rule's mirror on the same three temperatures: this one alarms
more than 0.15 above design, that one more than 0.05 below. The margins are
deliberately asymmetric — excess air costs money, deficient air costs air
quality — and neither can fire while the other does. If the fraction is
genuinely high, command the OA damper to minimum and watch mixed air: it should
climb toward return temperature within minutes, and if it does not the problem
is mechanical (the [economizer-failure](../../../playbooks/economizer-failure.md)
playbook's on-site steps). Check the minimum position setpoint first — the most
common cause is a number dialled up during a ventilation complaint, and that is
a $0 fix.
