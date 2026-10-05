---
schema: cxf-library/fault-card/v1
id: SYS-0007
name: Zone heating active during summer / warm weather
equipment: sys
status: verified
phase: 2
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §16, SYS-0007 (pdf pp. 145-146) — equation, all three tunables, the four diagnoses, the whole impact profile, and the truncated notes line"
  - "The reference's own provenance line for that card: PNNL RetuningOpps Z01; ~20% prevalence (PNNL 151-building study)"
  - "Library precedent: VAV-0006 (the zone-level reheat rule this one overlaps and does not duplicate — see Deviations), VAV-0003, AHU-0016"
g36: null
clusters: [CLU-05]
suppresses: []
suppressed_by: []
related: [VAV-0006, VAV-0003, VAV-0001, AHU-0016, SYS-0002]
playbooks: [vav-min-flow-reheat]
operating_states: "all. There is no occupied gate and no mode gate, because the reference specifies none and because the fault is defined by the weather rather than by the schedule: a zone reheating at 26 degC outdoors is waste at 03:00 as surely as at 15:00, and a night-setback heating cycle at that outdoor temperature is itself the finding."
preconditions: "Instantiated PER ZONE. `rht_vlv_cmd` is one VAV box's or one FCU's reheat command, not a building aggregate, and the deployment runs one instance per zone (the reference files this rule under SYS and scopes it to VAV and FCU equipment). `rht_vlv_cmd` must be the modulating command in percent of travel; a two-position coil that reports only on/off should be bound as 0/100 and the threshold left where it is, and a coil whose command is in some other unit needs `reheat_active_threshold` retuned to it. `oat` must be a fresh, shaded site outdoor air temperature: this rule's standing false positive is a sun-baked wall sensor reading 23 degC on a 17 degC afternoon, and the sensor-health rules (SYS-0005/SYS-0009/SYS-0010) are what adjudicate that before the finding is believed. Where a zone has a legitimate reason to heat above the lockout — a freezer anteroom, a pool hall, a humidity-controlled space with reheat downstream of a dehumidification coil — the binding is wrong rather than the rule, and that zone should not be instantiated."
points:
  - rht_vlv_cmd
  - oat
outputs:
  - name: yFault
    description: True while this zone's reheat command has stayed above reheat_active_threshold with outdoor air above zone_heating_lockout_temp, continuously for alarm_delay
params:
  reheat_active_threshold:
    default: 10.0
    unit: "%"
    description: "Reheat command above which the coil counts as actively consuming energy rather than leaking through a shut valve (the reference's own 10%, and the same number VAV-0006 uses for the same test)"
    cxf: rhtOn.t
  zone_heating_lockout_temp:
    default: 21.0
    unit: "°C"
    description: "Outdoor air temperature above which no zone should be calling for heat — the temperature an OAT heating lockout would be programmed at (the reference's own 21 °C / 70 °F). Note this is 3 K above VAV-0006's cooling_season_oat and asks a different question; see Deviations"
    cxf: warmOut.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: "Continuous persistence required before the alarm asserts (30 min). This is the reference's published AlarmDelay and also stands in for the `lockout_check_duration` its equation names but never publishes — one knob, see Deviations"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Zone reheat energy during warm weather (pure waste)
  savings_range: "100% of reheat energy while active; up to 20% of zone annual energy (PNNL RetuningOpps Z01; ~20% prevalence in the PNNL 151-building study)"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "waste_kw = rht_vlv_cmd/100 × vav_rht_capacity_kw, accumulated over the hours yFault holds; the cooling energy spent on the air the coil is undoing is waste on top and belongs to the air handler's own accounting"
emissions:
  scope: "1"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:81170d88e808d51ed3e4980ee1e99faa"
  date: 2026-10-05
---

## Description

