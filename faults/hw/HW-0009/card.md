---
schema: cxf-library/fault-card/v1
id: HW-0009
name: Boiler proof-of-operation failure
equipment: hw
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "ASHRAE Guideline 36-2021 §5.1.6 — the definition of *proven*: the equipment's DI status point matches the state its DO command point was set to. This rule is that comparison, held in both directions"
  - "ASHRAE Guideline 36-2021 §5.21.10.5 — the hot water plant's pump command/status alarm: commanded on with status off is Level 2 after 15 s, commanded off with status on is Level 4 after 60 s. Two directions, two windows, two severities — the shape this family instantiates per equipment"
  - "ASHRAE Guideline 36-2021 §5.1.15.5.b.1 — faulted equipment. Fans and pumps are faulted by status not matching command; a chiller by status still off five minutes after the start command, and only at the first start, 'because status will come and go if [it] cycles on low load'; a BOILER by its own safety-shutdown alarm contact and by leaving-water temperature, never by a status proof. Both halves of that clause shape this card (see Deviations)"
  - "ASHRAE Guideline 36-2021 §5.21.3 — boiler staging: wait five minutes for a newly enabled boiler to prove it is operating correctly. The published number behind start_proof_time"
  - "HVAC FDD Reference v1.0 ch.14 specifies no boiler proof-of-operation rule — its three cards are HW-0001/HW-0002/HW-0003. Name, severity 2 and category PROTECTIVE are argued here by analogy to PMP-0001, the reference's own command-versus-proof card"
  - "points/hw.points.json boiler_cmd (the plant-level enable the BAS writes, not a lead/lag sequencer's internal stage command) and boiler_status (the FIRING status, per HW-0001's contract)"
  - "Burner-management sequence timing — prepurge, pilot trial for ignition, main-flame establishing — comes from the burner control's listing and the boiler safety codes governing the vessel (in North America, ASME CSD-1 for smaller automatically fired boilers and NFPA 85 for larger combustion systems). No clause of either is cited here: the intervals are per-burner and published in the manufacturer's sequence of operation"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0001, HW-0002, HW-0010, PMP-0003]
playbooks: [hot-water-plant-faults]
operating_states: "all — the rule watches the enable itself, so there is no plant state in which it has nothing to say. The one state it must not be evaluated in is an enabled boiler that has stopped firing because it is satisfied (see preconditions)."
preconditions: "Four bindings and one gate. (1) boiler_cmd must be the enable the BAS actually writes to THIS boiler, per the dictionary note: a lead/lag sequencer's internal stage command changes state with no DO behind it, and a plant enable broadcast to several boilers makes every lag boiler read as firing without a command. (2) boiler_status must be the burner's FIRING (flame) status, HW-0001's contract. A status echoed back from the enable relay makes the two conjuncts one conjunct and the rule can never fire — PMP-0001's warning, and it bites harder here because the enable and the flame are separated by a whole burner-management sequence. (3) One instance per boiler, both points from the same boiler; the OR across a multi-boiler plant destroys the measurement exactly as it does in HW-0001. (4) start_proof_time must clear this burner's published light-off sequence including any listed recycle attempt (see Deviations). THE GATE: an enabled boiler that is satisfied stops firing while its enable stays true — its own operating control cycles the burner underneath the BAS. Every satisfied interval longer than start_proof_time then reads as a failure to start. The host must suspend yFailToStart while the boiler is enabled and satisfied (leaving water at or above its own setpoint, or the plant staged with no call), or bind an enable that already means *should be firing now*. G36 makes the same carve-out for chillers by evaluating status only at the first start; a two-point graph cannot see a first start, so the gate is host-side. Delivery quality is also host-side: a status held at its last value through a comms outage reads as a fault in whichever direction the stale value points, and the rule cannot tell that from the real thing."
points:
  - boiler_cmd
  - boiler_status
