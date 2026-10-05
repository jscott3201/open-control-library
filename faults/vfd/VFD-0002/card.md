---
schema: cxf-library/fault-card/v1
id: VFD-0002
name: At minimum speed with load unsatisfied
equipment: vfd
status: verified
phase: 2
method: rule
severity: 3
category: COMFORT_ENERGY
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §15, VFD-0002"
  - "Engineering best practice"
g36: null
clusters: []
suppresses: []
suppressed_by: [VFD-0001, VFD-0005]
related: [VFD-0001, VFD-0003, VFD-0004, VFD-0005]
playbooks: [vfd-pump-faults]
operating_states: "drive enabled and its control loop active"
preconditions: "The drive must be enabled and its loop in automatic. A drive stopped, in hand, or overridden sits at or below minimum speed with the process variable wherever the building left it, which is this rule's exact signature and none of its meaning; the host owns that exclusion, and VFD-0005 suppresses this rule while remote automatic control is absent. vfd_process_value and vfd_process_sp must come from the same loop in the same units, and pv_error_threshold must have been retuned into those units — the shipped 10.0 is a placeholder, not a site value (see Deviations). vfd_speed is the drive's own feedback, so a drive that is not tracking its command undermines the minimum-speed premise: VFD-0001 (see suppressed_by) silences this rule while that is true. A loop whose setpoint is being reset by a trim-and-respond sequence must have a settled setpoint bound here, since a setpoint moving faster than the loop can follow produces a standing error at any speed."
points:
  - vfd_speed
  - vfd_process_value
  - vfd_process_sp
outputs:
  - name: yFault
    description: True while the drive has stayed at or below min_speed + speed_tolerance with the process variable more than pv_error_threshold from setpoint, continuously for at least sustained_duration
params:
  min_speed:
    default: 20.0
    unit: "%"
    description: The drive's configured minimum speed, in points of rated speed
    cxf: minSpd.k
  speed_tolerance:
    default: 3.0
    unit: "%"
    description: How far above the minimum the feedback may sit and still count as parked at minimum
    cxf: tol.k
  pv_error_threshold:
    default: 10.0
    unit: "1"
    description: Absolute deviation of the process variable from setpoint, IN THE LOOP'S OWN UNITS, above which the load counts as unsatisfied. PER-LOOP SITE CONFIGURATION — the shipped 10.0 is a placeholder carried over from the reference's "10%" and means nothing until it is set in the units the host binds
    cxf: pvOff.t
  sustained_duration:
    default: 900.0
    unit: s
    description: Continuous violation required before the alarm asserts (15 min). ADOPTED — the reference's tunables line is truncated and publishes no value
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: VFD-driven equipment at minimum speed
  savings_range: Context-dependent; equipment undersized or obstructed
  climate_sensitivity: neutral
  runtime_estimation: "Qualitative — no per-fault model; the reference points to Energy Impact Reference §4.4"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:eb1ff89459aa410286eaaad01914e3fa"
  date: 2026-10-05
---

## Description

A drive sitting on its minimum speed is a loop with no downward room left. That
is normal at light load — the whole point of a minimum is to keep the machine
above the speed where it stops cooling itself — and it becomes a finding only
when the process variable it is supposed to control is nowhere near setpoint at
the same time. Then the loop is pinned against a limit while the thing it
controls is wrong, and no amount of further control action will fix it. The
direction of the miss names the fault: below setpoint the loop wants more and
the drive will not give it (obstruction, undersized machine, torque limit);
above setpoint the minimum is set too high for the load, which is the more
common finding on a lightly loaded pump. The distinction is one subtraction away
in the host, from points it already has.

## Detection Logic

```
speed_floor = min_speed + speed_tolerance        (20.0 + 3.0 = 23.0 %)
at_min      = vfd_speed < speed_floor
pv_error    = |vfd_process_value − vfd_process_sp|

yFault = (at_min AND pv_error > pv_error_threshold)
         sustained continuously for sustained_duration
```

