---
schema: cxf-library/fault-card/v1
id: RTU-0008
name: Refrigerant undercharge — superheat/subcooling divergence
equipment: rtu
status: verified
phase: 2
method: rule
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Library-authored — HVAC FDD Reference v1.0 §11 specifies no refrigerant-charge rule for packaged units; its playbook reaches charge only as a cause of short-cycling (Step 2.1.2)"
  - "NIST SP 1087, Kim, Yoon, Payne & Domanski, *Cooling Mode Fault Detection and Diagnosis Method for a Residential Heat Pump*, NIST, October 2008: §5.4.3 Table 5.2 (undercharge fault-direction rows, zones A and B), §5.4.2 and §5.4.4 (the 0.5 °C upstream-subcooling test and its role in selecting a chart), §5.5.1 Figs. 5.16-5.17 (fault level versus EER degradation), §5.5.2 Table 5.16 (undercharge diagnosis results) — a residential heat-pump study restated here for unitary packaged equipment (see Deviations)"
  - "Breuker & Braun 1998b and Rossi & Braun 1997, reproduced as SP 1087 Table 5.1(a) — the fixed-orifice refrigerant-leakage row, which is the half of the packaged population SP 1087's own TXV rig does not represent"
  - "Li, H. & Braun, J.E. (2009), *Decoupling features and virtual sensors for diagnosis of faults in vapor compression air conditioners*, HVAC&R Research 15(1) — the virtual-refrigerant-charge sensor built from four surface-mounted temperatures and validated across seven unitary systems; the lineage for reading charge off exactly the temperatures this rule reads"
  - "Kim, W. & Braun, J.E. (2020), Energy and Buildings 225 — integrated virtual sensors demonstrated on rooftop units, the packaged-equipment continuation of that work"
  - "Hu, Y. et al. (2021), Energy and Buildings 248 — single-feature charge inference degrades when other faults are present; the simultaneous-fault caveat, carried in Deviations as on HP-0004"
  - "Sibling precedent: HP-0004 (the heat-pump original this card mirrors, including the sub-condition-flag resolution), RTU-0002 (fixed bands named as a simplification of a regressed baseline), RTU-0007 (settled-compressor and stage-change gating as host preconditions)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HP-0004, RTU-0009, RTU-0007, RTU-0001, RTU-0010, RTU-0011]
playbooks: [rtu-compressor-refrigerant]
operating_states: "mechanical cooling, compressor running and settled — one instance per refrigerant circuit. Cooling-only packaged equipment, so there is no mode split and no defrost state to exclude; a unit in economizer free cooling has its compressors off and is covered by the compressor gate."
preconditions: "The compressor must be running and must have held its current stage for min_runtime_for_eval (15 min). With the compressor off all four temperatures equalise and both differences collapse to zero; after a start or a stage change superheat overshoots for minutes while the metering device catches up, and on a multi-stage unit the whole refrigerant-side operating point moves (RTU-0007's stage-change precondition, same reason). Short-cycling units may never present a settled window at all — that silence is RTU-0001's finding, not a healthy charge. evap_sat_temp and cond_sat_temp are host-derived P-T lookups and each lookup MUST be configured for the refrigerant actually in the machine: a wrong refrigerant biases both differences at once and in opposite directions, which is this rule's exact fault pattern. The suction and liquid probes must have good pipe contact and be insulated from ambient air — on a rooftop an unshaded, uninsulated liquid-line probe reads solar gain and fabricates collapsed subcooling. Head-pressure control must be at its normal control point; a unit deliberately flooding the condenser on a cool day moves subcooling by design. Read yTxvSaturated as diagnostic context, NOT as an evaluability gate: this rule has no in-graph NO_EVAL test and false never means healthy (see Deviations)."
points:
  - suction_temp
  - evap_sat_temp
  - cond_sat_temp
  - liquid_temp
outputs:
  - name: yFault
    description: True while suction superheat has stayed above superheat_high_band and liquid subcooling below subcooling_low_band, both continuously for at least alarm_delay
  - name: yTxvSaturated
    description: Sub-condition flag (NOT an evaluability flag; false never means NO_EVAL) — true when liquid subcooling has fallen below subcooling_two_phase_floor, meaning the liquid line is no longer measurably subcooled and the metering device is being fed two-phase refrigerant
