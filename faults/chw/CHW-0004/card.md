---
schema: cxf-library/fault-card/v1
id: CHW-0004
name: Chilled water low delta-T syndrome
equipment: chw
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "HVAC FDD Reference v1.0 §13 (ch. 'Chilled Water Plants', pdf pp. 122-123), CHW-0004"
  - "PNNL-27338 §3"
  - "PNNL-25985"
g36: null
clusters: [CLU-06]
suppresses: []
suppressed_by: []
related: [CHW-0001, CHW-0003, AHU-0014, FCU-0004]
playbooks: [low-delta-t]
operating_states: "chilled water plant producing, with the chiller loaded above min_load_for_eval — the rule's own yLoadOk is that state"
preconditions: "chwst, chwrt and chiller_load must describe the same hydraulic loop at the same moment. On a primary/secondary plant that is the precondition most often violated: the chiller's own entering/leaving temperatures see primary flow and read a healthy delta-T while the secondary loop that actually serves the coils is short-circuiting through the decoupler, which is the fault. Bind the temperatures where the coils are — the secondary supply and return headers on a decoupled plant, the chiller connections on a variable-primary one — and bind chiller_load from the same plant. Both temperatures must be in °C (the rule converts nothing) and design_delta_t must be this loop's design value, not the shipped 5.6 °C, before any verdict means anything. The two sensors must also be in the right places: nothing in the rule can tell a swapped supply/return pair from a genuine low delta-T (see Deviations), and a supply sensor reading high biases delta-T low in exactly the direction that alarms. Sensor calibration is worth confirming before a first deployment, because a 0.5 K offset on a 2.8 K trip line is 18% of the decision. Evaluability is signalled in-rule by yLoadOk: when it is false the verdict is NO_EVAL, not a healthy plant."
points:
  - chwst
  - chwrt
  - chiller_load
outputs:
  - name: yFault
    description: True while the chilled water delta-T has stayed below design_delta_t × low_dt_fraction with the chiller loaded above min_load_for_eval, continuously for at least alarm_delay
  - name: yLoadOk
    description: Evaluability signal — true when chiller_load is above min_load_for_eval, the load below which a small delta-T says nothing about the plant. False means NO_EVAL and the host must ignore yFault
params:
  design_delta_t:
    default: 5.6
    unit: "°C"
    description: "Design chilled water delta-T (the reference's 5.6 °C = 10 °F). PER-LOOP SITE CONFIGURATION — read it off the plant's design documents; a 6.7 K (12 °F) plant and a 4.4 K (8 °F) plant are both common and neither is served by the shipped value."
    cxf: designDt.k
  low_dt_fraction:
    default: 0.5
    unit: "1"
    description: "Fraction of design delta-T below which the plant is faulted (the reference's 50%). Kept as its own parameter rather than folded into the trip line so that a site can retune the tolerance and the design value independently — see Deviations."
    cxf: lowDtLimit.k
  min_load_for_eval:
    default: 40.0
    unit: "%"
    description: "Chiller load below which delta-T is not evaluated. The reference's min_load_for_eval: a lightly loaded plant has a small delta-T because there is little load, not because anything is wrong."
    cxf: loadOk.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous low delta-T at load required before the alarm asserts (60 min). The reference's AlarmDelay, renamed to the library's convention"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: CHW pump energy + staging inefficiency
  savings_range: "5-15% pump energy; forces extra pumping"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "excess_pump_kw ≈ chw_pump_kw × (design_dt − actual_dt) / design_dt — the reference's formula. chw_pump_kw is not one of this rule's points and is not in the CHW dictionary, so the host supplies it; actual_dt is the delta-T the graph already computes"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, one July week, plant mode at 60 s"
    scenarios: 1
    failures: 0
    notes: "single RunPeriod with timeline/cadence validation; gated only where chiller load exceeded 40%. The January rerun had no evaluable loaded window and is not counted; supersedes earlier outputs whose 900 s source cadence had been mislabeled as 300 s"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:f12c5169f0aa1755a37402fe2dbfe04a"
  date: 2026-10-05
---

## Description

A chilled water plant is sized on a temperature difference, not on a flow.
Design the coils for 5.6 K between supply and return and the pumps move enough
water to carry the peak load; let that difference fall to 2 K and the same load
needs nearly three times the flow, so the pumps run faster, the second pump
starts, and eventually a second chiller comes on to make water the first could
have made if the water had come back warm enough to use. That is low delta-T
syndrome, and its signature is that nothing looks broken — every zone is
comfortable and the only symptom is a plant working much harder than the
building it serves. It is measured at the plant because that is where the
individual causes add up: a bypassing three-way valve, a fouled coil, a filter
nobody changed, each too small to see from the AHU that owns it.

