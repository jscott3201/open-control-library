---
schema: cxf-library/fault-card/v1
id: HP-0006
name: Reversing-valve internal bypass leakage
equipment: hp
status: verified
phase: 2
method: rule
severity: 3
category: EFFICIENCY_LOSS
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "NIST SP 1087, Kim, Yoon, Payne & Domanski, Cooling Mode Fault Detection and Diagnosis Method for a Residential Heat Pump (October 2008), §5.1 Table 5.2 — zone-A fault-direction chart"
  - "NIST SP 1087 §5.4.2 (TXV control-limit test), §5.5.1 Figs 5.16-5.17 and §5.5.2 Tables 5.12/5.18 (EER sensitivity, misdiagnosis floor), §3.2.3 Table 3.3 (no-fault steady-state values)"
  - "Li & Braun 2007, HVAC&R Research 13(2) — multiple-simultaneous-fault caveat"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HP-0003, HP-0004, HP-0001, HP-0007]
playbooks: [heat-pump-faults]
operating_states: "cooling, compressor running and settled — the source's direction chart is cooling-mode only (see Deviations)"
preconditions: "The compressor must be running and must have held its current capacity for at least 15 min: the source's own steady-state detector needed 6-15 min after a start before its features settled, and a unit still pulling down shows deviations on physics rather than on fault. Defrost must be excluded, not merely tolerated — a defrost cycle reverses the circuit deliberately and scrambles every refrigerant-side temperature this rule reads, so the host gates on defrost_status. The four nominals are commissioning values, not library constants; until the host has written them from this unit's own no-fault operation at the condition the instance runs in, the rule is comparing against the source rig's numbers and means nothing (see Deviations). Nothing in the rule cross-checks its inputs: a drifted discharge sensor, or a P-T lookup configured for the wrong refrigerant, moves a conjunct on its own. TXV evaluability is signalled in-rule by yTxvOk; when it is false the verdict is NO_EVAL, not healthy."
points:
  - evap_sat_temp
  - cond_sat_temp
  - comp_discharge_temp
  - liquid_temp
outputs:
  - name: yFault
    description: True while all four refrigerant-side deviations — evaporating and discharge temperature above nominal, condensing temperature and subcooling below it — have held together for at least alarm_delay
  - name: yTxvOk
    description: Evaluability signal — true when subcooling is above txv_control_min_subcool, the inlet condition under which the expansion valve is still compensating and the source's direction chart applies; false means NO_EVAL and the host must ignore yFault
params:
  evap_sat_nominal:
    default: 10.0
    unit: "°C"
    description: Expected evaporating saturation temperature for this unit at this operating condition. PER-UNIT COMMISSIONING VALUE; the shipped default is the source rig's no-fault reading (see Deviations)
    cxf: kEvapNom.k
  evap_sat_rise_min:
    default: 2.0
    unit: K
    description: Rise above evap_sat_nominal that counts as the leakage signature. Adopted — the source classifies residuals against a fitted model, not a fixed band
    cxf: evapHigh.t
  disch_nominal:
    default: 66.0
    unit: "°C"
    description: Expected compressor discharge line temperature. PER-UNIT COMMISSIONING VALUE on the same terms as evap_sat_nominal
    cxf: kDschNom.k
  disch_rise_min:
    default: 5.0
    unit: K
    description: Rise above disch_nominal that counts. Wider than the other bands because discharge temperature swings hardest with lift and load
    cxf: dschHigh.t
  cond_sat_nominal:
    default: 40.0
    unit: "°C"
    description: Expected condensing saturation temperature. PER-UNIT COMMISSIONING VALUE
    cxf: kCondNom.k
  cond_sat_fall_min:
    default: 2.0
    unit: K
    description: Fall below cond_sat_nominal that counts. This is the conjunct that separates leakage from condenser airflow restriction, which pushes condensing temperature the other way
    cxf: condLow.t
  subcool_nominal:
    default: 5.0
    unit: K
    description: Expected liquid-line subcooling (cond_sat_temp − liquid_temp). PER-UNIT COMMISSIONING VALUE
    cxf: kScNom.k
  subcool_fall_min:
    default: 1.5
    unit: K
    description: Fall below subcool_nominal that counts. Must stay below subcool_nominal − txv_control_min_subcool or the fault band closes entirely (see Deviations)
    cxf: scLow.t
  txv_control_min_subcool:
    default: 0.5
    unit: K
    description: Subcooling below which the expansion valve inlet is two-phase, the valve has reached its opening limit and the source's zone-A chart no longer describes the unit. Taken directly from NIST SP 1087 §5.4.2
    cxf: txvOk.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous persistence of the full pattern required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Heat pump compressor — discharge gas short-circuited back to suction, doing no useful work
  savings_range: 4-9% EER at 5-10% leakage, rising to 23-34% at 27-38% leakage (NIST SP 1087 Table 5.12)
  climate_sensitivity: both
  runtime_estimation: "waste_kw = elec_power × eer_degradation_fraction — the leaked mass flow is compressed and then discarded across the valve, so the share of the draw that moved it buys nothing"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:04bdf03b92bee643d7e6186c505038a1"
  date: 2026-10-05