outputs:
  - name: yFault
    description: True while either direction has been asserting — the boiler has failed to prove firing, or is firing unbidden. The two directions are mutually exclusive, so yFault names one of them and the flags say which
  - name: yFailToStart
    description: "Sub-condition flag — enabled with no proven flame, continuously for start_proof_time. Not an evaluability output: false never means NO_EVAL"
  - name: yUnexpectedRun
    description: "Sub-condition flag — proven firing with no enable, continuously for stop_proof_time. Same kind as yFailToStart"
params:
  start_proof_time:
    default: 300.0
    unit: s
    description: "How long an enabled boiler may go without proving flame before the rule calls it a failure to start (5 min). G36 gives a newly enabled boiler five minutes to prove correct operation during a stage change (§5.21.3) and gives a chiller the same five minutes from its start command; a boiler needs every second of it, because the path from enable to flame crosses prepurge, pilot trial and main-flame establishing. COMMISSIONING VALUE against the burner's published sequence: a long prepurge or a listed recycle attempt pushes it toward 600 s"
    cxf: startProof.delayTime
  stop_proof_time:
    default: 120.0
    unit: s
    description: "How long a boiler may keep firing after its enable drops before the rule calls it an unexpected run (2 min). Twice G36's 60 s for a hot water pump (§5.21.10.5), because a burner's controlled shutdown and post-purge can hold a derived status point true after the fuel valve has closed, and this is the direction where a false alarm sends someone to a boiler that is merely finishing"
    cxf: stopProof.delayTime
energy_impact:
  affected_subsystem: "Heating availability first; in the unexpected-run direction, 100% of the fuel a boiler burns with nothing asking for it"
  savings_range: "direction-dependent — none for yFailToStart (a boiler that will not fire burns nothing; the cost is unmet heat and whatever carries the load instead), 100% of that boiler's fuel input while yUnexpectedRun is active"
  climate_sensitivity: heating-dominant
  runtime_estimation: "waste_kw = fuel_power while yUnexpectedRun is active and zero while yFailToStart is. fuel_power is HW-0002's point and the host supplies it; this rule reads no meter. The fail-to-start direction has no waste term at all — its cost is availability, which this library does not price"
emissions:
  scope: "1"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:22ca393fffc23948414362e2dd3ab69c"
  date: 2026-10-05
---

## Description

An enable is a request; firing is an outcome, and on a boiler the distance
between them is a whole burner-management sequence — prepurge, pilot trial for
ignition, main-flame establishing, each with its own proving switch. When the
sequence does not complete, the burner controller locks out and stays locked
out while the BAS keeps writing an enable into a boiler that has already decided
not to run. From the safety system's point of view nothing is broken; a lockout
is the burner control doing its job. What is broken is availability, and in
January a locked-out lead boiler is an emergency even though every component
behaved correctly. The mirror case — firing with no enable, from a switch left
in HAND or a welded contactor — burns fuel nobody asked for.

## Detection Logic

```
yFailToStart   = (boiler_cmd  AND NOT boiler_status)  sustained for start_proof_time
yUnexpectedRun = (boiler_status AND NOT boiler_cmd)   sustained for stop_proof_time

yFault         = yFailToStart OR yUnexpectedRun
```

Block graph (`rule.cxf.jsonld`):

![HW-0009 block graph](diagram.svg)

Seven blocks: a negation, an And and a delay per direction, and one Or. The two
And gates cannot be true on the same tick, and each delay drops its output the
moment its And does, so the flags are mutually exclusive by construction — a
mismatch that changes direction takes the second window from zero
(`hand_switch_flip_never_asserts_both_directions`).

Both flags are diagnostic. Neither is an evaluability output and there is no
NO_EVAL condition inside this graph: the rule is evaluable whenever both points
are delivered, and whether they are is the host's delivery-quality job. Silence
means command and status agree, which is the healthy answer in both states.

`delayOnInit = true` on both delays. A restart serves the full window before
either direction can assert, which does real work here: a plant coming back up
is exactly when a boiler *is* mid-light-off, and the CDL default would report
the sequence this rule was built to wait for.

## Possible Diagnoses

