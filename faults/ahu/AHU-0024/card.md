---
schema: cxf-library/fault-card/v1
id: AHU-0024
name: Duct static pressure reset not functioning
equipment: ahu
status: verified
phase: 2
method: statistical
severity: 3
category: EXCESS_CONSUMPTION
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0024"
  - "PNNL RetuningOpps A01"
  - "PNNL-25985 EEM-12"
g36: null
clusters: [CLU-02]
suppresses: []
suppressed_by: []
related: [AHU-0001, AHU-0023, AHU-0031, FPB-0002]
playbooks: [missing-reset]
operating_states: "Occupied, fan running"
preconditions: "AHU serves multiple zones; zone damper feedback available and aggregated by the host into zone_dmpr_pos_max. When zone damper data is missing or stale, the verdict is NO_EVAL, not healthy."
points:
  - dsp_sp
  - zone_dmpr_pos_max
outputs:
  - name: yFault
    description: True while the DSP setpoint has stayed flat over the evaluation window despite all zone dampers staying well below fully open, for at least alarm_delay
params:
  evaluation_window:
    default: 259200.0
    unit: s
    description: Window over which setpoint flatness and low damper demand are assessed (3 days)
    cxf: [spRef.samplePeriod, spFlatHeld.delayTime, dmprLowHeld.delayTime]
  sp_flat_tolerance:
    default: 25.0
    unit: Pa
    description: Max deviation of DSP_SP from its sampled baseline to count as flat (half the reference's 50 Pa min expected range)
    cxf: spFlat.t
  high_damper_threshold:
    default: 70.0
    unit: "%"
    description: Max zone damper position below which the setpoint could safely drop
    cxf: dmprLow.t
  alarm_delay:
    default: 86400.0
    unit: s
    description: Fault persistence before alarm (24 h)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU fan energy (cubic relationship)
  savings_range: 1-3% site energy; fan power ∝ pressure³ (PNNL-25985 EEM-12)
  climate_sensitivity: neutral
  runtime_estimation: "fan_waste_kw = ahu_fan_design_kw × [1 − (1 − SP_reduction/100)³] — a 20% DSP reduction yields ~49% fan energy savings"
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:0bc49a814c1f728a54e9a9a3ecff1244"
  date: 2026-10-05
---

## Description

The duct static pressure setpoint remains fixed despite varying zone demand.
A functioning DSP reset (G36 §5.16.1 trim-and-respond) lowers the setpoint
when zone dampers are not widely open; because fan power scales with the cube
of pressure, even small reductions produce outsized savings. The other half of
the "74% problem" with AHU-0023 — absent in 74% of buildings (PNNL
151-building study) — and a member of cluster CLU-02 with the same $0
desk-only fix.

## Detection Logic

```
baseline(dsp_sp) = dsp_sp sampled and held every evaluation_window (3 days)
sp_flat          = |dsp_sp − baseline(dsp_sp)| < sp_flat_tolerance,
                   continuously for evaluation_window
low_demand       = zone_dmpr_pos_max < high_damper_threshold,
                   continuously for evaluation_window

yFault = sp_flat AND low_demand, sustained for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0024 block graph](diagram.svg)

The setpoint chain mirrors AHU-0023's sampled-baseline flatness detector.
The demand condition needs no baseline at all: the reference's
`max(zone_dmpr_pos_all) < 70%` over the window is *exactly* equivalent to
"the highest zone damper stays below 70% continuously," which is one
`LessThreshold` plus a dwell `TrueDelay` on the host-aggregated maximum.
Worst-case time to alarm from cold start: `evaluation_window + alarm_delay`
(4 days).

## Possible Diagnoses

1. DSP reset never programmed in the BAS
2. DSP reset disabled by an operator
3. Trim-and-respond parameters misconfigured
4. Zone damper feedback not connected to the AHU controller

## Energy Impact

EXCESS_CONSUMPTION, HIGH confidence, PROXY_ESTIMATION (EEM-12). Fan energy
scales with the cube of duct pressure: a 20% setpoint reduction yields ~49%
fan energy savings. Savings 1–3% of site energy; climate-neutral. Prevalence:
74% of buildings.

## Emissions Impact

Scope 2, PROXY_EMISSIONS, HIGH confidence; typical 400–3,000 kg CO₂e/yr
(excess fan energy, cubic law). Avoided-emissions basis: MOER.

## Deviations

- **`zone_dmpr_pos_all` (array) → host-derived `zone_dmpr_pos_max`
  (scalar).** The reference consumes every zone damper position; library v1
  avoids array boundary points, so the host aggregates the maximum across the
  zones served and feeds one scalar (flagged `derived` in the point
  dictionary). The zone-side underlying points get their semantic tags when
  the VAV dictionary lands.
- **Windowed range → sampled-baseline flatness** on the setpoint chain, as
  AHU-0023 (`sp_flat_tolerance = min_expected_sp_range/2`; same
  `MovingAverage` ring-capacity rationale). The damper condition is an exact
  transformation, not an approximation: `max over window < t` ⇔ `continuously
  below t`.
- `delayOnInit = true` on all `TrueDelay`s (startup conservatism per
  AHU-0016).

## Notes

Evaluate together with AHU-0023: both fire → CLU-02 confirmed, one
trim-and-respond programming visit fixes both. The reference's PNNL-27338
AIRCx corollary — DSP > 0.2 in. w.g. during unoccupied hours — is covered
separately by the after-hours rules (AHU-0018 family).