---

## Description

A reversing valve fails two ways and HP-0003 only catches the first: a valve
that never shifts, caught by discharge air that contradicts the commanded mode.
This card catches the second — a valve that shifts correctly and then leaks
internally, letting hot discharge gas bypass the slide straight back to the
suction line. The unit still heats when told to heat, so the air-side test sees
nothing and capacity quietly falls instead. NIST SP 1087 imposed graded leakage
on an instrumented R410A residential heat pump and found it the most
EER-sensitive of the six faults studied: 5.0% leakage cost 5.5% EER, while
every other fault needed more than 10% fault level to lose the same 5%.

## Detection Logic

```
subcooling = cond_sat_temp − liquid_temp

evap_rise  = evap_sat_temp       − evap_sat_nominal  > evap_sat_rise_min
disch_rise = comp_discharge_temp − disch_nominal     > disch_rise_min
cond_fall  = cond_sat_nominal    − cond_sat_temp     > cond_sat_fall_min
subc_fall  = subcool_nominal     − subcooling        > subcool_fall_min

yTxvOk = subcooling > txv_control_min_subcool   (false ⇒ host reports NO_EVAL)
yFault = evap_rise AND disch_rise AND cond_fall AND subc_fall AND yTxvOk,
         sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HP-0006 block graph](diagram.svg)

The conjunction is the diagnosis, not a robustness measure. Condenser airflow
restriction also raises evaporating and discharge temperature and drops
subcooling, but pushes condensing temperature *up*; undercharge drops
condensing temperature and subcooling but leaves evaporating and discharge
temperature alone. Drop either conjunct and the rule stops naming this fault.

`yTxvOk` is the source's own control-limit test: above roughly 0.5 K of
subcooling the valve inlet is single-phase liquid and the valve is still
compensating, which is the regime its direction chart describes. Deploy knowing
the consequence — the evaluable subcooling window is
`(txv_control_min_subcool, subcool_nominal − subcool_fall_min)`, so a leak
severe enough to collapse subcooling past the floor reads NO_EVAL, not FAULT.

All five comparisons are strict. `persist` requires 30 continuous minutes, and
`delayOnInit = true` holds that window across a controller restart.

## Possible Diagnoses

1. Reversing valve internal bypass leakage — an eroded or worn slide seal; the
   valve body is replaced rather than repaired, $500–$2,000 plus refrigerant
   recovery
2. Compressor internal leakage — worn discharge valves or scroll flanks produce
   the same refrigerant-side pattern from a different component, and the source
   imposed both under one fault label. The service call separates them; this
   rule cannot
3. Valve parked off-seat — a weak solenoid, a blocked pilot line, or low charge
   leaving too little differential to seat the slide, which then leaks by
   definition
4. Stale nominals — an instance commissioned at one operating condition and
   evaluated at another. Check the commissioning record before condemning
   hardware
5. Refrigerant-side instrumentation — a discharge sensor reading high, or a
   host P-T lookup configured for the wrong refrigerant, each move a conjunct
   with nothing in the rule to contradict them

## Energy Impact

EFFICIENCY_LOSS, MEDIUM confidence, PROXY_ESTIMATION. NIST SP 1087 Table 5.12
measured EER degradation tracking leakage close to one-for-one across four test
conditions: 5.0% leakage → 5.5% EER lost, 9.3% → 8.9%, 27.2% → 23.0%, 38.2% →
33.5%. The estimator is `waste_kw = elec_power × eer_degradation_fraction`,
with the fraction taken from that relationship or from HP-0001's fitted
baseline on the same unit. PROXY because this rule reads four temperatures and
no power at all. MEDIUM because the direction evidence is a controlled
fault-imposition study but the magnitudes come from one lab unit in one mode.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, MEDIUM confidence; typically 200–1,500 kg CO₂e/yr for
a commercial packaged heat pump, all of it compressor electricity, so the
avoided-emissions basis is the marginal operating emissions rate (MOER). Note
what is *not* here: an internally leaking valve loses nothing to atmosphere, so
there is no scope 1 refrigerant component to add — the whole impact is the
extra electricity, and it is worst at high lift, when the unit runs longest and
the grid is dirtiest.

## Deviations

- **Fixed nominal targets replace the source's regressed reference model.** NIST
  SP 1087 classifies each feature's residual against a third-order polynomial
  fitted in three variables (indoor drybulb, outdoor drybulb, indoor dew point).
  The block set's only regression primitive is a host-fitted line (HP-0001),
  so the four nominals become commissioning constants valid near the condition
  they were recorded at — the same named simplification RTU-0002 makes when
  it fixes its per-stage split baselines.
- **The shipped nominals are the source's rig, not your unit.** 10 °C
  evaporating, 66 °C discharge, 40 °C condensing and 5 K subcooling are the
  no-fault steady-state readings of NIST SP 1087 §3.2.3, taken on an R410A
  residential split at 26.7 °C indoor / 27.8 °C outdoor. They make the card
  runnable as delivered and nothing more.
- **The deviation bands are adopted, not transcribed.** The source's neutral-case
  thresholds run 0.3–1.0 K at 99% credibility, derived from lab instrumentation
  *and* a fitted model. Against a fixed nominal the operating-condition swing
  that model absorbed lands in the residual instead, so the shipped bands are
  several times wider. Narrow them only as far as a site's own commissioning
  spread allows.
- **Superheat is not consumed, though the source lists it as a feature.** Its
  chart marks superheat neutral for this fault and for every other TXV-in-control
  pattern, so a neutrality conjunct would discriminate nothing while adding
  `suction_temp` and two thresholds. Measurement point matters here: the
  source's superheat is at the evaporator exit where the TXV holds it, whereas a
  superheat computed from `suction_temp` sits downstream of the bypass and would
  read high — which is a reason for charge rules to distrust suction-line
  superheat while this fault is live, not a reason to test it here.
- **The evaluability test is the source's general control-limit criterion, not
  its rig-specific zone split.** §5.4.2 grounds it physically — subcooling above
  ~0.5 K at the valve inlet means single-phase liquid and a valve still
  compensating — while the 9 °C superheat boundary the report also uses is a
  fitted artifact of that rig's flow-coefficient curve. Only the general form
  belongs in a portable graph.
- **Not a suppression relationship with HP-0003.** That card asks whether the
  valve switched at all, from discharge air; this one asks whether a switched
  valve holds its seal, from refrigerant temperatures. Neither failure implies
  the other and both can be absent at once, so the link is carried by `related`
  and `suppresses`/`suppressed_by` stay empty.
- **Cooling mode only.** The source imposed every fault in cooling and publishes
  no heating-mode direction chart. Symmetry is physically plausible and
  unverified, and this library does not ship unverified direction charts; a
  heating-mode instance needs its own grounding and its own nominals.
- **A leak below roughly 5% of refrigerant flow is out of reach.** The source's
  own classifier misread 2.4–2.5% leaks as no-fault or as undercharge at EER
  losses of 1.3–3.4% (Table 5.18) — with lab instrumentation and a regressed
  baseline. A fixed-nominal rule is strictly weaker, so treat silence as "not
  this fault yet", never as a clean valve.
- **Single-fault reasoning only.** The direction chart is fitted from
  single-fault tests and its classifier assumes independence across features
  (Li & Braun 2007). Two simultaneous faults can cancel a conjunct or fabricate
  one; this card names one hypothesis, and the playbook's charge check is what
  rules out the common companion.
- **Strict comparisons at all five limits.** CDL Reals has no `GreaterEqual`, so
  a feature sitting exactly on its band edge reads healthy and subcooling
  exactly at `txv_control_min_subcool` reads NO_EVAL. The disagreement is
  measure-zero on real-valued signals and both sides of every limit are pinned
  by vectors.
- `persist.delayOnInit = true` (CDL default is `false`), the library's standing
  choice: a unit already showing the full pattern when the controller restarts
  waits out the 30 minutes rather than alarming on the first tick.
- Operating state, the compressor-running gate and the defrost exclusion live in
  frontmatter for host enforcement rather than in the block graph, per the
  library's design stance. Defrost is the sharp case: it reverses the circuit on
  purpose, which is this rule's fault pattern by design.
- The source publishes no deployable test vectors; every scenario in
  `vectors.json` is authored from its Table 5.2 direction chart and replayed
  against the pinned engine rev.

## Notes

Check charge first: undercharge shares two of this rule's four conjuncts, is far
more common, and is cheap to rule out. The
[heat-pump-faults](../../../playbooks/heat-pump-faults.md) playbook orders that
work. Expect HP-0001 on the same unit — a leak large enough for this rule to
see costs 5% EER or more, a third of the way to that card's alarm. Read
`yTxvOk` before `yFault`: a severe leak can drive subcooling under the
control-limit floor and silence the rule when it matters most. `evap_sat_temp`
and `cond_sat_temp` are host-derived P-T conversions, and a lookup configured
for the wrong refrigerant biases three of the four conjuncts at once.
