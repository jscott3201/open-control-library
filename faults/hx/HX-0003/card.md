---
schema: cxf-library/fault-card/v1
id: HX-0003
name: Heat transfer persists with control valve commanded closed
equipment: hx
status: verified
phase: 2
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: MEDIUM
estimation_method: DIRECT_MEASUREMENT
source:
  - "EnergyPlus 25.1 Engineering Reference, Heat Exchangers — signed heat transfer from both capacity rates and inlet temperatures: https://bigladdersoftware.com/epx/docs/25-1/engineering-reference/heat-exchangers.html"
  - "DOE FEMP, Energy Management Information System Capabilities — monitoring reduced/changed HX heat transfer from temperature evidence for condition-based maintenance"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [HX-0001, HX-0002]
playbooks: [hydronic-heat-exchanger-faults]
operating_states: "The HX's intended isolating valve is finally commanded closed, residual transport/thermal soak has expired, and validated branch measurements can establish signed heat transfer"
preconditions: "The named valve must be intended to isolate the entire monitored HX exchange path; its point is the final physical output after minimum-position, exercise, local/HAND, freeze, and protective logic. control_valve_cmd and the four temperatures/two flows behind heat_transfer_rate must share one HX scope and aligned timestamps. The host publishes signed transfer only after positive finite flow/capacity, fluid-property, point-quality, and side-energy-balance checks; invalid derivation means NO_EVAL. Exclude a site-commissioned transport/thermal-soak interval after closure and after direction, setpoint, pump, or valve changes. Commission the transfer limit above the closed/no-load uncertainty envelope. Natural circulation, a manual/parallel bypass, and a failed check valve remain valid unintended paths rather than suppression reasons. Passive exchangers without an isolating valve are excluded."
points:
  - control_valve_cmd
  - heat_transfer_rate
outputs:
  - name: yFault
    description: True after absolute heat transfer remains above the commissioned no-load limit with the isolating valve command below its closed limit for alarm_delay
  - name: yValveClosed
    description: Immediate diagnostic sub-condition; false never means NO_EVAL
  - name: yTransferPresent
    description: Immediate direction-independent diagnostic sub-condition based on absolute signed transfer; false never means NO_EVAL
params:
  closed_command_limit:
    default: 5.0
    unit: "%"
    description: "ADOPTED_TUNABLE library valve-closed starting point. The strict comparator treats exactly 5% as not closed; align with actuator scaling and any intentional minimum position."
    cxf: valveClosed.t
  unexpected_transfer_limit:
    default: 5.0
    unit: kW
    description: "NO_PORTABLE_DEFAULT executable placeholder. Commission above reconciled closed/no-load transfer uncertainty, meter resolution, and residual-loss envelope, and below the minimum actionable unintended exchange."
    cxf: transferHigh.t
  alarm_delay:
    default: 900.0
    unit: s
    description: "ADOPTED_TUNABLE 15-minute persistence after the separate host soak/settling exclusion. It is not a substitute for determining the installation's water transport and metal/pipe thermal time constant."
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: Unwanted hydronic transfer plus source/load plant and pumping needed to create it
  savings_range: Site-specific; proportional to validated abs(heat_transfer_rate) and fault duration when the transfer is truly unwanted
  climate_sensitivity: both
  runtime_estimation: "waste_kwh_thermal = integral(max(abs(heat_transfer_rate) - commissioned_no_load_rate, 0)) while yFault is active; convert to fuel/electricity with the actual marginal source efficiency/COP"
emissions:
  scope: "1+2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:aff5ae932f23f7127de8a90723fadb04"
  date: 2026-10-05
---

## Description

When an HX isolation/control valve is finally commanded closed, meaningful
continued transfer indicates an unintended hydraulic/thermal path. The valve
may be passing, never have reached its seat, leave a bypass open, or permit
gravity circulation. This rule treats both heating and cooling directions the
same by comparing the absolute value of a validated signed heat-transfer rate.

The observation is broader than "leaking valve." Position feedback, branch
flow, check-valve state, and a piping walk-down distinguish actuator failure,
seat leakage, bypass, and thermosiphon after the alarm.

## Detection Logic

```text
yValveClosed = control_valve_cmd < closed_command_limit
yTransferPresent = abs(heat_transfer_rate) > unexpected_transfer_limit
yFault = (yValveClosed AND yTransferPresent) continuously for alarm_delay
```

![HX-0003 block graph](diagram.svg)

The two sub-condition outputs are immediate diagnostics. `TrueDelay` uses
`delayOnInit = true`, but its 15 minutes do not replace the host's independent
post-close soak exclusion. Both comparisons are strict: exactly 5% is not
closed at the shipped setting and exactly 5 kW is not transfer-present.

## Possible Diagnoses

1. Passing valve seat, debris, erosion, or insufficient close-off rating.
2. Actuator/linkage failed or commanded scaling does not reach physical close.
3. Manual bypass, three-way/parallel path, or wrong valve bound to the rule.
4. Failed/missing check valve or gravity/thermosiphon circulation.
5. Residual transport/metal/pipe soak not actually expired.
6. Flow/temperature bias, fluid-property error, time skew, or energy imbalance.

## Energy Impact

CRITICAL_WASTE with DIRECT_MEASUREMENT and MEDIUM confidence. Once the host has
validated signed kW and confirmed the transfer is unwanted, thermal waste is
the integral above the commissioned no-load envelope. Source energy depends on
the boiler/chiller/heat-pump/district efficiency and concurrent pumping.

## Emissions Impact

Scope 1+2, PROXY_EMISSIONS. Apply actual marginal source efficiency/COP and
emissions factors to the validated unwanted thermal energy; direction alone
does not identify the fuel/electric split.

## Deviations

- **The rule says transfer, not valve leakage.** A closed command plus transfer
  cannot uniquely identify the path. Natural circulation and a bypass are real
  findings with different repairs, kept explicit in diagnosis.
- **Heat transfer is host-derived.** The graph applies `Abs` only after the host
  validates two side estimates, safe finite capacity rates, and alignment. It
  does not derive kW from unguarded divisions.
- **5 kW has no portable authority.** It is an executable fixture. Commission a
  no-load uncertainty envelope before enabling the rule.
- **Persistence is not soak.** A host exclusion restarts after closure and every
  material hydraulic/thermal discontinuity; otherwise a long normal cooldown
  can consume the timer and manufacture a finding.
- **Optional position feedback stays diagnostic.** Requiring it would sharply
  reduce deployability, and command/position disagreement is a distinct future
  rule. Use it in the playbook when available.
- **No cluster/suppression.** HX-0001 may co-occur, but neither verdict
  universally invalidates or causally owns the other.
