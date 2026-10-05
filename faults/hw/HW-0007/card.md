---
schema: cxf-library/fault-card/v1
id: HW-0007
name: HW supply temperature too high at low load
equipment: hw
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "PNNL-27338 §4.4.2 (pp. 4.12-4.13), high HW supply temperature: pump VFD speed below 35% together with supply water above 130 °F over the averaging window"
  - "PNNL-27338 §4.5.2 (pp. 4.16-4.17) — the missing HWS reset this fault is usually the water-side symptom of; HW-0008 is its detector"
  - "PNNL-27338 §1.2, §2.1 — the data_window / no_required_data / max_dx_time gating this library expresses as preconditions and evaluability outputs"
  - "PNNL-27338 (Katipamula et al. 2018) — adapted via an internal paraphrased deep-read digest, not distributed (HW candidate #6)"
  - "Library-authored extension: faults/hw/README.md index (name, severity, method); the HVAC FDD Reference v1.0 ch.14 specifies only HW-0001..052 and does not contain this rule"
  - "Sibling-rule precedent: CHW-0004 (evaluability floor + hour of persistence), HW-0003 (threshold + TrueDelay on a hot water plant), VFD-0001 (min_cmd_for_eval / yCmdOk)"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HW-0008, HW-0004, HW-0005, HW-0006, HW-0003, HW-0010]
playbooks: [hot-water-plant-faults]
operating_states: "Heating plant enabled with the distribution loop circulating above min_pump_speed_for_eval — the rule's own yLoopOk is that state"
preconditions: "hws_temp and hw_pump_vfd_speed must describe the same loop. On a primary/secondary or injection-mixed plant that is the precondition most often violated: the boiler's own leaving-water temperature is high by design and says nothing about what the building gets, so bind hws_temp on the system side of the decoupler or mixing valve, and bind the speed from the distribution pump that serves it rather than a boiler primary circulator. On a lead/lag distribution pair, bind the lead pump's speed or a host-computed plant speed; two pumps at 30% each are not one pump at 30%, and the graph cannot tell them apart. The loop must be variable-speed: a constant-volume distribution pump has no speed signal worth reading and this rule does not apply to it. Before first deployment, check high_hws_temp_threshold against the plant's design supply temperature and the low end of whatever reset schedule exists — 54.4 °C is PNNL's number for a typical hydronic system and it is below the design water temperature of an old cast-iron radiator plant, which will read faulted at every mild hour until someone decides whether the emitters can actually accept cooler water. Sites with combined heating/DHW plants must exclude the rule or gate it host-side, exactly as HW-0003 requires: a boiler holding 60 °C for service water is not a heating fault. Evaluability is signalled in-rule by yLoopOk: when it is false the verdict is NO_EVAL, not a plant making appropriate water."
points:
  - hw_pump_vfd_speed
  - hws_temp
outputs:
  - name: yFault
    description: True while the HW distribution pump has stayed below low_load_speed_threshold with supply water above high_hws_temp_threshold and the loop circulating, continuously for alarm_delay
  - name: yLoopOk
    description: Evaluability signal — true when hw_pump_vfd_speed is above min_pump_speed_for_eval, the speed below which the loop is not moving enough water for its supply temperature to describe how the plant is serving the building. False means NO_EVAL and the host must ignore yFault
params:
  low_load_speed_threshold:
    default: 35.0
    unit: "%"
    description: "Pump speed below which the loop counts as lightly loaded (PNNL-27338 §4.4.2). Pump speed is the load proxy: on a DP-controlled loop the drive slows as coil valves close, so a low speed means the building is drawing little of what the plant is making"
    cxf: lowSpeed.t
  high_hws_temp_threshold:
    default: 54.4
    unit: "°C"
    description: "Supply water temperature above which the plant is making full-temperature water (PNNL-27338 §4.4.2's 130 °F). PER-PLANT SITE CONFIGURATION — check it against the design supply temperature and the bottom of the reset schedule before trusting a verdict (see Deviations)"
    cxf: hotSupply.t
  min_pump_speed_for_eval:
    default: 10.0
    unit: "%"
    description: "Pump speed at or below which the loop is not judged to be circulating and nothing is evaluated. ADOPTED — PNNL specifies no plant-running gate, and without one a stopped pump reads as the lightest possible load (see Deviations)"
    cxf: circulating.t
  alarm_delay:
    default: 3600.0
    unit: s
    description: "Continuous low-load-with-hot-water required before the alarm asserts (60 min). ADOPTED — PNNL publishes no alarm persistence for this measure, only a 15-60 min averaging window; 60 min is the top of that range and the library's standing plant-rule persistence (HW-0003, CHW-0004)"
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Boiler fuel and distribution standing losses
  savings_range: "1-3% of site energy for HW supply-temperature reset and DP reset together (playbooks/hot-water-plant-faults.md, from PNNL-27338's hot-water measure set); the split between the two is not published"
  climate_sensitivity: heating-dominant (worst in the swing seasons and the mild hours of the heating season, which is where most of the heating hours are)
  runtime_estimation: "No closed-form estimator. The waste is the sum of two terms the rule does not measure: distribution standing loss, which scales with the difference between water and surroundings along every metre of pipe, and the boiler's own efficiency penalty for running hotter than the load requires — largest on a condensing boiler held out of condensing range by a return temperature that follows the supply. A host with fuel_power (HW-0002's point) can bound the first term by trending fuel input across the flagged hours, when useful delivered heat is small by construction"