Read the flags first; they split the list in two.

**yFailToStart — enabled, not firing:**

1. **The burner management system has locked out** — flame failure, low-water
   cutoff, high limit, low gas pressure, a combustion-air proving switch. The
   controller holds a code until someone resets it, and that code names the
   fault this rule can only point at.
2. **No fuel to burn.** A manual gas cock closed after service, a tripped safety
   shutoff valve, a regulator or gas-train pressure switch out of range. Common
   after any work on the fuel train.
3. **The burner cannot spin.** Combustion-air blower overload tripped, drive
   faulted, disconnect open, coupling or belt gone.
4. **Nothing is wrong.** The boiler is enabled and satisfied, its burner off by
   its own operating control — the precondition gate, and the first thing to
   rule out on a plant that has not implemented it.
5. **The status point failed low** — a current switch set above the burner's
   actual draw, a flame-relay contact, or a point bound to the wrong boiler.

**yUnexpectedRun — firing, not enabled:**

6. **Local control has the machine** — Hand/Off/Auto in HAND at the burner
   panel, or the boiler firing on its own aquastat with the BAS out of the loop.
   The most common cause, usually left over from a service call.
7. **The command never reaches the boiler, or never leaves.** Welded contactor
   or relay, an output wired to the wrong terminal, an enable inverted at the
   interposing relay.
8. **The status point is stuck true** — a latched current switch, or a point
   bound to a boiler that really is running.

## Energy Impact

PROTECTIVE, MEDIUM confidence, QUALITATIVE_ONLY. The directions have opposite
economics. A boiler that will not fire wastes no fuel; it costs unmet heat and
whatever carries the load instead, neither of which this library prices. A
boiler firing unbidden wastes its whole fuel input, directly measurable wherever
the host binds `fuel_power`. MEDIUM because two booleans disagreeing is
unambiguous evidence of *something* while the diagnosis stays wide open, and
because the status point is uncorroborated — diagnoses 5 and 8 are that point
failing.

## Emissions Impact

Scope 1, QUALITATIVE_EMISSIONS. Fuel burned at the boiler is a direct emission,
so the unexpected-run direction abates Scope 1 at the plant rather than Scope 2
at the meter — HW-0003's basis, and the reason a heating-plant waste term is
worth more per kWh than the same number on a fan. The fail-to-start direction
carries no emissions term of its own; whatever picks up the load carries it,
and electric backup heat usually carries it badly.

## Deviations

- **Composed from `Logical.Not` + `Logical.And` + `Logical.TrueDelay` per
  direction plus one `Logical.Or`, rather than `CDL.Logical.Proof`.** Proof is
  exported at the pinned rev and loads (a probe graph exported a content id),
  but four of its published behaviors, each reproduced on that probe, break this
  template: it starts checking as soon as *either* the `feedbackDelay +
  debounce` timer lapses or the measurement has been stable for `debounce`,
  whichever is first, so a status stably false all night gets no proof window at
  all (`yLocFal` asserted on the same tick the command rose); one window serves
  both directions, so 300 s to start and 120 s to stop cannot both be expressed;
  both outputs latch true together on an unstable measurement, a third meaning
  the mutual-exclusivity contract cannot carry, and hold until a stable-equality
  edge; and its internal delay-on-init is fixed, alarming from the first tick.
  None of that is a defect — it is the block designed for fans and pumps, whose
  status follows the command in seconds.
- **`start_proof_time` 300 s.** G36 gives a newly enabled boiler five minutes to
  prove correct operation at a stage change (§5.21.3) and a chiller five minutes
  from its start command (§5.1.15.5.b.1.ii); a pump gets 15 s, and the spread is
  the burner sequence. Still a commissioning value on the HP-0001 convention:
  prepurge, trial for ignition, flame establishing and any listed recycle
  attempt live in the burner manufacturer's sequence of operation, and no
  portable number covers both a fire-tube with a 90-second purge and a
  condensing boiler that lights in twenty.