params:
  superheat_high_band:
    default: 15.0
    unit: "°C"
    description: Suction superheat above which the evaporator is judged starved. COMMISSIONING-SET PLACEHOLDER — the shipped 15.0 sits just above the no-fault compressor-suction superheats NIST SP 1087 reports for its test unit (10.2-13.7 °C). Set it from this unit's own charging chart plus a tolerance, and on a fixed-orifice machine set it at the chart's high-superheat corner — low indoor wet-bulb against a high outdoor drybulb (see Deviations); a suction-line probe reads higher than an evaporator-exit probe on the same machine.
    cxf: shHigh.t
  subcooling_low_band:
    default: 3.0
    unit: "°C"
    description: Liquid subcooling below which the condenser is judged short of liquid. COMMISSIONING-SET PLACEHOLDER on the same terms — charging-chart targets run roughly 8-11 °C at design on a TXV unit and lower on a fixed-orifice one, and this band sits well under either so normal load swings do not reach it.
    cxf: scLow.t
  subcooling_two_phase_floor:
    default: 0.5
    unit: "°C"
    description: "Subcooling below which the metering-device inlet is taken to be two-phase. Drives yTxvSaturated only. 0.5 °C is SP 1087's own single-phase/two-phase test (§5.4.2), used there to pick which fault-direction chart applies; unlike the bands above it is a physical boundary, not a per-unit tuning."
    cxf: txvSat.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous divergence required before the alarm asserts (30 min). Long enough to outlast a metering device hunting after a load step, short enough that a real charge loss is reported within the hour
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: RTU compressor energy — capacity lost to reduced refrigerant mass flow
  savings_range: 5-13% of compressor energy at the charge losses this rule detects (NIST SP 1087 Figs. 5.16-5.17)
  climate_sensitivity: cooling-dominant
  runtime_estimation: "waste_kw = compressor_kw × d / (1 − d), where d is the EER-degradation fraction — the extra runtime needed to deliver the same cooling. SP 1087 puts d at roughly 0.065-0.13 for a 20% charge shortfall, so d = 0.10 is the default sizing until a site figure exists; compressor_kw is not one of this rule's points and the host supplies it"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:6185ddcde7a0e68b845e3af5bcea5b05"
  date: 2026-10-05
---

## Description

A machine short of refrigerant runs short of liquid. The metering device runs
out of authority trying to keep the evaporator fed, so the evaporator starves
while the condenser loses its liquid seal: superheat climbs and subcooling
collapses at once. That divergence is the signature — capacity loss alone says
nothing about cause, and a rooftop unit loses capacity quietly for a season
before anyone calls it in. NIST SP 1087 imposed graded charge faults on a
TXV-equipped R410A machine in cooling and recorded this pair once the valve
saturated (§5.4.3, Table 5.2, zone B); Breuker & Braun's fixed-orifice charts
show it from the first pound lost. This rule forms both differences from four
refrigerant-side temperatures and alarms when they sit past their commissioned
bands for half an hour.

## Detection Logic