emissions:
  scope: "1"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:47d67bb2538aa6a4e4d0fcfb95b2726e"
  date: 2026-10-05
---

## Description

A hot water plant that cannot lower its supply temperature spends the heating
season paying design-day prices for mild-day heat. The boiler makes water hot
enough for the coldest hour of the year and on a 10 °C afternoon the building
takes almost none of it; what is left is standing loss plus a boiler at the
least efficient end of its curve — on a condensing boiler, never condensing at
all, because the return comes back as hot as the supply went out.

The rule reads that off two signals: pump speed says how much of the plant's
output the building is drawing, because a DP-controlled drive slows as the coil
valves close, and supply temperature says what the plant is making anyway. Light
draw with full-temperature water is nearly always the water-side symptom of a
reset that is missing, disabled or bottomed out too high, which is why HW-0008
sits next to this card. Library-authored: ch.14 has no such rule, and the logic
and both published thresholds come from PNNL-27338 §4.4.2.

## Detection Logic

```
yLoopOk = hw_pump_vfd_speed > min_pump_speed_for_eval   (false ⇒ host reports NO_EVAL)

yFault  = yLoopOk
      AND hw_pump_vfd_speed < low_load_speed_threshold
      AND hws_temp > high_hws_temp_threshold,
          sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![HW-0007 block graph](diagram.svg)

`lowSpeed` and `hotSupply` are PNNL's two conditions, both strict and both
reading their thresholds as parameters. `lightLoad` and `overTemp` chain
`Logical.And` rather than using a `MultiAnd`, which no card in this library
does.

`circulating` is the third comparison and the one PNNL does not specify. The
low-speed side of a low-speed test is exactly where a stopped pump lives: a
plant on overnight setback reads 0% with 65 °C water standing in the boiler and
satisfies both published conditions perfectly. The same comparison is exposed as
`yLoopOk`, so a host can tell "the loop is circulating and the water is
appropriate" from "nobody is pumping and the rule has no opinion" — two readings
that both come out as `yFault = false`.

`persist` requires 60 continuous minutes and carries `delayOnInit = true`.
Persistence is continuous, not accumulated: a pickup that takes the pumps to 40%
for ten minutes discards the elapsed time rather than pausing it. The alarm
falls on the tick the condition ends, with no release delay, and `TrueDelay`
asserts at exactly `T + delayTime`, so the realized test is "held for strictly
more than `alarm_delay`" at tick resolution.

## Possible Diagnoses

Authored for this card — PNNL-27338 publishes detection thresholds and no
diagnosis list, and the reference has no HW-0007 card to transcribe.

1. No HW supply temperature reset programmed — the common case and a remote fix.
   HW-0008 detects the same failure directly, at the setpoint
2. Reset programmed but its low end is too high. A schedule bottoming out at
   65 °C looks correct in the sequence and behaves nearly as badly in the mild
   hours; this is the diagnosis that survives a casual review of the controls
3. Reset disabled or overridden during a cold snap and never released
4. Boiler minimum-temperature protection setting the floor. Where that limit is
   what stops the reset going lower the finding is real but the fix is a
   blending valve, a buffer arrangement or a condensing retrofit
5. Boiler-local control ignoring the BAS — the aquastat runs its own high limit
   and the reset never reaches the fire. Cross-check `hws_temp` against
   `hws_temp_sp`; this is the one case where HW-0008 stays quiet while this
   rule fires
6. Emitters that genuinely need the temperature. An old cast-iron system sized
   for 82 °C water may be operating as designed — a plant-design finding, and
   the reason the threshold is site configuration rather than law

## Energy Impact

EXCESS_CONSUMPTION, MEDIUM confidence, PROXY_ESTIMATION. The hot water playbook
puts HW supply-temperature reset and DP reset together at 1–3% of site energy
from PNNL-27338's measure set, with no published split and no estimator that
converts a flagged hour to kilowatts. The mechanism is definite: standing loss
scales with how much hotter the water is than its surroundings, and the
boiler-side term is small on a non-condensing machine and large on a condensing
one. Confidence is MEDIUM because of the threshold rather than the measurement —
the rule does not know the plant's design supply temperature, and 54.4 °C at low
load is a finding on modern coils and Tuesday on a 1950s radiator system.
Heating-dominant, weighted to the mild hours where most heating hours live.

## Emissions Impact

Scope 1, PROXY_EMISSIONS, MEDIUM confidence. The waste is fuel, not electricity:
the pump is doing useful work at whatever speed the loop needs, and what is
thrown away is the extra fuel burned to hold water hotter than the building
asked for, plus what leaks out of the pipe. A decarbonised grid does not touch
it, and the condensing case is worth separating in a report — a plant holding
its return above the dew point pays a combustion penalty on every therm. No
factor is applied here; the host multiplies whatever fuel figure it can bound
(see `runtime_estimation`) by a static combustion factor, as HW-0003 does.

## Deviations

- **This rule is a library extension, not a transcription.** Ch.14 specifies
  HW-0001, 051 and 052 and publishes no card, tunables table or vectors for
  anything resembling this one. The detection logic and both published
  thresholds are paraphrased from PNNL-27338 §4.4.2 (Katipamula et al. 2018);
  `name`, `severity: 3` and `method: rule` are `faults/hw/README.md`'s.
- **130 °F ships as 54.4 °C, 0.044 K below the exact conversion.** Shipping
  54.444… would put a fifteen-digit literal in the graph to express a number the
  source stated to three significant figures. The shipped value is very slightly
  more sensitive than PNNL's, by an amount an order of magnitude finer than any
  hot water sensor a building owns, and the parameter is per-plant
  configuration in any case.
- **`min_pump_speed_for_eval` is ADOPTED, and it is the largest departure from
  the source.** PNNL's algorithm is two conditions with no plant-running gate,
  which read literally is unsound on the low side: a stopped pump reads 0%, the
  lightest possible load, while boiler water stays hot for hours — every
  overnight setback would report the fault. PNNL gates every algorithm on data
  sufficiency (§1.2/§2.1), the same class of concern SCHEMA.md routes to
  preconditions and evaluability outputs, but nothing in §4.4.2 excludes a
  stopped pump. Closed here as a conjunct that is also exposed as `yLoopOk`;
  precedent is CHW-0004's `yLoadOk` and VFD-0001's `yCmdOk`.
- **The floor ships at 10%, well below the fault threshold, on purpose.** Its
  only job is to exclude a pump that is stopped or barely turning; the band
  between it and `low_load_speed_threshold` is where the fault lives, and a pump
  at 15% is genuinely circulating and genuinely lightly loaded. VFD-0001 sets
  its analogous floor at 20% because it asks a harder question of the signal.
  Note the direction before retuning: raising this number narrows the fault band,
  and raising it above 35% deletes the rule.
- **`alarm_delay` is ADOPTED at 60 minutes.** PNNL publishes no fault
  persistence for this measure, only a 15–60 minute rolling average. Sixty is
  the top of that range and the library's standing plant-rule persistence
  (HW-0003, CHW-0004), and it rides out the transient that would otherwise
  dominate: morning warm-up, when the loop is hot and the pumps have not ramped
  because the coil valves are still opening.
- **Continuous persistence replaces PNNL's window average.** The reference
  averages both signals across the window; this rule requires the conjunction on
  every tick for the full hour. For a steady condition — what a missing reset
  produces — the two agree; they differ on intermittency, and this one is
  stricter, so a loop alternating 20 minutes hot-and-loafing with 20 minutes busy
  never alarms though its window average might trip. CHW-0004 made the same
  trade. `Reals.MovingAverage` is not the alternative: its fixed 64-checkpoint
  ring needs a tick of at least window/63 before it silently drops samples.
- **Pump speed alone is the load proxy; no OAT conjunct.** PNNL's neighbouring
  high-DP measure (§4.2.2, this library's HW-0005) crosses pump speed with
  mild outdoor air and §4.4.2 deliberately does not — the pump speed *is* the
  load measurement, and a lightly loaded building on a cold, internally-driven
  afternoon is as valid a finding as one in April. An OAT gate would narrow the
  rule to the swing seasons and make two hot water cards answer nearly the same
  question. HW-0003 needs warm weather because it asks whether the plant should
  run at all; this one needs light load because it asks what temperature the
  running plant should make.
- **Strict comparisons on all three thresholds.** CDL `Reals` has no `LessEqual`
  or `GreaterEqual` in any case, so a pump at exactly 35.0%, water at exactly
  54.4 °C and a pump at exactly 10.0% all fall on the no-fault, no-eval side.
  Equality is measure-zero in continuous data and perfectly reachable in a BAS
  that scales drive feedback to whole percent, so each boundary carries an
  on-the-line vector plus one a tenth of a unit either side. Both the vector and
  the parameter spell 54.4, so the temperature boundary is decided by the
  strictness rather than by rounding.
- `persist.delayOnInit = true` (CDL default `false`), the library's standing
  choice: a plant already loafing with hot water at controller restart waits out
  the full hour rather than alarming on the first tick.
- **No suppression contract with HW-0008, deliberately.** The two rules see one
  failure from opposite ends, and the tempting move is to suppress this one. The
  findings are separable in both directions — a reset that moves correctly but
  bottoms out at 65 °C fires this rule and not HW-0008, and a flat setpoint
  parked at 50 °C fires HW-0008 and never trips 54.4 °C here — and suppression
  is a two-card contract. They are `related` and belong in one visit.
- **`related` spans the family and is asserted here first.**
  `faults/hw/README.md`'s Relationships section documents HW-0001/HW-0002/HW-0003 only
  and is another writer's. One entry needs its sign stated: HW-0004 (low loop
  delta-T) can point the opposite way, since hotter supply water at a given
  return temperature *raises* delta-T, so a plant showing both has two causes
  rather than one.
- **`clusters: []`.** CLU-02 ("Missing Reset Strategy") is the syndrome this
  fault belongs to and already carries the air-side consequence rule AHU-0031,
  this card's structural analog one system over. But the cluster is AHU-scoped
  today, membership is `clusters/clusters.json`'s to declare, and the natural
  HW-side trigger is HW-0008 rather than this rule.
- **`playbooks: [hot-water-plant-faults]`.** Its Applies-To row names only
  HW-0001/HW-0002/HW-0003 — the index owner's line to extend — but Step 1 already
  carries this fault's remedy in full, including the OAT-based reset schedule,
  because it was written from the same PNNL measure set. `missing-reset` is the
  near miss: its procedure is SAT and DSP plots throughout, and the card that
  should claim it on the hot water side is HW-0008.
- **Blind spots, in the order they will bite.** A constant-volume loop has no
  speed signal and the rule does not apply — bind nothing rather than binding a
  run status scaled to 100. A loop whose DP setpoint never resets holds its pumps
  fast at low load, which suppresses this rule silently; that miss is
  HW-0005/HW-0006's finding. The rule sees water, not setpoints, so it cannot
  separate "the reset never ran" from "the boiler ignored it" — diagnosis 5,
  which needs `hws_temp_sp`. And nothing guards against a supply sensor reading
  high: a 3 K offset moves this decision by more than half the distance between
  a well-reset plant and a flagged one.
- **No published test vectors.** PNNL-27338 publishes none for its hot water
  measures and the reference has no card for this fault, so every scenario in
  `vectors.json` is authored from the equation and replayed against the pinned
  engine rev.

## Notes

Read `yLoopOk` before `yFault`. A plant on setback, a summer weekend, or a loop
whose pumps have stopped all hold it false, and every `yFault = false` underneath
means "not evaluated" rather than "the water is at the right temperature".

Trend `hws_temp` against `hws_temp_sp` and outdoor air for a week before
dispatching anyone. A supply temperature tracking a flat setpoint is diagnosis 1
or 2 and the fix is written from a desk; a setpoint that resets while the water
does not follow is diagnosis 5, at the boiler's own controller; a floor no
schedule explains is diagnosis 4, a plant-design conversation. Only then ask
whether the emitters can accept cooler water — diagnosis 6, the one case where
the right outcome is to retune `high_hws_temp_threshold` and close the finding.
Where both fire, HW-0008 is the one to fix: it names the cause and this rule
measures the consequence.