- **`stop_proof_time` 120 s, deliberately not G36's 60 s.** The pump number is
  for a device that stops when its starter opens; a burner runs a controlled
  shutdown and post-purge that a derived status point can stay true through.
  Double the pump window buys that margin in the direction where a false alarm
  costs the most credibility and the least energy — G36 rates it Level 4 against
  Level 2 for the other.
- **One severity for two directions.** G36 splits them (Level 2 commanded-on,
  Level 4 commanded-off); one card carries one severity, set by the worse
  direction — a lead boiler that will not fire. Hosts wanting the split read the
  flags, which is what they are for.
- **This is an availability rule and must never be read as a safety layer.** A
  failed proof usually means the burner management system locked out, which is
  the safety system working. G36 makes the same architectural choice: a boiler
  is declared faulted by its own safety-shutdown alarm — network or hardwired
  contact — and by leaving-water temperature, never by a status proof
  (§5.1.15.5.b.1.iii). Where that contact reaches the BAS it is the better
  signal and it names the cause; this card is the coverage rule for the many
  plants that never bring it in, and it sits downstream of every interlock
  rather than beside them.
- **The satisfied-boiler gate is host-side and the rule is wrong without it.**
  G36 solves the same problem for chillers by evaluating status only at the
  first start, on the stated grounds that status comes and goes when equipment
  cycles on low load. Reproducing that needs edge and latch state the family
  template does not carry, and it would put an operating-state decision inside a
  graph the design stance keeps status-blind. The precondition names it instead,
  where the host can meet it with the setpoint it already has.
- **No evaluability output; both boolean flags are direction flags.** No in-rule
  condition makes "no mismatch" anything other than an answer. What can make the
  verdict meaningless is a stale or missing point, and that is delivery quality
  — the host's job under the design stance, not a boolean this graph could
  compute from the two points it is judging.
- **`delayOnInit = true` on both delays** (CDL default `false`), the library's
  standing choice, load-bearing here: an engine restart during a plant restart
  lands mid-light-off, and the default would report the burner-management
  sequence as a failure to complete it.
- **`suppresses: []`.** A boiler that will not fire leaves the loop cold, but
  HW-0004 and HW-0007 already require the plant to be making heat as a host
  precondition, so an edge here would duplicate a gate those cards own. HW-0001
  is `related` in both directions rather than suppressed: a lockout-retry cycle
  produces real starts and HW-0001 should count them (see Notes).
- **`g36: null` despite four G36 citations.** The field carries the clause a
  transcribed fault condition came from, and this rule transcribes none: G36
  defines *proven* (§5.1.6) and specifies this alarm for pumps (§5.21.10.5), but
  its hot water plant AFDD routine (§5.21.11) has no command-versus-status fault
  at all. The citations live in `source`.
- **`playbooks: [hot-water-plant-faults]`, with a gap.** It has no step for a
  boiler that will not prove; the nearest content is Step 2.5, low water flow
  tripping a safety, filed under short-cycling, and its Applies-To row does not
  name this rule. Both edits belong to the playbook's owner; the Notes carry the
  field procedure meanwhile, on the RTU-0009 precedent.
- **Name, severity 2, `category: PROTECTIVE` and `method: rule` are authored**,
  mirrored from PMP-0001 and VFD-0001, the library's other command-versus-proof
  cards. No published test vectors exist for this rule; every scenario in
  `vectors.json` is authored from the equation and replayed against the pinned
  engine rev.

## Notes

Read the burner controller before the trend. A locked-out boiler annunciates
*why*, and that code is worth more than everything in this card — the rule's
contribution is noticing at 3 a.m. on a Sunday instead of when the building
opens cold. Run it beside HW-0001: a burner that locks out, retries, proves
flame for a minute and locks out again shows up as short-cycling first and as a
flickering `yFailToStart` here (`flame_pulse_clears_and_restarts_the_proof` pins
that shape), and both firing together mean the same visit. PMP-0003 is the pump
instance of the same template; its start window is 15 s against this card's 300,
which is the burner sequence measured in parameters.
