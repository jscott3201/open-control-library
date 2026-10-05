---
schema: cxf-library/fault-card/v1
id: ERV-0003
name: Frost protection active above release conditions
equipment: erv
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "Library-authored complement to HVAC FDD Reference v1.0 §15 ERV-0002: that rule detects frost protection missing in cold weather; this rule detects the same reported sequence state persisting after warm-weather release should have occurred"
  - "Library precedent: ERV-0002 (oat / erv_frost_prot / erv_enabled point contract and strict frost-state watchdog), ERV-0001 (recovery lost intentionally during frost protection), and the repository's 900 s transient-rejection convention"
  - "points/erv.points.json oat, erv_frost_prot, and erv_enabled — the binding distinction between an active sequence state and a permissive/advisory flag"
  - "PNNL-19004 p.55 and DOE/NREL Ventilation Integrated Comfort System report pp.28-29 — public examples whose frost controls use exhaust-leaving/core-entering conditions rather than one portable OAT release threshold, supporting site configuration rather than transcription"
  - "Greenheck ERV controller IOM 484118, p.4 — manufacturer example combining a device-specific OAT permissive with wheel differential pressure; evidence that technology/controller logic varies, not support for the shipped +5 °C"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [ERV-0001, ERV-0002]
playbooks: [erv-effectiveness]
operating_states: "ERV enabled and expected to recover energy, with both air streams moving and the installed frost sequence capable of releasing under warm conditions"
preconditions: "erv_frost_prot must be the actual active state of the installed frost sequence, not a frost-enable permissive, alarm, or low-temperature advisory. release_oat must be commissioned from that unit's own release logic and must remain above ERV-0002's engagement threshold; the shipped +5 °C is only an executable starting point. OAT must be valid and representative of the ERV intake. The host must verify both air streams are moving and exclude manual frost tests, commissioning, smoke/purge modes, maintenance overrides, and technology-specific recovery modes that legitimately retain frost protection above the configured line. A unit with no frost sequence or no observable active state is not deployable for this rule."
points:
  - oat
  - erv_frost_prot
  - erv_enabled
outputs:
  - name: yFault
    description: True while the enabled ERV has reported frost protection active above release_oat continuously for sustained_duration
params:
  release_oat:
    default: 5.0
    unit: "°C"
    description: "ADOPTED_TUNABLE: outdoor temperature above which continued frost protection is suspicious. Configure from the installed sequence's actual release point; +5 °C is not a universal frost boundary and deliberately leaves a neutral band above ERV-0002's -10 °C engagement default."
    cxf: aboveRelease.t
  sustained_duration:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE: continuous warm-weather frost state required before alarm (15 min). Long enough to reject ordinary release and sensor-filter lag; raise it where the manufacturer's sequence completes a longer defrost or recovery transition."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: "Energy recovery, frost preheat, and supply/exhaust fan balance"
  savings_range: "Site-specific. Active frost protection may bypass or slow recovery, energize preheat, or unbalance the two air streams for every hour it remains active."
  climate_sensitivity: heating-dominant
  runtime_estimation: "Qualitative only from this rule's three points. A host may estimate lost recovery with ERV-0001's effectiveness model and add measured preheat/fan power while yFault is active; do not infer kilowatts from the frost-state boolean alone."
emissions:
  scope: "1+2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:2d84409ba6461e88a64910d5f847870c"
  date: 2026-10-05
---

## Description

Frost protection is supposed to trade recovery efficiency for equipment safety
only while icing is credible. A preheat stage, bypass, wheel slowdown, or
airflow-unbalance strategy that stays active in mild weather continues paying
that trade after its benefit has disappeared. This rule watches the sequence's
reported active state against a commissioned outdoor-air release boundary. It
does not judge how the unit protects itself or redesign its frost sequence.

## Detection Logic

```text
above_release = oat > release_oat
candidate     = erv_enabled AND erv_frost_prot AND above_release

yFault = candidate sustained continuously for sustained_duration
```

Block graph (`rule.cxf.jsonld`):

![ERV-0003 block graph](diagram.svg)

Both the temperature comparison and timer are strict in their own ways: OAT at
exactly `release_oat` is clear, and the alarm appears only after the full
continuous duration. `delayOnInit = true` serves that duration after a restart.
Any release of frost mode, OAT return to the boundary, or ERV disable clears the
alarm and discards elapsed time immediately.

## Possible Diagnoses

1. Frost-mode software latch, timer, or state machine failed to release
2. OAT sensor biased high (false alarm), biased low (missed finding), stale, or
   installed where it does not represent the ERV intake
3. Preheat valve/relay, bypass damper, or wheel-speed command left overridden
4. BAS point bound to a frost permissive rather than the sequence's active state
5. `release_oat` configured below the installed sequence's true release point

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, QUALITATIVE_ONLY. The cost depends on the
frost technology: preheat can consume fuel or electricity, bypass/wheel slowdown
hands ventilation load back to downstream coils, and airflow imbalance adds fan
and envelope load. The rule measures duration but no power or recovered heat.

## Emissions Impact

Scope 1 + 2, QUALITATIVE_EMISSIONS. Electric fan/preheat and cooling effects are
scope 2; fuel-fired preheat or downstream heat is scope 1. Quantification needs
the host's power, airflow, and temperature measurements rather than this state
flag alone.

## Deviations

- **Library-authored complement, not a transcribed reference card.** The HVAC
  FDD Reference publishes ERV-0002's missing-protection direction; this card
  mirrors its point contract for the opposite operational failure.
- **`release_oat = 5 °C` is ADOPTED_TUNABLE.** It is intentionally distinct
  from ERV-0002's `-10 °C` engagement default, leaving a 15 K neutral band in
  which neither rule asserts. The installed sequence remains authoritative.
- **Public examples do not establish a generic OAT release line.** PNNL-19004
  controls an exhaust-leaving temperature and DOE's VICS prototype tempers
  core-entering air; Greenheck combines its own permissive with wheel pressure.
- **The roadmap classified 900 s as precedent; this card classifies it as
  ADOPTED_TUNABLE.** The library uses 15-minute rejection windows, but no cited
  source establishes that duration for every frost technology.
- **The optional `release_margin` is omitted.** With no configured release
  input, a zero-default margin duplicates `release_oat` without adding behavior.
- **`erv_enabled` remains in-graph.** ERV-0001/0002 already use this boundary
  point to avoid nightly raw alarms; fan proof, tests, and overrides remain host
  preconditions.
- **No suppression or cluster.** Excessive frost protection can genuinely cause
  low effectiveness, so ERV-0001 remains useful; shared remediation is carried
  by the playbook rather than a new taxonomy entry.
- **No empirical validation claim.** Required synthetic vectors ran; the
  current EnergyPlus harness has no defensible frost-state mapping for this PR.

## Notes

Confirm the point meaning before tuning the threshold. A flag that means
"frost protection available" rather than "frost protection active" will hold
this rule on all year and no temperature adjustment will fix the binding.