Block graph (`rule.cxf.jsonld`):

![VFD-0002 block graph](diagram.svg)

`minSpd` and `tol` carry the reference's two speed tunables as constants and
`speedFloor` adds them, so the composite trip point is assembled in the graph
rather than folded into a single threshold. That costs two blocks and buys
independent retuning: a site with a 30% minimum changes `minSpd.k` alone and the
tolerance keeps its own meaning (VAV-0001 assembles its trip point the same
way). `atMin` is a `Reals.Less` against that sum.

`err`, `absErr` and `pvOff` form the load term. Taking the absolute value before
the comparison is what makes the rule symmetric, and the symmetry is
load-bearing rather than incidental — over-delivery at minimum speed is a real
and distinct finding.

`both` conjoins the two terms and a single `persist` measures the duration: the
reference states one `sustained for sustained_duration` over the whole
conjunction, unlike VFD-0001's separately listed deviation window and alarm
delay. Any moment where either term releases drops the timer and discards the
accumulated time, so the alarm always describes one continuous episode. Both
comparisons are strict, which is where this rule departs from the reference's
`<=` (see Deviations): a drive at exactly 23.0% is not at minimum, and a process
variable exactly 10.0 units off setpoint is not unsatisfied.

## Possible Diagnoses

1. Minimum speed set too low — the reference's first diagnosis, which applies
   when the process variable is *below* setpoint; read the other way, a minimum
   set too *high* is what produces the over-delivery case
2. Mechanical obstruction reducing output: a closed isolation valve, a blocked
   strainer, a clogged filter bank, or a damper someone shut
3. System undersized for the actual load — a design finding rather than an
   operating one, and usually seasonal
4. Sensor error on the process variable — the loop is satisfying a setpoint the
   measurement is misreporting, and this rule cannot tell that from a real miss
5. VFD torque limit reached: the drive is holding speed down to protect itself,
   which reads as minimum speed from outside

## Energy Impact

COMFORT_ENERGY, LOW confidence, QUALITATIVE_ONLY. The reference offers no model
and this card does not invent one — the waste depends entirely on which
diagnosis holds. An obstruction spends the full minimum draw to deliver less
than it should; an oversized minimum on a lightly loaded pump is continuous
over-pumping, and the case where the cube law pays back on repair (the
`vfd-pump-faults` playbook notes that dropping pump speed by 20% drops pump
power by 49%); a sensor error spends energy chasing a number that was never
wrong. Comfort is the primary impact in the under-delivery direction, which is
what COMFORT_ENERGY records. Neutral climate sensitivity.

## Emissions Impact

Scope 2, QUALITATIVE_EMISSIONS, LOW confidence, basis N/A. VFD-driven equipment
is electric, so the whole consequence is purchased electricity, and the
reference's own range is "context-dependent; equipment undersized or
obstructed". The one case worth quantifying after diagnosis is the oversized
minimum, where the avoided draw is continuous and computable from the cube law
once the corrected minimum is known.

## Deviations

- **`pv_error_threshold` ships a placeholder default with no site authority.**
  The reference writes it as "10%" while its equation is an absolute difference;
  the point dictionary is canonical and resolves it in the loop's own units. So
  `pvOff.t = 10.0` means ten units of whatever the host binds — reasonable on a
  duct-static loop in pascals, and a rule disabled outright on one in inches of
  water. **Hosts MUST set it per loop.** Reading it as a percentage instead would
  need a division by setpoint and a setpoint-evaluability gate (VAV-0004's
  shape), which the reference's equation does not ask for.
- **`sustained_duration` has no published default.** The reference's tunables
  line for this card ends mid-sentence in both the chapter extract and the full
  text. This card adopts 900 s, the value the reference publishes for
  VAV-0004's structurally identical `tracking_duration` — a loop failing to
  reach setpoint, sustained. VFD-0001's 5-minute `deviation_duration` was
  rejected as a drive-response window, too short for a loop answering a load
  step. Hosts should retune to their loop's time constant.
