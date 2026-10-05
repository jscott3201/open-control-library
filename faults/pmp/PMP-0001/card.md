---
schema: cxf-library/fault-card/v1
id: PMP-0001
name: Pump commanded on, no flow detected
equipment: pmp
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §15 (ch. 'Pumps', pdf pp. 133-134), PMP-0001"
  - "G36 alarm patterns (the reference's citation; no clause identified — see Deviations)"
  - "Engineering best practice"
g36: null
clusters: []
suppresses: []
suppressed_by: [PMP-0002]
related: [PMP-0002, PMP-0005, PMP-0006, VFD-0001, HX-0002]
playbooks: [vfd-pump-faults]
operating_states: "pump commanded on and proven running, past the start transient — the rule's own yRunOk is that state"
preconditions: "pump_cmd, pump_status, and pump_flow must belong to the same pump. On a headered set that is the precondition most likely to be violated: sites commonly trend one loop flow meter and bind it to every pump on the header, and a lag pump that is running and delivering nothing then reads the lead pump's flow and is never detected. Bind the pump's own flow element, or accept that the rule only sees a whole-header failure. pump_flow must be in L/s (the rule converts nothing) and no_flow_threshold must have been set from this loop's design flow — the shipped 1.0 L/s is a placeholder, not a site value (see Deviations). The flow measurement itself is uncorroborated: nothing in this rule cross-checks the meter, and the reference's own diagnosis 5 is that the meter is what failed. Run status should be proof of rotation (a current switch or the drive's own run feedback), not a repeat of the command from a relay; a status point wired back from the start contactor makes the two conjuncts one conjunct. Evaluability is signalled in-rule by yRunOk: when it is false the verdict is NO_EVAL, not healthy, and that covers an idle pump, either kind of command/status mismatch, and the first flow_check_delay seconds of every start."
points:
  - pump_cmd
  - pump_status
  - pump_flow
outputs:
  - name: yFault
    description: True while the pump has been commanded on and proven running for at least flow_check_delay with its flow below no_flow_threshold, continuously for a further alarm_delay
  - name: yRunOk
    description: Evaluability signal — true when pump_cmd and pump_status have both been true for flow_check_delay, which is when the flow reading means something. False means NO_EVAL and the host must ignore yFault
params:
  no_flow_threshold:
    default: 1.0
    unit: L/s
    description: "Flow below which the pump is delivering nothing. PER-LOOP SITE CONFIGURATION — the reference states this as 5% of design flow and the rule carries absolute units, so the shipped 1.0 L/s is 5% of a 20 L/s design flow and means nothing on any other loop (see Deviations)"
    cxf: noFlow.t
  flow_check_delay:
    default: 60.0
    unit: s
    description: "How long the pump must be proven running before its flow reading is believed (1 min). The reference's own description is 'wait after pump start', so it is applied to the run condition rather than to the fault condition — see Deviations"
    cxf: settled.delayTime
  alarm_delay:
    default: 300.0
    unit: s
    description: "Continuous no-flow required, after the run condition has settled, before the alarm asserts (5 min). The reference's AlarmDelay, renamed to the library's convention"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Pump energy (no useful work)
  savings_range: "100% of pump energy while active, plus avoided motor and seal damage (HVAC FDD Reference §15)"
  climate_sensitivity: neutral
  runtime_estimation: "waste_kw = pump_rated_kw × (pump_speed/100)³ — the reference's formula verbatim. Both of its terms are outside this rule: pump_rated_kw is nameplate data and pump_speed is the drive feedback the VFD family binds as vfd_speed, so the host supplies them. On a constant-speed pump the cube term is 1 and the whole draw is the waste; on a drive at 60% speed it is about a fifth of nameplate"
emissions:
  scope: "2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:3fa07dea0d2d769e0de5ca7bed13e0c1"
  date: 2026-10-05
---

## Description

A pump that is running and moving no water is the cheapest fault in the building
to describe and one of the more expensive to leave alone. Every kilowatt going
into the motor goes into churning water and spinning bearings; none of it
reaches a coil. The building notices eventually — a chilled water loop that will
not hold temperature, a heating branch that never warms — but by then the
finding is a comfort complaint being chased at the air handler, a subsystem away
from the pump that caused it. The rule is the direct test: the pump says it is
on, the starter says it is running, the flow meter says nothing is moving. Its
value is coverage — impeller damage, a closed isolation valve, an air lock, a
sheared coupling and a failed flow meter all fire it, which is also why the
confidence rating is MEDIUM rather than HIGH.

## Detection Logic