## Detection Logic

```
delta_t   = chwrt − chwst
low_limit = design_delta_t × low_dt_fraction        (5.6 × 0.5 = 2.8 K)

yLoadOk = chiller_load > min_load_for_eval          (false ⇒ host reports NO_EVAL)
yFault  = delta_t < low_limit AND yLoadOk,
          sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![CHW-0004 block graph](diagram.svg)

`designDt` and `lowDtLimit` assemble the trip line inside the graph rather than
shipping a pre-multiplied 2.8, so both of the reference's numbers survive as
independent `set_param` targets; they are retuned for different reasons.

`lowDt` is strict, so a plant sitting exactly on the trip line reads healthy,
and the boundary is bit-exact rather than approximate: 5.6 halved is the double
nearest 2.8, which a realistic temperature pair can reach exactly, so the
comparison is decided by the strictness and not by rounding. `loadOk` is the
reference's `min_load_for_eval` and the whole NO_EVAL story — at 20% load a 1 K
delta-T is what a healthy plant produces, and exposing the conjunct as `yLoadOk`
lets the host tell that from a plant that is loaded and fine. `persist` requires
60 continuous minutes and carries `delayOnInit = true`; low delta-T is a plant
condition, and anything shorter is a valve stroking or a coil catching up.

## Possible Diagnoses

Transcribed from the reference's CHW-0004 card:

1. Three-way valve bypass allowing CHW to short-circuit — the classic cause, and
   worst as the building unloads, because that is when the bypass is widest
2. AHU/FCU coil fouling, so the water leaves the coil colder than it should.
   AHU-0014 and FCU-0004 see this from the air side, one unit at a time
3. Low airflow across cooling coils — dirty filters, a slow fan, a closed damper
4. CHW valve leaking or stuck partially open — the same arithmetic as a bypass
   valve with a different part number
5. Oversized CHW system relative to actual load — the case with no repair, where
   the delta-T is telling the truth

Causes 1 through 4 are local defects this plant-level rule aggregates: a building
with forty coils can reach the trip line with four misbehaving and thirty-six
fine, which is what makes the finding hard to chase and worth having.

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, PROXY_ESTIMATION. The reference's
estimator is `excess_pump_kw ≈ chw_pump_kw × (design_dt − actual_dt) / design_dt`
— a plant at 2.8 K on a 5.6 K design spends about half its pump energy on water
that comes back too cold to be worth moving — with a published range of 5–15% of
pump energy. That understates the cost: the expensive consequence is staging, a
second chiller serving a load the first could have carried, and the compressor
energy dwarfs the pumps. The reference names "staging inefficiency" without
putting a number on it and neither does this card, because the number depends on
the plant's staging logic. Confidence is MEDIUM because the finding is
plant-level and the repair is not. Cooling-dominant.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, MEDIUM confidence; the reference's typical range is
200–2,000 kg CO₂e/yr for pump and staging inefficiency together, on a marginal
operating emissions rate basis. All of it is purchased electricity, so the scope
does not vary by site the way a heating fault's does, and the timing works
against the building: low delta-T bites hardest on the hottest afternoons, which
are also the hours when the marginal generator is dirtiest.

## Deviations

- **The trip line is assembled in the graph rather than folded into a
  threshold.** A single `Reals.LessThreshold` with `t = 2.8` would compute the
  same verdict with one block instead of three and would lose both of the
  reference's tunables — a site with a 6.7 K design could no longer change it
  without recomputing the product, and the 50% fraction would stop being visible.
  Precedent: VFD-0002's assembled speed floor, which adds two constants where
  this rule multiplies, because the reference's composition is a product.
- **Strict `<` at the trip line.** The reference writes `<` too, and CDL `Reals`
  has no `LessEqual` in any case, so a plant at exactly 2.8 K reads healthy. The
  disagreement is measure-zero, and the on-the-line vector uses a 5 °C supply
  temperature rather than the published vectors' 6 °C for an arithmetic reason:
  8.8 − 6.0 lands one ulp above 2.8 and would have pinned the wrong side.
- **Strict `>` at the load floor, same treatment.** The reference writes
  `chiller_load > min_load_for_eval`, so a chiller at exactly 40% is NO_EVAL.
- **`yLoadOk` is the library's shape for the reference's NO_EVAL row.** The
  reference writes the load test as a conjunct of the fault condition and
  publishes a NO_EVAL vector for it; the graph computes that conjunct and
  additionally exposes it as a boundary output, which adds no logic and changes
  no verdict. It is a comparison of an input against a parameter rather than an
  echo of an input, which is what SCHEMA.md asks. Same stance as HP-0001's
  `yPowerOk`.
- **Nothing guards against an inverted delta-T.** A swapped supply/return pair,
  or sensors on the wrong side of a decoupler, produces a negative delta-T that
  is below any positive trip line and alarms permanently. The block set could
  express a guard, but suppressing negative delta-T would also suppress the
  genuine short-circuit case on a plant whose sensors are fine, so the honest
  answer is a documented blind spot. Commissioning check: swap the leads and
  watch the sign, once, before trusting the rule.
- **The rule is blind to which coil is responsible, and to how many.** That is
  the reference's design rather than a simplification — the individual coil
  faults are usually too small to detect one at a time, which is why the syndrome
  is measured in the return header. AHU-0014 and FCU-0004 are the coil-side
  rules worth running alongside it; neither is wired to this one.
- **Persistence stands in for averaging.** The rule consumes instantaneous points
  and the reference specifies no averaging, so a delta-T alternating above and
  below the line every 20 minutes never accumulates the hour and never alarms,
  though a plant spending half its day low is a genuine finding. A steady
  syndrome — a bypassing valve or a fouled coil — reads the same either way.
- **`AlarmDelay` is renamed `alarm_delay`.** The reference's tunables table
  spells the fault-persistence parameter in G36's PascalCase while spelling its
  three neighbours in snake_case; the library uses `alarm_delay` throughout and
  the value is unchanged at 60 min.
- `persist.delayOnInit = true` (CDL default `false`), the library's standing
  choice: a plant already below the line at controller restart waits out the full
  hour rather than alarming on the first tick.
- **`chiller_load` is per-chiller while the delta-T is per-loop.** On a
  multi-chiller plant the load signal belongs to one machine and the header
  temperatures to the loop, so a plant running two chillers at 45% each is
  evaluated on one of them. The reference names the same single point and does
  not address the case; bind the lead chiller or a host-computed plant load.
- **Three published test vectors, the rest authored.** The reference publishes
  normal delta-T, low delta-T and low load (NO_EVAL); all three are transcribed
  into `vectors.json` and pass. The remainder — the trip line and load floor
  boundaries, the mid-run collapse and recovery edges, the evaluability release,
  the intermittent case and the swapped-sensor blind spot — are library-authored.
- **The chapter's Notes line for this fault is truncated in the source
  extract.** It reads "One of the most common and costly CHW plant issues.
  Forces" and stops. Nothing here depends on the missing clause: the "forces
  extra pumping" reading in `energy_impact.savings_range` comes from the
  chapter's own Savings Range row, which is complete.
- **`clusters: [CLU-06]` is the existing cluster set's membership, not this
  card's authorship.** `clusters/clusters.json` already lists this fault as a
  member of "Chilled Water Plant Inefficiency" with CHW-0001 as the trigger,
  and that cluster's `playbook` slug resolves to
  `playbooks/chiller-efficiency.md`.
- **`playbooks` cites `low-delta-t`,** the reference's own playbook, whose
  Applies-To row names this card directly.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph, per the library's design
  stance. Severity 3, `method: rule` and the fault name are the reference's
  chapter 13 card.

## Notes

Source-pointer precision: the reference's ch.13 card cites "PNNL-27338 §3", but
in PNNL-27338's own numbering the low delta-T algorithm sits in the hot-water
distribution chapter (§4.6), and that document names chilled-water diagnostics
as future work. The citation is an analog/pattern source — the HW delta-T
algorithm mirrored to CHW — not a CHW-specific specification.

Read `yLoadOk` before `yFault`. A plant that is off, or coasting through a mild
morning at 25% load, holds `yLoadOk` false for hours, and every `yFault = false`
underneath it means "not evaluated" rather than "delta-T is fine".

Trend delta-T against plant load for a week before sending anyone: a delta-T
that degrades as the building unloads points at bypass and leaking control
valves, while one that is flat and low across the range points at fouling or at
a plant oversized for the building. Then check the largest coils, because the
syndrome is a sum. CHW-0001 shares this rule's cluster and the two reinforce
each other — extra pumping and an early-staged chiller both push kW/ton up — so
treat the delta-T as the trigger and the efficiency alarm as its consequence.
