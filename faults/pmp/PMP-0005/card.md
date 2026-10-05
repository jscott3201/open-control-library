---
schema: cxf-library/fault-card/v1
id: PMP-0005
name: Flow through stopped pump
equipment: pmp
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "DOE/Hydraulic Institute, Improving Pumping System Performance: A Sourcebook for Industry, 2nd ed., PDF p.9 / printed p.6 — discharge check valves prevent reversal while a pump is stopped"
  - "Library-authored signed branch-flow adaptation; no cited source publishes the shipped flow or persistence thresholds"
  - "Library precedents PMP-0003 (proof/status timing), SYS-0008 (mirrored signed direction outputs), and VFD-0003 (strict directional comparisons)"
g36: null
clusters: []
suppresses: []
suppressed_by: [PMP-0003]
related: [PMP-0001, PMP-0002, PMP-0003]
playbooks: [vfd-pump-faults]
operating_states: "all states in which this pump branch should be hydraulically isolated whenever pump_status is false"
preconditions: "pump_flow must be individual-branch flow for this exact pump; a common-header point cannot distinguish which branch is passing. Full direction semantics require signed flow with positive defined from suction to discharge. A nonnegative magnitude sensor can support yFault, but neither yForwardFlow nor yReverseFlow is then physically trustworthy and the host must mark both direction labels unavailable. The zero offset and uncertainty must be known, and stopped_flow_threshold must exceed them. pump_status must be fresh independent proof; an active same-pump PMP-0003 suppresses this verdict because the stopped/running premise is unreliable. Exclude intentional bypass/gravity paths, thermosiphon designs, maintenance flushing, free cooling, and approved parallel-pump transfer. When scope, proof, or operating intent is unknown the host reports NO_EVAL."
points:
  - pump_status
  - pump_flow
outputs:
  - name: yFault
    description: True while stopped-branch flow in either signed direction has persisted beyond sustained_duration
  - name: yForwardFlow
    description: Diagnostic direction flag — true while a stopped pump has signed positive suction-to-discharge flow above the allowance; unavailable on magnitude-only bindings and false is not an evaluability verdict
  - name: yReverseFlow
    description: Diagnostic direction flag — true while a stopped pump has signed negative flow beyond the allowance; unavailable on magnitude-only bindings
params:
  stopped_flow_threshold:
    default: 1.0
    unit: L/s
    description: "Absolute branch-flow allowance in either direction. NO_PORTABLE_DEFAULT: 1.0 L/s is an adoption-blocking placeholder that must be set above sensor noise/zero drift and against this branch's design flow."
    cxf: [forward.t, reverse.t]
  sustained_duration:
    default: 300.0
    unit: s
    description: "Continuous stopped-flow duration required before alarm. ADOPTED_TUNABLE; five minutes rejects valve-transfer and coast-down transients but is not a published universal value."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Pump branch, check valve, parallel header, and connected hydronic loop
  savings_range: "Site-dependent; unintended circulation can move heating/cooling and impose active-pump head while risking reverse rotation"
  climate_sensitivity: loop-dependent
  runtime_estimation: "QUALITATIVE_ONLY in-rule. A host may combine signed flow with loop temperatures and active-pump power, but this rule has neither thermal conditions nor pressure"
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
validation:
  - kind: simulation_fpr
    harness: simharness/v1
    date: 2026-08-20
    fleet: "EnergyPlus 25.1 OfficeLarge STD2019 Denver, one July + one January week, one individual HW pump, plant mode at 60 s"
    scenarios: 2
    failures: 0
    notes: "single RunPeriod with timeline/cadence validation; strictly positive pump active power is the disclosed status proxy and native mass-flow magnitude is converted at 997 kg/m3. Validates healthy yFault only, not signed direction or realistic reverse leakage"
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:d567fd02bea2ecc336bdf44f7680de4a"
  date: 2026-10-05
---

## Description

This rule detects material water flow through an individual pump branch while
that pump's independent run proof is false. Forward flow can indicate a passing
or missing check valve, parallel-header pressure, thermosiphoning, or bad proof;
reverse flow adds the risk of reverse rotation. The rule names the observed
hydraulic signature, not which component caused it.

## Detection Logic

```
forward         = pump_flow > stopped_flow_threshold
reverse         = -pump_flow > stopped_flow_threshold
yForwardFlow    = NOT pump_status AND forward
yReverseFlow    = NOT pump_status AND reverse
candidate       = yForwardFlow OR yReverseFlow
yFault           = candidate sustained for sustained_duration
```

![PMP-0005 block graph](diagram.svg)

Both comparisons are strict, so exactly ±1.0 L/s is clear at the defaults.
Direction outputs are raw diagnostic detail gated by stopped status; they are
not evaluability flags. `TrueDelay(delayOnInit=true)` applies to the OR of both
directions. A direct forward-to-reverse handoff therefore preserves the timer
because material stopped flow never ceased; an actual in-band interval resets
it, and recovery clears immediately.

## Possible Diagnoses

1. Passing, failed, reversed, or missing discharge check valve
2. Reverse flow driven by another pump on a common header
3. Thermosiphoning or a gravity path not represented in the operating gate
4. Pump run-proof failure or stale false status (PMP-0003)
5. Flow sensor zero error, sign inversion, or common-header misbinding
6. Isolation or bypass valve left open
7. Approved flushing, free-cooling, or transfer sequence not excluded

## Energy Impact

PROTECTIVE, MEDIUM confidence, QUALITATIVE_ONLY. Unintended branch flow can
waste active-pump head, transport unwanted heat, defeat staging, and rotate a
stopped pump backward. Magnitude depends on loop pressure and temperatures that
this graph does not consume.

## Emissions Impact

Scope 2, qualitative. Avoided electricity and thermal conditioning are
site-specific and require pressure, temperature, and active-equipment context.

## Deviations

- **The rule adds a signed convention without narrowing older consumers.**
  Positive is suction-to-discharge. Magnitude-only branch flow still supports
  `yFault` and PMP-0001/0002, but neither direction label; common-header flow is
  not a weaker proxy but the wrong measurement scope.
- **Both thresholds are site configured.** The 1.0 L/s and 300 s defaults are
  adopted executable placeholders, not manufacturer or standard limits.
- **Direction reversal does not reset persistence.** The timer watches absolute
  stopped-flow candidacy; continuous material flow remains one hydraulic event
  even when its sign changes. Vectors pin this explicitly.
- **PMP-0003 suppression is host-side and same-pump only.** A proof mismatch
  invalidates the stopped premise; the raw graph still alarms, preserving the
  evidence and avoiding command/status inputs that do not belong here.
- **No Pump Delivery Failure cluster is added.** Running-with-no-flow,
  deadheading, proof mismatch, and stopped-with-flow have incompatible premises
  and no shared trigger whose correction reliably clears all members.
