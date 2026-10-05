---
schema: cxf-library/fault-card/v1
id: TOWER-0005
name: Condenser water overcooling with fan energy
equipment: tower
status: verified
phase: 2
method: rule
severity: 3
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "NREL, Refrigeration Playbook: Natural Refrigerants (2021), cooling-tower control discussion near p.45 — variable-speed fans seek leaving-water setpoint and turn off when leaving water is below setpoint; mechanism only"
  - "EnergyPlus Engineering Reference, Cooling Towers and Evaporative Fluid Coolers — fan-off free convection and fan modulation/cycling are normal ways to meet tower outlet setpoint; simulation semantics only"
  - "Library-authored thresholds and persistence; no cited source publishes 1 K, 30%, or 600 s as portable fault limits"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [TOWER-0001, TOWER-0002, TOWER-0004, CHW-0005]
playbooks: [cooling-tower-performance]
operating_states: "Tower cell enabled for normal mechanical heat rejection with an active setpoint, proven fan operation, and normal automatic isolation/bypass control"
preconditions: "Bind tower_leaving_temp to the cold water leaving this tower/cell and tower_leaving_temp_sp to the active target for that same stream; the latter is commonly named condenser-water supply or entering-chiller setpoint. Parallel cells may share a common setpoint only when that target truly applies to each cell and per-cell outlet sensing exists. Fan status and speed must describe the same cell: status is independent run proof, while speed is preferably feedback. A verified command proxy is allowed only with a documented limitation and must not be used where local control, current limiting, or drive overrides can separate command from delivered airflow. Configure minimum_fan_speed from the same-cell feedback/power relationship and effective drive minimum before adoption; the shipped 30% is NO_PORTABLE_DEFAULT. Points must be fresh and time-aligned. Exclude waterside economizer/free cooling, thermal-storage charging, emergency heat rejection, mandated low-condenser-water modes, startup/shutdown transients, drain-down, manual/local operation, and abnormal isolation or bypass states. Sensor calibration and topology must be valid; otherwise report NO_EVAL."
points:
  - tower_leaving_temp
  - tower_leaving_temp_sp
  - tower_fan_status
  - tower_fan_speed
outputs:
  - name: yFault
    description: True after material overcooling and proven loaded fan operation persist continuously for sustained_duration
  - name: yOvercooled
    description: True while active tower-leaving setpoint exceeds measured leaving temperature by strictly more than overcooling_allowance
  - name: yFanLoaded
    description: True while this fan is independently proven on and speed is strictly above minimum_fan_speed
params:
  overcooling_allowance:
    default: 1.0
    unit: K
    description: "Allowed tower-leaving temperature undershoot. ADOPTED_TUNABLE: set outside combined sensor, setpoint-distribution, and control-loop uncertainty."
    cxf: overcooled.t
  minimum_fan_speed:
    default: 30.0
    unit: "%"
    description: "Speed above which fan use is considered materially loaded. NO_PORTABLE_DEFAULT: 30% is an adoption-blocking placeholder; configure from measured same-cell fan feedback/power and the drive's effective minimum. The strict graph comparison leaves exactly 30% clear."
    cxf: speedHigh.t
  sustained_duration:
    default: 600.0
    unit: s
    description: "Continuous overcooling with loaded fan required before alarm. ADOPTED_TUNABLE: longer than ordinary setpoint and cell-stage response."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Cooling-tower fan electricity and, secondarily, condenser-water/chiller control
  savings_range: "Site-specific; the defensible upper bound is measured tower-fan kW during yFault, not a portable percentage"
  climate_sensitivity: cooling-dominant
  runtime_estimation: "upper_bound_waste_kwh = same-cell fan kW integrated only while yFault is true. Refine against the fan power needed after setpoint/sequence correction; the graph itself reads no power."
emissions:
  scope: "2"
  method: PROXY_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, July week, two parallel variable-speed tower objects, plant mode at 60 s"
    scenarios: 2
    failures: 0
    notes: "7,610 loaded/settled evaluated ticks across 16 windows after an 1800 s lead; per-object outlet temperature is compared with the shared condenser-loop target, positive fan electricity is run proof, and Air Flow Rate Ratio x100 is an effective-airflow proxy rather than mechanical VFD feedback. January had zero evaluable loaded windows and is not counted"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:b4c4b0066b62393e887a37208afffd2e"
  date: 2026-10-05