- **`<=` becomes strict `<`.** The reference writes `vfd_speed <= min_speed +
  speed_tolerance`; CDL Reals has no `LessEqual`, so a drive reporting exactly
  23.0% reads as off-minimum. The disagreement is measure-zero, and the composite
  boundary is pinned from three sides because the trip point is a sum of two
  tunables that a host may move.
- **The floor is assembled in the graph, not folded into a threshold.** A single
  `LessThreshold` with `t = 23.0` would be one block instead of four and behave
  identically as shipped, but it would collapse two of the reference's four
  tunables into one number and force a host retuning the minimum to recompute the
  sum by hand.
- **Canonical point names replace the reference's** `process_variable` and
  `setpoint` with `vfd_process_value` and `vfd_process_sp` per
  `points/vfd.points.json`, which marks both provisional and deliberately
  untyped — the semantic tags belong on the application-specific point (duct
  static, differential pressure, supply temperature) the host binds underneath.
- **No evaluability output.** Both terms are direct comparisons on bound inputs,
  with no computed data-quality condition the host cannot see for itself.
  Contrast VFD-0001's `yCmdOk` and ERV-0001's `yTempDeltaOk`, which are
  derived. The exclusions that matter here — drive disabled, loop in hand,
  setpoint still ramping — are operating-state gating and live in frontmatter.
- **`suppressed_by: [VFD-0001, VFD-0005]` is authored**, not the reference's.
  This rule infers from `vfd_speed` that an automatic loop is pinned at its
  floor. Bad command/feedback tracking breaks the speed premise, while local or
  bypass operation breaks the automatic-loop premise. Both suppressors carry
  matching relationships and must be instance-scoped to this drive.
- **A stopped drive satisfies the speed term trivially, and nothing in the graph
  stops it.** Feedback of 0% is below any positive floor, so a stopped machine
  under an off-setpoint loop alarms — and since a stopped machine is usually
  *why* the loop is off setpoint, the alarm is near-guaranteed. The reference's
  equation has the same property and lists no run status; the exclusion is a host
  precondition, and the behaviour is pinned as a vector so any future in-graph
  run gate has to rewrite it deliberately.
- **The over-delivery direction is a deliberate keep.** The reference's absolute
  value admits it and this card implements it rather than narrowing to
  under-delivery, because "minimum set too high" is a genuine and common finding
  the same three points already detect. Hosts wanting only under-delivery read
  the sign from `vfd_process_value` and `vfd_process_sp` directly.
- `persist.delayOnInit = true` (CDL default is `false`): a drive already pinned
  at minimum with an unsatisfied load when the controller starts waits out the
  full 15 minutes rather than alarming on the first tick.
- **The reference's playbook Applies-To line does not name this card** — it lists
  only VFD-0001 and the two future PMP rules. The family README assigns the
  playbook to both VFD rules and the frontmatter follows it;
  `playbooks/vfd-pump-faults.md` carries VFD-0002 as an explicitly marked
  library addition.
- Frontmatter `clusters` is empty and `g36` is null: no cluster in the reference
  contains a VFD rule, and this is a research-backed 050-range card sourced to
  engineering best practice. The reference publishes no test vectors, so every
  scenario in `vectors.json` is library-authored.

## Notes

Diagnosis order in practice starts with the cheapest disambiguation, which is
the sign of the error rather than anything in the field. Below setpoint: check
for obstruction before touching the minimum, because raising the minimum on an
obstructed loop hides the fault and pays for it forever. Above setpoint: the
minimum is the first thing to look at, and lowering it to what the drive and the
driven equipment can actually tolerate is a BAS change with no capital cost. In
both directions, confirm the process-variable sensor against a second reading
before acting — diagnosis 4 costs nothing to rule out and invalidates everything
downstream of it.