```
yRunOk = (pump_cmd AND pump_status) sustained for flow_check_delay
                                        (false ⇒ host reports NO_EVAL)

yFault = (pump_flow < no_flow_threshold AND yRunOk)
         sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![PMP-0001 block graph](diagram.svg)

`running` forms the reference's first two conjuncts and `settled` holds them for
`flow_check_delay` before the rest of the rule believes anything. That placement
is the one structural decision in this card: a start is exactly when a healthy
pump reads zero flow — the check valve has not lifted, the volute is still
filling — so delaying the run condition rather than the fault condition means
the rule has no verdict at all for the first minute of every run, and `yRunOk`
tells the host so.

The consequence is that the two delays are not interchangeable with one longer
delay, which is where this card differs from VFD-0001's superficially similar
chain. A pump that starts dry alarms at 360 s, the two delays adding; a pump
that has been running for hours and loses its flow alarms at 300 s, because the
run condition settled long ago and only `persist` is left to run.

`yRunOk` is also the rule's whole answer to the reference's NO_EVAL row. Four
states hold it down and none of them is a healthy pump: the pump is off, it is
commanded on and not running, it is running while commanded off, or it started
less than a minute ago. `noFlow` is a strict `Reals.LessThreshold`, so flow
sitting exactly on the threshold is not no-flow.

## Possible Diagnoses

Transcribed from the reference's PMP-0001 card:

1. Pump impeller failure — eroded, corroded, or spun loose on the shaft; the
   pump turns, the head collapses, and the loop gets nothing
2. Isolation valve closed — the cheapest cause and the most common one, and what
   the playbook's first on-site step (2.3) goes looking for
3. Air lock in the pump — a bubble the pump cannot push through, often
   self-clearing and then recurring
4. Broken coupling between motor and pump — near no-load motor current with a
   stationary pump shaft, free to confirm with a clamp meter
5. Flow meter failure — the pump is fine and the measurement is wrong; this rule
   cannot tell that from any of the four above, and it is the first to rule out
   because it is the only one that costs nothing to check

The discriminator for 1 against 2 and 4 is the pump differential pressure, which
this rule does not read and PMP-0002 does: high DP with no flow is a deadhead,
low DP with no flow is a pump that has stopped making head.

## Energy Impact

PROTECTIVE, MEDIUM confidence, DIRECT_MEASUREMENT. The savings line is the whole
pump — 100% of its energy while the condition lasts, because none of it is doing
work — and climate sensitivity is neutral. DIRECT_MEASUREMENT is the reference's
rating, defensible with one caveat: the measurement it refers to is the pump's
own power, which this rule does not read, so `waste_kw = pump_rated_kw ×
(pump_speed/100)³` is only as good as the nameplate and speed feedback behind
it. The larger cost is usually not energy at all — a pump running dry runs its
mechanical seal without the water that lubricates and cools it, and that damage
is measured in thousands rather than kilowatt-hours.

## Emissions Impact

Scope 2, DIRECT_EMISSIONS, MEDIUM confidence; the reference's typical range is
100–500 kg CO₂e/yr for pump energy doing no useful work, on a marginal operating
emissions rate (MOER) basis. Pumps are electric everywhere, so the scope
assignment is not site-dependent the way a heating fault's is. The avoided motor
and seal damage is real and deliberately not in the range — the reference counts
it as protective value rather than as CO₂e, and so does this card.

## Deviations

- **`no_flow_threshold` ships an absolute placeholder, not the reference's
  percentage.** The reference gives 5% of design flow; the rule carries L/s
  because CDL parameters carry units and this library does no unit conversion in
  v1. The shipped 1.0 L/s is 5% of a 20 L/s design flow and is arbitrary on any
  other loop — 25% of design on a 4 L/s zone pump (alarms at normal part load),
  1% on a 100 L/s primary (detects almost nothing). **Hosts MUST set it per
  loop**, as `pump_flow`'s dictionary notes require. Precedent: VAV-0001's
  `ventilation_requirement`.
- **A variable-speed pump at genuine low load can sit under 5% of design flow
  with nothing broken**, because the threshold is a fraction of *design* flow,
  not of current demand. The reference's stance — a pump making no useful work is
  a finding regardless of why — is right for a protective rule, but the action
  differs: that case is a missing minimum-flow bypass or a deadhead, PMP-0002's
  territory. Sites seeing this rule fire at low load should read PMP-0002 first.
- **`flow_check_delay` gates the run condition, not the fault condition**, which
  is what the reference's own description ("wait after pump start") asks for.
  Applying it to the conjunction instead — VFD-0001's and AHU-0027's shape —
  would give the same 360 s alarm on a dry start and a 60 s slower alarm on every
  mid-run flow loss, for no stated reason. This chain is therefore not a
  repackaged single delay, and collapsing the two tunables would change behaviour.
- **The command/status mismatch is NO_EVAL here and is nobody's fault rule
  today.** A pump commanded on that never proves running, and a pump running
  against a command of off, are both real and both silent in this library:
  `yRunOk` goes false and this rule stands down. The check that would cover them
  is a status-versus-command comparison the library has for fans only in the
  after-hours sense (AHU-0018, RTU-0006). The reference's vector table has
  the same hole — it publishes OFF/OFF as NO_EVAL and states no verdict for the
  mismatch rows — so this decision is the card's, not a transcription.
- **The flow meter is uncorroborated and the rule reports its failure as a pump
  fault.** Diagnosis 5 is in the reference's list and nothing in three points
  separates it from the other four. The nearest cross-check is `pump_dp`, which
  this rule does not bind: a failed meter leaves DP at its normal operating
  point, an impeller failure collapses it, a deadhead raises it. That is a
  host-side read of a dictionary point, and the reason to run PMP-0002 on the
  same pump.
- **`yRunOk` is the library's, not the reference's.** The reference writes the
  command and status tests as conjuncts of the fault condition and the graph
  computes exactly that; exposing the settled conjunction adds no logic and
  changes no verdict. It is a computed signal — the AND of two booleans held for
  a delay — which is what SCHEMA.md asks an evaluability output to be.
- **Strict `<` at the flow threshold.** The reference writes `<` too, and CDL
  `Reals` has no `LessEqual` in any case, so flow of exactly 1.0 L/s reads as
  flow. The disagreement is measure-zero and errs toward silence; all three sides
  are pinned. A site whose BAS quantizes flow coarsely, or clamps small readings
  to zero as many meters do, should retune rather than rely on the signal landing
  off the boundary.
- **`suppressed_by: [PMP-0002]` is an authored relationship**, not the
  reference's — neither card declares suppression. Both rules fire on one
  physical event (every valve on the loop closed against a running pump), and
  PMP-0002 is the specific diagnosis where this is the general condition.
  Worse than redundant, this card's diagnosis list is actively misleading on a
  deadhead: impeller failure, air lock and a broken coupling all produce *low*
  pump DP. Precedent for authoring the edge: the VFD-0001/VFD-0002 pair.
- **The two rules do not alarm at the same moment.** On a mid-run valve closure
  both land 300 s after the event; on a pump started into a closed system this
  rule waits out `flow_check_delay` and lands at 360 s while PMP-0002 lands at
  300 s. A host implementing the suppression as "drop PMP-0001 if PMP-0002 is
  active" is right in both cases; "drop it only if PMP-0002 alarmed first" is
  right in one.
- **The energy formula's inputs are not this rule's inputs.** `waste_kw =
  pump_rated_kw × (pump_speed/100)³` needs nameplate power and drive speed, and
  the pump dictionary carries neither. The host supplies them — speed is the VFD
  family's `vfd_speed` where a drive exists, and the term is 1 where one does
  not. Transcribed unchanged otherwise.
- **Persistence stands in for averaging.** The rule consumes instantaneous
  points; the reference specifies no averaging and G36's AHU set would have used
  a 5-minute rolling mean. The two are not equivalent — flow alternating between
  zero and normal every four minutes never accumulates `alarm_delay`, though a
  pump catching and losing its prime all day is a genuine finding. A steady
  failure reads the same either way.
- **`AlarmDelay` is renamed `alarm_delay`.** The reference's tunables table
  spells this one parameter in G36's PascalCase while spelling its two neighbours
  in snake_case. The value is unchanged at 5 min and the CXF path
  (`persist.delayTime`) is the one every other card exposes for the quantity.
- **The chapter number is uncertain.** The reference's page headers label Energy
  Recovery, Pumps and Variable Frequency Drives all as "Ch. 15", which cannot be
  right for all three. `source` follows the VFD cards' §15 and names the chapter
  title and page range so the citation resolves regardless.
- **`g36` is null although the reference cites G36.** Its source line reads "G36
  alarm patterns; engineering best practice" — a family of patterns, not a
  clause — so there is nothing to put in the field and the claim is carried in
  `source` as prose. Contrast FCU-0005, where the reference names §5.22.6.
- `settled.delayOnInit` and `persist.delayOnInit` are both `true` (CDL default is
  `false`), the library's standing choice: a pump already running dry at
  controller restart waits out the full 360 s, and its flow reading is treated as
  untrustworthy for the first minute exactly as after a real start.
- Three of the reference's test vectors are published and reproduced under their
  own names — normal operation, no flow while running, and pump off; the
  remaining eleven scenarios in `vectors.json` are library-authored.
- `clusters` is empty: `clusters/clusters.json` defines no cluster containing a
  pump rule, and this card does not edit the cluster set.

## Notes

Read `yRunOk` before reading `yFault`. On a lead/lag pair the standby pump's
`yRunOk` is false for weeks at a time, and every `yFault = false` under it means
"not evaluated", not "no problem". The
[vfd-pump-faults](../../../playbooks/vfd-pump-faults.md) playbook owns the
service procedure — its step 2 takes this fault and PMP-0002 together in the
order that costs least: DP setpoint and reset sequence remotely, then closed
isolation valves and a blocked strainer on site, then the impeller. Step 2.6 is
worth reading before deploying on a variable-primary chilled water plant, where
a missing minimum-flow bypass deadheads the lead pump whenever the last AHU
valve closes and presents as a fleet of these alarms. That playbook's header and
the chapter README both still list the pump family as future work; both belong
to other owners to correct.
