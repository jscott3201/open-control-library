---
schema: cxf-library/fault-card/v1
id: ERV-0002
name: Frost protection not engaging
equipment: erv
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §15, ERV-0002"
  - "Engineering best practice"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [ERV-0001]
playbooks: [erv-effectiveness]
operating_states: "ERV enabled, heating season — the rule cannot fire above frost_threshold"
preconditions: "oat must be a live, sane outdoor-air reading. This rule trusts it completely, which makes diagnosis 2 (a sensor reading warmer than actual) invisible from inside the rule: a sun-struck or failed-high sensor produces silence, not an alarm. erv_frost_prot must be bound to the frost sequence's *active* state — the preheat stage, wheel speed reduction, or bypass modulation the unit actually uses — and not to an enable or permissive flag, which reads true all winter and silences the rule permanently. Both supply and exhaust fans must be running: the enable half of the operating state is in the graph as erv_enabled, the fan half is not, and a unit enabled with stopped fans is not moving the air that would frost the core. Units with no frost-protection sequence at all must be excluded host-side — they hold this fault true for the entire heating season, which is a design finding rather than an operating one."
points:
  - oat
  - erv_frost_prot
  - erv_enabled
outputs:
  - name: yFault
    description: True while the ERV is enabled and outdoor air is below frost_threshold with the frost sequence reporting inactive, continuously for at least alarm_delay
