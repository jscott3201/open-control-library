---
schema: cxf-library/fault-card/v1
id: AHU-0001
name: Duct static pressure too low at full fan speed
equipment: ahu
status: verified
phase: 1
method: rule
severity: 3
category: PROTECTIVE
confidence: LOW
estimation_method: QUALITATIVE_ONLY
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0001"
  - "G36 §5.16.14 FC#1"
g36: "§5.16.14 FC#1"
clusters: []
suppresses: []
suppressed_by: []
related: [AHU-0024, AHU-0031, FPB-0002]
playbooks: []
operating_states: "OS 1–5 (all)"
preconditions: "Supply fan running — with the fan off both the pressure reading and the speed feedback are meaningless. The unit must be a multi-zone VAV AHU under duct static pressure control; a constant-volume unit has no dsp_sp to compare against. The speed feedback must be a real VFD readback rather than the commanded speed echoed back, since a defeated or bypassed drive reports 100% while the fan turns at line speed or not at all. When any gate is unmet the verdict is NO_EVAL, not healthy."
points:
  - dsp
  - dsp_sp
  - sf_speed
outputs:
  - name: yFault
    description: True while duct static pressure has stayed more than dsp_error_threshold below its setpoint with the fan above speed_full_threshold, for at least alarm_delay
params:
  dsp_error_threshold:
    default: 25.0
    unit: Pa
    description: Shortfall below the duct static pressure setpoint that counts as a real deficit rather than control error (0.1 inWC)
    cxf: gapBig.t
  speed_full_threshold:
    default: 99.0
    unit: "%"
    description: Supply fan speed above which the fan is treated as having no reserve left
    cxf: spdFull.t
  alarm_delay:
    default: 1800.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts (30 min)
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU fan system
  savings_range: "no direct savings — avoided fan and motor damage, plus indirect energy through the duct leakage or obstruction causing the deficit"
  climate_sensitivity: neutral
  runtime_estimation: "none in-rule — QUALITATIVE_ONLY; if a leakage or obstruction cause is confirmed, size the recovered fan energy per Energy Impact Reference §4.4 from fan hours and the pressure the fan is spending against the defect"
emissions:
  scope: "N/A"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:e621fd6c71d0e16c2eefa5e05edad265"
  date: 2026-10-05
---

## Description

The fan is at the stop and the duct is still short of pressure. VAV boxes
throttle against that pressure to hold zone airflow; without it they run wide
open and still starve. The control loop has already asked for everything and
the measurement has not moved, so the deficit is mechanical rather than a
tuning problem — a belt slipping on its sheaves, a fire/smoke damper that
closed and never reopened, a filter bank past its change-out point, or a VFD
that has silently derated all read the same way from the outside. This is a
protective fault: it catches the mechanical failure early and explains
complaints that would otherwise be chased zone by zone. The energy story is
real but indirect — a fan pinned at 100% against a leaking or obstructed duct
burns full fan power to deliver less than design air.

## Detection Logic

