---
schema: cxf-library/fault-card/v1
id: HP-0008
name: Auxiliary heat active above lockout with compressor running
equipment: hp
status: verified
phase: 2
method: rule
severity: 2
category: EXCESS_CONSUMPTION
confidence: MEDIUM
estimation_method: PROXY_ESTIMATION
source:
  - "Winkler and Ramaraj, Field Validation of Air-Source Heat Pumps for Cold Climates, NREL/TP-5500-84745 (2023), Table 5 and note b, p.15 — auxiliary lockout is the maximum OAT for auxiliary operation; observed site values vary"
  - "NREL HPXML Data Dictionary v4.0, BackupHeatingLockoutTemperature — backup is disabled above the configured temperature and dual-fuel uses BackupHeatingSwitchoverTemperature"
  - "Library-authored executable combination of heating mode, configured OAT lockout, concurrent compressor/auxiliary proof, defrost exclusion, and persistence"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HP-0001, HP-0002, HP-0007]
playbooks: [heat-pump-faults]
operating_states: "normal automatic heat-pump heating when the configured strategy prohibits auxiliary heat above its site-specific lockout while the compressor runs"
preconditions: "aux_heat_status must independently prove active heat production by an explicitly classified auxiliary electric, gas, or other source; do not bind demand, command echo, availability, crankcase heat, base-pan heat, or defrost heat. The 5 degC placeholder is adoption-blocking: replace it with the installed balance/lockout strategy for climate, tariff, and equipment. mode_command code 1 must mean heating; comp_status must prove the same applicable compressor scope. OAT must be valid and representative. Exclude defrost, emergency heat, compressor failure/lockout, demand response, commissioning, and explicit high-capacity recovery. On dual-fuel equipment instantiate only when simultaneous compressor/fuel operation above the configured switchover is prohibited. Unmet obligations are NO_EVAL, not healthy."
points:
  - aux_heat_status
  - oat
  - mode_command
  - defrost_status
  - comp_status
outputs:
  - name: yFault
    description: "True after heating mode, above-lockout OAT, concurrent auxiliary/compressor proof, and no defrost persist for sustained_duration"
  - name: yHeatingMode
    description: "Diagnostic sub-condition flag; true when mode_command equals heating_mode_code. False never means NO_EVAL"
  - name: yAboveLockout
    description: "Diagnostic sub-condition flag; true when OAT is strictly above aux_heat_lockout_oat. False never means NO_EVAL"
  - name: yConcurrentHeat
    description: "Diagnostic sub-condition flag; true when auxiliary heat and compressor proofs are both active. False never means NO_EVAL"
params:
  heating_mode_code:
    default: 1
    unit: "1"
    description: "LIBRARY_PRECEDENT host-mapped heating-mode integer. Rebind when the site enumeration differs."
    cxf: kHeat.k
  aux_heat_lockout_oat:
    default: 5.0
    unit: "°C"
    description: "NO_PORTABLE_DEFAULT and adoption-blocking placeholder. Replace with the installed unit's commissioned auxiliary lockout or dual-fuel switchover strategy; equality is clear."
    cxf: aboveLockout.t
  sustained_duration:
    default: 300.0
    unit: s
    description: "ADOPTED_TUNABLE continuous signature duration intended to reject brief staging transitions; commission against the OEM sequence."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Heat-pump compressor and auxiliary heating source
  savings_range: "Potentially material when resistance or fuel backup runs unnecessarily; site- and stage-dependent"
  climate_sensitivity: heating-dominant
  runtime_estimation: "PROXY_ESTIMATION only when active auxiliary stage kW or fuel input is known: fault hours x verified stage input. Do not count required supplemental capacity as avoidable."
emissions:
  scope: "1/2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:8917cda230eb3df1fff05a46442b7cf9"
  date: 2026-10-05
---

## Description

This rule detects a proven auxiliary heat source operating concurrently with
the heat-pump compressor above the configured outdoor lockout in heating mode,
outside defrost. It targets prohibited concurrent operation, not emergency
heat, required low-temperature supplementation, or balance-point optimization.

## Detection Logic

```
heating_mode   = mode_command == heating_mode_code
above_lockout  = oat > aux_heat_lockout_oat
concurrent_heat = aux_heat_status AND comp_status
candidate = heating_mode AND above_lockout AND concurrent_heat
            AND NOT defrost_status

yHeatingMode = heating_mode
yAboveLockout = above_lockout
yConcurrentHeat = concurrent_heat
yFault = candidate sustained for sustained_duration
```

![HP-0008 block graph](diagram.svg)

The OAT comparison is strict. Diagnostic outputs are immediate;
`TrueDelay(delayOnInit=true)` applies only to the complete signature. Any
false candidate subcondition resets timing and clears a mature alarm.

## Possible Diagnoses

1. Auxiliary lockout or dual-fuel switchover misconfigured or disabled
2. Staging/thermostat sequence energizes backup heat too early
3. OAT sensor bias, bad location, stale value, or wrong unit conversion
4. Auxiliary proof bound to command, availability, or a non-space-heating load
5. Compressor-capacity fault causing an authorized recovery mode not host-gated
6. Defrost, emergency, demand-response, or commissioning state omitted from gating

## Energy Impact

Unnecessary resistance or fuel backup can displace more efficient compressor
heating. Estimate only from verified auxiliary stage input during fault hours,
after proving that the installed sequence did not require the extra capacity.

## Emissions Impact

Scope 2 applies to electric auxiliary heat and compressor power; Scope 1 can
apply to fuel backup. Use measured or nameplate stage input and appropriate
time-varying electricity or fuel emissions factors.

## Deviations

- **5 °C is not a portable default.** It is an adoption-blocking placeholder;
  the applicable value is the installed balance/lockout or switchover strategy.
- **Confidence is MEDIUM rather than the brief's proposed HIGH.** Auxiliary
  role/proof, dual-fuel behavior, site setpoint, and emergency/recovery modes
  must be established before concurrent operation is avoidable.
- **Auxiliary semantics are provisional.** Brick and 223 provide generic
  heating/status patterns but no exact auxiliary role; topology and independent
  proof must identify the real supplemental source.
- **Defrost is encoded; other exceptions are host gates.** Emergency, failure,
  demand-response, and recovery state semantics are site-specific.
- **No automatic suppression is added.** HP-0007 direction should lead diagnosis,
  but unexpected compressor proof can leave this concurrent-energy signature valid.
- **No empirical FPR or TPR is claimed.** Current datasets do not expose aligned
  auxiliary-production proof, defrost state, and the configured site lockout.