---

## Description

A healthy variable-speed tower reduces or stops fan work when leaving water is
already colder than its active target. This rule finds the narrower waste case:
the water is materially below setpoint while the same cell is proven running
above a meaningful fan speed. Cold water with the fan off remains explicitly
silent; natural convection is normal operation, not a fault.

## Detection Logic

```
temperature_error = tower_leaving_temp_sp - tower_leaving_temp
overcooled         = temperature_error > overcooling_allowance
fan_loaded         = tower_fan_status AND tower_fan_speed > minimum_fan_speed
candidate          = overcooled AND fan_loaded

yOvercooled = overcooled
yFanLoaded  = fan_loaded
yFault      = candidate sustained for sustained_duration
```

![TOWER-0005 block graph](diagram.svg)

The algebra avoids a second subtract block: setpoint minus measured leaving
temperature is positive when the tower overcools. Both comparisons are strict,
so exactly 1 K of undershoot or exactly 30% fan speed remains clear.
`TrueDelay(delayOnInit=true)` resets immediately if temperature recovers, fan
proof drops, or speed unloads.

## Possible Diagnoses

1. Leaving-water setpoint is reset upward but the tower sequence still follows
   a stale, local, or differently scaled setpoint
2. Fan minimum speed, VFD floor, or cell-stage deadband is too high
3. Fan is in hand/local, speed command is overridden, or drive feedback is wrong
4. Isolation or bypass valve is stuck/mis-sequenced, making the sensed stream
   unrepresentative of the controlled common header
5. Leaving-water sensor reads low or active setpoint is mapped to the wrong node
6. Cell lead/lag logic keeps too many fans loaded after the heat-rejection load falls
7. A legitimate low-water strategy was not excluded by the host

## Energy Impact

EXCESS_CONSUMPTION with MEDIUM confidence. The direct waste is tower-fan
electricity during the persisted condition. Integrating measured same-cell fan
power gives a conservative upper bound, not guaranteed savings: the corrected
sequence may still require some fan work. Chiller energy can move in either
direction with condenser-water temperature and must not be added without a
site-specific optimum-lift model.

## Emissions Impact

Scope 2 from avoidable fan electricity, using the site's marginal operating
emissions rate. No portable savings or emissions fraction is claimed.

## Deviations

- **No source establishes the three numeric defaults.** The 1 K allowance and
  10-minute persistence are adopted commissioning starts. The 30% loaded floor
  is `NO_PORTABLE_DEFAULT`: configure it from same-cell feedback/power before
  adoption because a speed percentage is not a universal meaningful-energy line.
- **The 1 K allowance must exceed measurement uncertainty.** Common-header
  setpoints compared with individual-cell outlet temperatures can create a
  false undershoot on parallel towers; topology and setpoint distribution are
  preconditions, not hidden graph assumptions.
- **Speed feedback is preferred.** A verified command proxy is weaker because
  a drive in local, limited, or overridden operation can deliver something
  else. The host must label that binding and avoid near-threshold conclusions.
- **This rule stays outside CLU-10.** It is a control/energy finding, not the
  condenser-side degradation syndrome triggered by TOWER-0001.
- **No suppression is encoded.** TOWER-0004 failure-to-start can invalidate the
  fan premise, while its unexpected-run direction can cause this exact fault;
  whole-rule suppression would erase a useful diagnosis.
- **Simulation validates healthy FPR only, not causal TPR.** The recorded Denver
  July campaign uses native per-object outlet temperature, fan power, and
  airflow ratio at 60 s. Airflow ratio is an effective-airflow proxy rather
  than mechanical VFD feedback, and the parallel towers share one loop target.

## Notes

Read both diagnostic outputs before dispatch. `yOvercooled=true` with
`yFanLoaded=false` is often normal free convection. `yFanLoaded=true` without
overcooling says the fan may simply be doing useful work.