A reheat coil is heating a zone while it is 26 degC outside. Nothing about the
building needs that heat, and whatever the coil delivers the plant paid to cool
the air first — simultaneous heating and cooling seen from the zone end, which
PNNL found in about a fifth of the buildings it retuned. The usual cause is the
absence of a control decision rather than the failure of one: most zone
sequences call for reheat any time the space falls under its heating setpoint
and nothing in that logic knows what the weather is doing, so a zone under an
overcooled supply duct in July asks for heat all afternoon and gets it. The fix
is a line of programming — lock the reheat valve out above an outdoor air
temperature — which is why one instance usually means the whole zone family is
affected. Instantiated per zone: `rht_vlv_cmd` is one box's valve.

## Detection Logic

```
yFault = rht_vlv_cmd > reheat_active_threshold
     AND oat > zone_heating_lockout_temp
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![SYS-0007 block graph](diagram.svg)

Four blocks, and either conjunct blocks the fault on its own.

Both comparisons are strict, which is what the reference writes (`>` in both
terms) rather than a reinterpretation of it, so a valve reported at exactly
10.0% and an outdoor air temperature of exactly 21.0 degC each fail their term.
Those exact values matter more than usual here, because 10% and 21 degC are
round numbers a retuned site will park on deliberately.

`persist` starts on the crossing, not at midnight, so a zone already reheating
when the weather crosses the lockout alarms 30 minutes after the crossing.
Sustained means continuous — a ten-minute dip below the lockout discards the
elapsed time rather than pausing it — and the fault clears on the tick the valve
shuts, because `TrueDelay` delays the rising edge only. It asserts at exactly
`T + delayTime`, and `delayOnInit = true` (CDL default `false`) makes a zone
already reheating at controller restart wait out the full 30 minutes.

## Possible Diagnoses

The reference's four, in its order:

1. **Zone heating lockout not programmed by OAT** — the sequence has no weather
   term at all. The common case, the cheap fix, and why findings arrive in batches
2. **Reheat valve stuck open** — mechanical, and distinguishable from the trend:
   a stuck valve reads the same command all day and ignores a commanded close
3. **Zone controller demanding heat due to sensor error** — a zone temperature
   sensor reading low makes the box genuinely believe the space is cold, and
   every part of the control chain then behaves correctly
4. **Perimeter heating operating independently of BAS** — baseboard or radiant
   perimeter on its own thermostat or reset curve, which the BAS neither
   commands nor sees

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT — the reference's profile.
The waste term needs no counterfactual: `waste_kw = rht_vlv_cmd/100 ×
vav_rht_capacity_kw` for every hour the condition holds, all of it pure loss
because the heat is applied to air the plant just paid to cool. The reference
puts it at 100% of reheat energy while active and up to 20% of the zone's annual
energy, at ~20% prevalence across the PNNL 151-building study. Cooling-dominant
by climate, but the multiplier is what matters: the defect is nearly always
systemic, so the site number is the per-zone number times the count of zones
sharing the sequence.

## Emissions Impact

Scope 1, DIRECT_EMISSIONS, HIGH confidence; the reference gives 500-4,000 kg
CO₂e/yr for reheat waste during warm weather, on a static Scope 1 factor. That
assumes hot-water reheat from a gas-fired boiler, the common case. Electric
reheat, or hot water from a heat pump or district loop, moves the same kilowatts
into Scope 2 — the quantity is unchanged and the inventory line is not, so hosts
should follow the actual heating source (the same caveat VAV-0006 carries).

## Deviations

- **`lockout_check_duration` and `AlarmDelay` are treated as one knob.** The
  reference's equation ends "sustained for `lockout_check_duration`" but its
  tunables table publishes three parameters, none named that, including
  `AlarmDelay = 30 min`. Both names describe how long the condition must hold
  and only one has a published number, so the graph carries a single `TrueDelay`
  at 30 minutes. This is the opposite call from VAV-0006 and SYS-0005, which
  chain two timers because their references publish two numbers; inventing a
  second here would put an unsupported duration in the card.
- **Both comparisons are strict, matching the reference's own operators,** so
  the library's standing `>=` → `>` deviation does not apply on this card.
- **Filed under SYS, instantiated per VAV or FCU zone.** The reference's header
  reads `Equipment: VAV, FCU` while the ID is `SYS-0007` — a tension in the
  source, not in this card. Deployment is one instance per zone with a host-side
  rollup, and `rht_vlv_cmd` and `oat` are duplicated into
  `points/sys.points.json` with matched groundings, because lint resolves a
  card's points against its own family dictionary.
- **No cross-zone aggregation in the graph, and the count is where the real
  diagnosis lives.** One zone reheating in summer is a zone problem; half the
  zones on an air handler is a supply-air-temperature problem, and the
  [vav-min-flow-reheat](../../../playbooks/vav-min-flow-reheat.md) playbook's
  step 1.4 turns that ratio into the discriminator. The graph cannot express a
  variable-width zone vector, so the rollup is the host's.
- **Overlaps VAV-0006 deliberately, and the two ask different questions.**
  VAV-0006 is reheat *at minimum flow* during the cooling season: three terms,
  an 18 degC season threshold, and a damper term that separates waste from a
  zone answering a genuine load. This card has no damper term and a 21 degC
  threshold, so it fires on a zone that is genuinely cold and genuinely being
  heated — because above 21 degC outdoors the reference's claim is that no zone
  should be heating at all. A box tripping this one alone is reheating with its
  damper open, which points at diagnosis 3 or an overcooled supply duct. Both
  are CLU-05 members with VAV-0006 as trigger, so the cluster encodes the fix
  order.
- **No occupancy gate, no mode gate, no supply-fan gate.** The reference
  specifies none, and adding one would change the fault: unoccupied reheat above
  21 degC outdoors is a worse instance of it, not an exception. The one
  defensible gate — a reheat coil downstream of a dehumidification coil, where
  warm-weather reheat is design intent — is a binding decision, so it lives in
  `preconditions`.
- **`oat` drift is the standing false positive and this card does not solve
  it.** A sun-baked or drifted outdoor sensor reading 3 K high manufactures the
  fault across every zone at once, which is also the tell. SYS-0005,
  SYS-0009 and SYS-0010 adjudicate `oat` directly; where a host runs them,
  an active sensor finding on the bound `oat` makes this rule NO_EVAL through
  the `adjudicates` fan-out.
- **`clusters: [CLU-05]` is a declaration, not an edit.** CLU-05 already lists
  SYS-0007 as a member with VAV-0006 as trigger.
  `playbooks/vav-min-flow-reheat.md` already names SYS-0007 in its step 2.3
  text but not in its Applies-To row; adding the ID there is the playbook
  owner's edit, flagged rather than made.
- **The reference's Notes block is truncated mid-sentence in the source
  document** — "Found in ~20% of buildings. In perimeter zones this is
  especially" — and is quoted below as far as the source runs. The perimeter-zone
  claim is not recoverable from the chapter text and has not been reconstructed.
- Severity 3, `method: rule`, phase 2 and the whole impact profile are the
  chapter card's, matching the provisional row in `faults/sys/README.md`; the
  reference's §5.8.1 index carries no severity column, so the chapter card
  governs.
- Operating states and preconditions are declared in frontmatter for host
  enforcement rather than encoded in the block graph, per the library's design
  stance.

## Notes

Reference note, quoted as far as the source runs: "Found in ~20% of buildings.
In perimeter zones this is especially".

Count the zones before dispatching anyone. This fault is systemic far more often
than mechanical: diagnosis 1 is a missing line of sequence logic nobody wrote for
any box, so the normal shape is dozens of zones alarming on the same warm
afternoon and clearing together when one lockout is programmed. A single zone
alarming while its neighbours stay quiet points at diagnoses 2, 3 and 4.

The remote fix is the [vav-min-flow-reheat](../../../playbooks/vav-min-flow-reheat.md)
playbook's step 2.3: a summer reheat lockout above an outdoor air temperature,
applied in batch, at no cost. Set it at the reference's 21 degC before arguing
about the number — lower and the site fights genuine morning heating loads in
the shoulder seasons, higher and it pays the difference in reheat.

Check the outdoor air sensor first, and check it once rather than per zone. It
is the single input every instance shares, a 3 K error moves every finding at
the site together, and verifying it against a hand-held reference takes ten
minutes.