```
press_gap = dsp_sp − dsp
yFault    = (press_gap > dsp_error_threshold)      pressure short of setpoint by more than tolerance
        AND (sf_speed  > speed_full_threshold)     fan has no reserve left
            sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0001 block graph](diagram.svg)

The gap form is the reference's `dsp < dsp_sp − eps_dsp` rearranged so one
positive number carries the tolerance (see Deviations). The speed conjunct is
what makes the rule mean anything: pressure below setpoint at part speed is an
ordinary control loop working, and only a loop that has run out of fan is
evidence of a defect. Both comparisons are strict, so a deficit sitting exactly
on 25 Pa and a speed feedback parked exactly on 99.0% both read healthy.
`persist` requires 30 minutes of continuous violation — enough to separate a
mechanical fault from a morning start-up, a damper stroke, or the pressure dip
after a bank of boxes opens at once — and any interruption restarts the timer.

## Possible Diagnoses

1. Ductwork obstruction or collapse — a closed fire/smoke damper, a dropped
   internal liner, or debris left after a renovation
2. Fan belt slipping or broken
3. Fan motor or VFD fault — a drive derating on a thermal or current limit
   reports full speed command while delivering less
4. Excessive duct leakage — a disconnected branch or a failed flex connection
   downstream of the sensor
5. DSP sensor fault — a plugged or disconnected sensing tube reads low with a
   perfectly healthy duct behind it

## Energy Impact

PROTECTIVE, LOW confidence, QUALITATIVE_ONLY. No waste term is computable and
none is published: the alarm is about equipment damage and undeliverable
airflow, not kilowatt-hours. What energy there is depends on the cause — duct
leakage means the fan produces air that never reaches a zone, an obstruction
means the fan spends pressure across the blockage, and a slipping belt or
derated drive costs comfort and a repair bill instead. No PNNL measure covers
this fault, hence LOW confidence and no range. Climate-neutral. If a leakage or
obstruction cause is confirmed, size the recovered fan energy per Energy Impact
Reference §4.4 from fan hours and the pressure spent against the defect.

## Emissions Impact

QUALITATIVE_EMISSIONS, LOW confidence; negligible direct emissions. A fan
already at 100% draws what it draws whether or not the duct is short of
pressure, so detecting this fault avoids no emissions on its own. Scope is
`N/A` — there is no emitting stream to attribute, the general shape of a
PROTECTIVE fault. Any credit belongs to the repair that follows and to the DSP
reset it enables, which is AHU-0031's to claim. Avoided-emissions basis: N/A.

## Deviations

- **The fan-speed conjunct uses the reference's bare `sf_speed ≥ 99%`.**
  G36-2018 §5.16.14 FC#1 subtracts a VFD speed error allowance
  (`> 99% − eps_VFDSPD`, 94% at the Table 5.16.14.5 default); the reference
  ch.9 card simplifies to a bare 99% and this card follows its primary source.
  Drives that plateau just under 99% read NO-fault under the shipped default —
  retune `speed_full_threshold` to 94.0 for the G36 form.
- **`dsp < dsp_sp − eps_dsp` rewritten in gap form.** Subtracting first and
  testing `dsp_sp − dsp > eps_dsp` keeps `eps_dsp` the positive number the
  reference publishes, retunable at one CXF path, instead of negating it into
  an `AddParameter`. Same rearrangement as AHU-0028; the one-ulp difference
  at the threshold is not observable at 1 Pa sensor resolution.
- **`sf_speed >= 99%` → strict `>`.** CDL `Reals` offers only strict
  comparisons, so a feedback parked at exactly 99.000% reads as not-at-full-
  speed and the rule stays silent. A host binding a coarsely quantized speed
  point (integer percent) should retune `speed_full_threshold` to 98.9 rather
  than rely on the drive overshooting.
- **The fan-running condition is a precondition, not a wire.** The reference
  lists it under Preconditions rather than in its Logic row, so it stays in
  frontmatter for the host. This is the opposite choice from AHU-0031, where
  the reference puts `sf_status = ON` in the logic and the graph carries it.
- **Operating states OS 1–5 are declared, not gated.** The reference marks the
  fault applicable in every operating state, so the frontmatter records
  applicability and the graph carries no state logic.
- **First `PROTECTIVE` fault in the library, and the first with no emissions
  scope.** `emissions.scope` is the literal string `"N/A"`; the schema types
  the field as free text, so the convention is set here.
- `persist.delayOnInit = true` (Modelica/CDL default is `false`), the library's
  standing choice: a deficit already present at load waits out the full 30
  minutes instead of alarming on the first tick after a controller restart.

## Notes

Rule out diagnosis 5 first: a plugged sensing tube reads low forever and the fan
chases it to 100%, which is the same trace as a collapsed duct. Two tells
separate them — a genuine deficit moves when zone demand moves, and a healthy
duct with a bad sensor still delivers zone airflow. This fault is the low-side
mirror of AHU-0031 (excessive static pressure): same two signals, opposite
failure, and they cannot be true at once. If the DSP setpoint has never been
reset from its design value — the case AHU-0024 detects — a fan at 100%
against an unrealistic setpoint is a tuning artifact, not a broken belt.
