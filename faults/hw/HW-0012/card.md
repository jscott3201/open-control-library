---
schema: cxf-library/fault-card/v1
id: HW-0012
name: Excess boiler stages at low plant load
equipment: hw
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "LBNL Simulated Boiler Plant dataset inventory, PDF pp.4-8 — two identical parallel boilers, staged heat-load control, status channels, and useful secondary-loop power; it does not publish the shipped generic thresholds or an over-staging fault"
  - "PNNL Heating Plant Equipment Modeling Inputs — boiler type, sizing, efficiency, and stage-up part-load ratios are plant-specific inputs, supporting an adoption-blocking staging map rather than a portable threshold"
  - "Library precedent RTU-0001/TOWER-0003 — verified native integer stage/count comparison and initialization-safe continuous persistence"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0001, HW-0002, HW-0011]
playbooks: [hot-water-plant-faults]
operating_states: "normal automatic boiler staging after fleet, availability, load basis, minimum-time, and stage-transition state have settled"
preconditions: "boiler_stage_count must count proven firing staging units in one configured eligible fleet, not enabled or available units. A unit may be a whole boiler or a comparable modular burner section, but unlike units must not be mixed in one scalar count. hw_plant_load_fraction must be useful plant heating load divided by one commissioned eligible-fleet capacity basis, remain in 0..1, and keep that denominator stable through the evaluation window. If fleet membership or capacity basis changes, restart evaluation or report NO_EVAL. The threshold and allowed count require site commissioning against sizes, turndown, minimum flow, venting, emissions, redundancy, and minimum on/off policy; the shipped numbers are adoption-blocking placeholders. Exclude warm-up, freeze protection, emergency redundancy, exercise, maintenance, rotation, recent starts/stops, stage overlap, and intentional safety/application limits. OAT alone is not a valid load proxy. Freshness, alignment, finite values, and derivation provenance remain host obligations."
points:
  - boiler_stage_count
  - hw_plant_load_fraction
outputs:
  - name: yFault
    description: True after a valid low-load plant operates above the commissioned allowed firing-stage count continuously for sustained_duration
  - name: yLoadOk
    description: Evaluability output; true only for an inclusive 0..1 load fraction. False means NO_EVAL and every other output is uninterpretable
  - name: yLowLoad
    description: Raw diagnostic; true when load fraction is strictly below low_load_fraction, including for invalid negative values, so consult yLoadOk first
  - name: yExcessStages
    description: Raw diagnostic; true when firing stage count is strictly above max_stages_at_low_load
params:
  low_load_fraction:
    default: 0.35
    unit: "1"
    description: "NO_PORTABLE_DEFAULT: executable adoption-blocking placeholder. Replace from the commissioned staging map, boiler sizes, turndown, minimum-flow, and emissions constraints before deployment."
    cxf: lowLoad.t
  max_stages_at_low_load:
    default: 1
    unit: "1"
    description: "NO_PORTABLE_DEFAULT: executable adoption-blocking placeholder. Replace with the allowed comparable firing-unit count for the configured fleet and load region."
    cxf: excessStages.t
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE continuous over-staging proof. Set beyond ordinary stage overlap, rotation, and minimum-on/off transitions."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Boiler fleet staging, jacket/standby loss, purge/light-off loss, and plant efficiency
  savings_range: "Site-dependent; excess firing units can move each boiler below useful modulation and add jacket, standby, and purge loss"
  climate_sensitivity: heating-dominant, especially low-load shoulder periods
  runtime_estimation: "QUALITATIVE_ONLY. Fault hours do not determine avoidable fuel without per-boiler input, useful load, staged-efficiency curves, and a commissioned counterfactual."
emissions:
  scope: "1"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:5f5748540da32f7ac07fe2d05e6c46cb"
  date: 2026-10-05
---

## Description

This rule identifies a configured boiler fleet keeping more firing units on
than its commissioned staging map permits at low useful load. The point is not
that two boilers are universally wrong: unequal machines, modular burners,
minimum-flow requirements, redundancy, emissions, and minimum run time can make
two units correct. Those facts define the adoption contract and are why both
shipped decision thresholds deliberately block portable deployment.

## Detection Logic

```text
negative_load = hw_plant_load_fraction < 0
above_one     = hw_plant_load_fraction > 1
yLoadOk       = NOT negative_load AND NOT above_one

yLowLoad      = hw_plant_load_fraction < low_load_fraction
yExcessStages = boiler_stage_count > max_stages_at_low_load

yFault = TrueDelay(yLoadOk AND yLowLoad AND yExcessStages,
                   sustained_duration)
```

Block graph (`rule.cxf.jsonld`):

![HW-0012 block graph](diagram.svg)

The load-validity interval includes exactly 0 and 1. Exact load 0.35 and exact
stage count 1 are clear because both decision comparisons are strict. The graph
uses `CDL.Integers.GreaterThreshold` directly. `delayOnInit=true` requires the
full interval on evaluator startup and any false conjunct resets the timer.

Read `yLoadOk` first. A negative load makes raw `yLowLoad=true`, but it gates
`yFault` off and means NO_EVAL; non-finite/freshness checks remain host-side.

## Possible Diagnoses

1. Stage-down threshold, timer, or minimum-run logic set too conservatively.
2. Lead/lag sequence leaving a second boiler latched after load falls.
3. Enabled/available units mistakenly counted as proven firing units.
4. Load numerator, commissioned capacity, or fleet membership derived wrong.
5. Boiler sizes/turndown make the adopted scalar stage rule invalid.
6. Redundancy, exercise, freeze, emissions, or minimum-flow mode not excluded.

## Energy Impact

At the same useful load, excess firing machines can add jacket and standby loss,
operate each burner below its efficient modulation region, and add purge or
light-off cycles. Magnitude depends on equipment and sequence. Without measured
fuel and a commissioned alternative staging model, the result remains
qualitative and no generic savings percentage is claimed.

## Emissions Impact

Any scope-1 impact follows the site-specific fuel penalty of the actual staging
sequence. This rule has neither a fuel measurement nor a counterfactual staging
model, so it does not assign an emissions quantity.

## Deviations

- The brief classed 0.35 and one stage as adopted tunables. They are reclassified
  `NO_PORTABLE_DEFAULT`: source material does not establish them, and boiler
  sizing, turndown, topology, and policy change the correct values materially.
- The load denominator is one stable commissioned eligible-fleet capacity, not
  ambiguously "current" capacity. Availability changes require a new explicit
  configuration and evaluation restart.
- Range validity is implemented in-graph as `yLoadOk`; finite values, source
  quality, and derivation provenance cannot be proven by inverted comparisons.
- The LBNL plant has useful healthy staging channels but no injected over-stage
  fault. No local dataset was available and EnergyPlus's target loop has only
  one real boiler, so this card records no simulation validation claim.
- No Boiler Control Instability cluster is created. Short cycling, hunting, and
  over-staging can co-occur, but no one trigger reliably occurs first or shares
  one repair that clears all members.

## Notes

Investigate HW-0001 for resulting starts, HW-0011 for unstable modulation and
temperature, and HW-0002 for measured efficiency degradation. Never reduce
stages until manufacturer turndown, minimum flow, venting, emissions, safety,
redundancy, and minimum-time requirements have been checked.