params:
  frost_threshold:
    default: -10.0
    unit: "°C"
    description: Outdoor-air temperature below which the frost-protection sequence is expected to be engaged. Signed by construction — this is a point on the Celsius scale, not a magnitude
    cxf: coldOat.t
  alarm_delay:
    default: 300.0
    unit: s
    description: Continuous unprotected operation required before the alarm asserts (5 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: ERV equipment protection
  savings_range: Avoided equipment damage (frozen coils, cracked plates)
  climate_sensitivity: heating-dominant
  runtime_estimation: "Qualitative — the reference gives no per-fault model and points to Energy Impact Reference §4.4. The loss is avoided damage, not metered energy"
emissions:
  scope: "2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:c975d4eb5acff4ebf97b661be18c5552"
  date: 2026-10-05
---

## Description

Below roughly -10 °C the moisture in the exhaust airstream starts freezing onto
the recovery core as it gives up its heat, so every ERV that runs in a cold
climate carries a frost-protection sequence — preheat the incoming air, slow the
wheel, or bypass part of the outdoor air around the core. This rule watches for
that sequence failing to appear when the weather calls for it. It is a watchdog
on a control sequence, not on a physical measurement: nothing here observes ice,
only that the conditions for ice are present and the thing meant to prevent it
reports itself inactive. The failure is silent — frost accumulates over hours
and the first symptom is usually a collapsed plate core or a seized wheel found
in spring — which is what severity 2 and PROTECTIVE reflect: the cost is a core.

## Detection Logic

```
cold    = oat < frost_threshold                (-10.0 °C)
noFrost = NOT erv_frost_prot

yFault  = (cold AND noFrost AND erv_enabled) sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![ERV-0002 block graph](diagram.svg)

`coldOat` carries a negative parameter, which the library normally avoids — see
Deviations for why a sub-zero temperature threshold is the exception. `noFrost`
inverts the frost status so the conjunction reads as a single sentence: cold
outside, protection off, unit running. `unprotected` combines the first two and
`armed` adds the enable, which is the reference's third conjunct rather than a
host-side gate — so with the ERV disabled `yFault` reads false, and that false
means *not applicable*, not *protected*.

The comparison is strict and so is the reference's, so nothing is lost at the
boundary: outdoor air at precisely -10.0 °C is not a fault and -10.1 °C is.
`persist` requires 5 continuous minutes, and the delay does real work despite
the short window — frost sequences stage in and out around their own setpoint,
and an outdoor reading crossing -10 °C on a windy afternoon can toggle the cold
term several times before the sequence latches. Any engagement of the sequence,
however brief, drops the timer and discards the accumulated time.

## Possible Diagnoses

1. Frost protection control sequence disabled — switched off during
   troubleshooting, or never enabled at commissioning
2. OAT sensor error, reading warmer than actual — the sequence is working and
   has simply not been told it is cold; the rule cannot distinguish that from a
   dead sequence, since its own cold term reads the same sensor
3. Frost protection damper or valve actuator failure — the sequence commands,
   nothing moves, and the status point may or may not admit it
4. Preheat coil not functioning: no hot water, a closed isolation valve, or a
   failed electric element behind a status point that still reports "on"

## Energy Impact

PROTECTIVE, MEDIUM confidence, QUALITATIVE_ONLY. There is no energy model here
and the reference does not attempt one — it points to the Energy Impact
Reference §4.4 and stops. The value is avoided equipment damage: frozen plate
cores crack, iced wheels stall their drive motors, and both end in replacement
rather than repair (the `erv-effectiveness` playbook prices a core at
$2,000-$5,000). Heating-dominant by construction — the rule is unreachable above
-10 °C. The second-order energy cost is unmodelled but ordered: a partially iced
core is a degraded core, so a unit that runs unprotected through a cold snap
shows up in ERV-0001's effectiveness test afterwards, as a consequence rather
than a coincidence.

## Emissions Impact

Scope 2, DIRECT_EMISSIONS, HIGH confidence (the reference rates emissions
confidence above energy confidence here, and the frontmatter's single
`confidence` field carries the MEDIUM from the energy profile). The reference's
typical range is "protective; pump energy minor; equipment damage primary" with
no avoided-emissions basis — the consequence is the embodied carbon of a
replacement core plus the conditioning energy spent while the recovery device is
out of service, neither of which this rule can meter.

## Deviations

- **The threshold parameter is negative, deliberately.** The library normally
  expresses negatives as a `Sources.Constant` plus a `Subtract` so every literal
  reads as a magnitude, but a sub-zero temperature threshold is the documented
  exception: -10 °C is a point on a scale with a fixed zero, not a negated
  quantity, and `0 − 10` would invent arithmetic the physics does not have.
  `coldOat.t = -10.0` is what an operator types into the BAS, and a host retuning
  for a milder climate sets -7.0, not 7.0.
- **`erv_enabled` is a conjunct in the graph, not a host-side operating-state
  gate.** The library keeps operating-state gating in frontmatter, but the
  reference writes the enable into the equation itself, so the graph implements
  what the reference states. ERV-0001 makes the same choice with the same
  point, so the pair behaves consistently on a unit that is off.
- **No evaluability output.** SCHEMA.md asks for one when the reference's
  semantics include an in-rule evaluability condition; the only gating term here
  is `erv_enabled`, which the host binds as a boundary input and can read
  directly. Contrast ERV-0001's `yTempDeltaOk`, which is computed from three
  temperatures and cannot be seen from outside the rule.
- **The frost test reads `oat`, the site sensor, not `erv_oa_entering_temp`,**
  following the reference's required-points list and the point dictionary, whose
  `oat` entry names this frost test as a consumer. On most units the two are the
  same sensor; where they are not, a well-sited inlet sensor is arguably the
  better measure of what the core sees, but a rooftop `oat` in direct sun reads
  warm, which is diagnosis 2 and silences the rule.
- **Fan status stays a host precondition.** The reference's ERV operating state
  is "ERV enabled, both supply and exhaust fans running". The enable is in the
  graph because the equation names it; fan status is not in this rule's required
  point list and is not invented here.
- **A unit with no frost sequence looks identical to a broken one.** The rule
  reads a status point and cannot ask whether the sequence exists, so deploying
  it on a unit that never had frost protection produces a season-long standing
  alarm. That is a real finding, but it belongs in a design review, so the
  exclusion is host-side.
- `AlarmDelay = 5 min` becomes `persist.delayTime = 300 s` with
  `delayOnInit = true` (CDL default is `false`), the library's standing choice: a
  unit already running unprotected at controller start waits out the full five
  minutes rather than alarming on the first tick.
- Frontmatter `clusters` is empty: the reference defines no cluster containing an
  ERV rule, and this card does not edit the cluster set. The relationship to
  ERV-0001 is carried by `related` and the shared playbook.
- Frontmatter `g36` is null. This is a research-backed 050-range rule sourced to
  engineering best practice; G36 has no ERV frost sequence to cite.
- The reference publishes no test vectors for this card; every scenario in
  `vectors.json` is library-authored.

## Notes

The [erv-effectiveness](../../../playbooks/erv-effectiveness.md) playbook now
starts with a dedicated mode/frost-state check: verify the OAT against a second
local sensor, confirm the point means active protection rather than a permissive,
and exercise one safe transition through the installed sequence. Continue by
checking the damper, valve, wheel, or preheat source that implements protection.
A unit that ran unprotected through a cold snap should have its core inspected
for damage before it is trusted again.