```
suction_superheat = suction_temp  − evap_sat_temp
liquid_subcooling = cond_sat_temp − liquid_temp

yTxvSaturated = liquid_subcooling < subcooling_two_phase_floor
                (sub-condition flag; false does NOT mean NO_EVAL)

yFault        = suction_superheat > superheat_high_band
                AND liquid_subcooling < subcooling_low_band,
                sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![RTU-0008 block graph](diagram.svg)

The conjunction is the diagnosis, not a noise filter. High superheat with
subcooling *high* is the liquid-line or filter-drier restriction pattern, where
refrigerant backs up ahead of the restriction; with subcooling merely *normal*
it is a fixed-orifice unit at low indoor load, which is no fault at all. Low
subcooling with superheat still normal is a valve compensating successfully.
Only the two together indict the charge.

Both comparisons are strict, so a unit exactly on either band reads healthy, and
both bands are per-unit commissioning values — the shipped defaults are
placeholders, not thresholds anyone measured on the machine in front of you.
`persist` requires 30 continuous minutes and clocks them from the conjunction,
not from the first symptom; `delayOnInit = true` holds that window across a
controller restart. `yTxvSaturated` reports whether the liquid line is still
measurably subcooled; it does not gate the alarm, and the reason it must not is
the card's main deviation.

## Possible Diagnoses

1. Refrigerant leak — brazed joints, Schrader cores, service-valve packing and
   flare connections, in that order of prevalence; a top-up without a leak
   search buys months, not years
2. The unit was charged short, at commissioning or after a rooftop repair that
   vented the circuit and was recharged by pressure rather than by weight
3. Condenser airflow restriction (RTU-0007) — a fouled coil moves this pair the
   same way and separates only on condensing temperature, which rises rather
   than falls. This rule reads differences, not levels, so RTU-0007's split is
   the discriminator and neither card suppresses the other
4. Instrumentation: a P-T derivation configured for the wrong refrigerant, or a
   liquid-line probe with poor contact, missing insulation or sun on it. Each
   fabricates the pattern on a correctly charged machine

## Energy Impact

EFFICIENCY_LOSS, MEDIUM confidence, PROXY_ESTIMATION.
`waste_kw = compressor_kw × d / (1 − d)`, the extra compressor runtime needed
to deliver the same cooling at a degraded EER. NIST SP 1087 sizes `d` directly:
every fault it tested except compressor leakage needed a fault level above 10%
to cost 5% of EER, and a 20% charge shortfall cost 6.5-13% of EER, the largest
hit in its Figure 5.17. Confidence is MEDIUM because the rule fires on a
pattern, not a severity — it reports that the charge is low, not by how much, so
`d` is a population number until the technician's gauge set supplies a real one.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, MEDIUM confidence; typically 300-2,000 kg CO₂e/yr for
a commercial packaged unit, the same order as RTU-0002 and RTU-0007 and all of
it compressor electricity, so the avoided-emissions basis is the marginal
operating emissions rate (MOER) and the waste peaks on the hot afternoons when
the grid is dirtiest. A leaking circuit also vents refrigerant, and R410A
carries a GWP near 2,000; that release is a scope 1 emission this card does not
estimate, because the leak rate is not observable from any point the rule reads.
Sites with refrigerant-tracking obligations should account for it separately.

## Deviations

- **The 0.5 °C subcooling floor is a sub-condition flag, not an evaluability
  gate.** SP 1087 uses it (§5.4.4) to select *which* fault-direction chart
  applies, never to suppress evaluation, and that chart still lists falling
  subcooling as an undercharge symptom. Gating `yFault` on it would silence the
  rule exactly when the liquid line has flashed to two-phase — severe
  undercharge, not missing data — so the flag is named `yTxvSaturated`, not
  `y…Ok`.
- **The flag keeps HP-0004's name on a family where half the population has no
  TXV.** Nothing saturates on a fixed-orifice unit, but the measurement is
  identical — the liquid line is no longer measurably subcooled — and `outputs`
  says so. A per-family rename would cost hosts binding both cards a shared
  signal name for one word of accuracy.
- **The rule does not know which expansion device it is watching, and does not
  need to.** SP 1087 splits its charts on that device — Table 5.2 for TXV,
  Table 5.1(a) for fixed orifice — but both list superheat up with subcooling
  down for undercharge, so the pattern is common ground and no point or
  parameter records the device. The split moves into commissioning instead, and
  it lands on the superheat band: on a fixed-orifice unit superheat follows
  indoor wet-bulb and outdoor drybulb rather than being controlled (charging
  charts for that population are two-dimensional for that reason), so the band
  belongs at the chart's high-superheat corner — low indoor wet-bulb against a
  high outdoor drybulb — or a dry-climate unit alarms on its hottest afternoons
  (`fixed_orifice_unit_at_low_indoor_load` pins the case).
- **Fixed bands replace SP 1087's regressed no-fault baseline.** SP 1087
  compares each feature against a third-order regression on three variables; this
  library's only regression primitive is a host-fitted line, so charging-chart
  nominal targets stand in — a real simplification, named as one on RTU-0002's
  precedent, and the reason untouched defaults can alarm forever.
- **Sensitivity runs opposite between the two populations.** On a TXV unit
  undercharge moves no superheat until the valve saturates (SP 1087's zone A),
  so a mild loss the valve absorbs reads healthy here; earlier investigators
  reported difficulty detecting undercharge below roughly 40% charge loss
  (Breuker & Braun 1998b; Stylianou & Nikanpour 1996, via SP 1087 §2). A
  fixed-orifice unit is caught earlier and pays for it in false-alarm exposure
  at low load.
- **Two features, so condenser airflow restriction is not excluded.** It shares
  the superheat-up/subcooling-down pair and separates on condensing temperature
  moving up rather than down — a level test needing a baseline, which RTU-0007
  has and this rule does not. Diagnosis 3 names it and
  `condenser_restriction_reads_as_undercharge` pins that this rule fires on it;
  no suppression either way, because neither rule adjudicates the other's
  evidence.
- **The grounding transfers from a residential heat pump to packaged
  equipment.** A starved evaporator and an unsealed condenser are circuit-level
  physics, and Li & Braun (2009) validated charge inference from these same four
  surface temperatures across seven unitary systems, Kim & Braun (2020) carrying
  it onto rooftop units. What transfers is the pattern, not the numbers: no
  SP 1087 threshold is adopted except the physical two-phase floor.
- **Cooling-only, which is where this card stands on firmer ground than
  HP-0004.** SP 1087 tested cooling exclusively, so the heat-pump reading has to
  carry a caveat about the coils swapping roles in heating. A cooling-only
  packaged unit has no reversing valve and no defrost cycle: the sensors keep
  their heat exchangers year-round, `operating_states` needs no mode split and
  `preconditions` no defrost gate. Bind HP-0004 for a heat-pump rooftop.
- **Compressor and steady-state gating stay host preconditions.** The graph
  computes the fault given valid data, per SCHEMA.md; `comp_status` and
  time-since-stage-change are not among its inputs, and the 30-minute
  `alarm_delay` does not substitute — a post-start superheat overshoot starts
  the persistence timer rather than being excluded from it.
- **The pattern chart is a single-fault chart.** SP 1087 imposed one fault at a
  time, and Hu et al. (2021) show single-feature charge inference degrading when
  other faults are present, their residuals superposing. Two faults at once can
  cancel this rule's pattern or fake it; the diagnosis list is a ranking, not a
  verdict.
- **Strict `>` and `<` at both bands.** CDL `Reals` has no `GreaterEqual` or
  `LessEqual`, so a unit sitting exactly on a band reads healthy. The
  disagreement is measure-zero on real-valued signals; both sides of both bands,
  and of the two-phase floor, are pinned bit-exactly by vectors.
- **The playbook covers the work but does not yet list this rule.** Its
  Step 2.1.2 already sends the technician to superheat and subcooling against
  manufacturer specs, the measurement this rule automates; the Applies-To row
  and a charge-specific Step 1 entry are the index owner's to add.
- `persist.delayOnInit = true` (CDL default is `false`), the library's standing
  choice. Severity 3 and `category: EFFICIENCY_LOSS` follow HP-0004 and
  RTU-0002; the namespace `urn:cxf-library:rtu-0008#` is SCHEMA.md's normative
  form, as on RTU-0007 and VAV-0010. No reference card exists to inherit any of
  this from and none publishes test vectors, so every scenario in `vectors.json`
  is authored from the equation and replayed against the pinned engine rev.

## Notes

Do not read a cleared alarm as a repaired machine: the host gates this rule on
the compressor running, so every stop drops `yFault` for the same reason a
recharge does, and a short-cycling unit may never complete a settled window at
all. When `yTxvSaturated` is true, expect flash gas at the sight glass and weigh
the recovered charge rather than trusting subcooling to confirm the fix. Check
RTU-0007 before opening the gauges — a fouled condenser makes this same pair and
costs a coil wash rather than a leak search. RTU-0009 is the overcharge branch,
HP-0004 the heat-pump original, and
[rtu-compressor-refrigerant](../../../playbooks/rtu-compressor-refrigerant.md)
orders the on-site work.
