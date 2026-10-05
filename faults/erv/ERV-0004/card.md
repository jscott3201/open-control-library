---
schema: cxf-library/fault-card/v1
id: ERV-0004
name: Recovery device proof-of-operation failure
equipment: erv
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: HIGH
estimation_method: PROXY_ESTIMATION
source:
  - "Library-authored active-recovery application of the command/status proof family established by AHU-0039, PMP-0003, and HW-0009"
  - "Buildings.Controls.OBC.CDL.Logical.Proof (Hu & Wetter, March 2023), available at engine pin e2ff2f8; the library uses the proven two-direction vocabulary but composes independent timers because the packaged block cannot express this card's required behavior"
  - "points/erv.points.json erv_recovery_cmd and erv_recovery_status — final-command and independent-physical-proof contracts for wheels, runaround pumps, and other active recovery devices"
  - "Greenheck ERV controller IOM 484118, pp.5 and 11 — the controller I/O identifies a wheel-rotation alarm and the menu documents automatic wheel jog; supports available independent proof and the final-command/jog binding caveat, not the 120 s defaults"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: [ERV-0001, ERV-0003, ERV-0005]
playbooks: [proof-of-operation, erv-effectiveness]
operating_states: "all operating states of an active recovery device; both mismatch directions remain meaningful whenever the final command and independent proof are available"
preconditions: "Applicable only to an active recovery component with a final run command and independent physical proof: a rotary wheel drive, runaround-loop pump, or equivalent. Passive fixed-plate cores are excluded. Both points must belong to the same component, be fresh, and arrive faster than the configured proof windows. Bind erv_recovery_cmd after frost, bypass, smoke, safety, automatic wheel-jog/exercise, and local sequence logic; an upstream ERV enable is not the same command. If jog is not represented in the final command, exclude its intervals host-side. erv_recovery_status must prove rotation/work (wheel speed/rotation switch, pump current/flow, or equivalent), not echo the command relay. Configure start_proof_time above the slowest legitimate acceleration and delivery delay and stop_proof_time above wheel coast-down. Exclude maintenance and local/hand testing at the host."
points:
  - erv_recovery_cmd
  - erv_recovery_status
outputs:
  - name: yFault
    description: True while either independently timed command/status mismatch direction is active
  - name: yFailToStart
    description: Diagnostic flag — command true and independent status false continuously for start_proof_time
  - name: yUnexpectedRun
    description: Diagnostic flag — status true and command false continuously for stop_proof_time
params:
  start_proof_time:
    default: 120.0
    unit: s
    description: "ADOPTED_TUNABLE: maximum allowed time from final run command to independent proof (2 min). Set above wheel/pump acceleration plus worst-case point-delivery latency; this is not a manufacturer-universal limit."
    cxf: startHeld.delayTime
  stop_proof_time:
    default: 120.0
    unit: s
    description: "ADOPTED_TUNABLE: maximum allowed time for proof to remain after command-off (2 min). Set above legitimate wheel coast-down or runaround-pump proof decay and point-delivery latency."
    cxf: stopHeld.delayTime
energy_impact:
  affected_subsystem: "Active recovery drive and the downstream heating/cooling load recovery should offset"
  savings_range: "Direction-dependent: fail-to-start loses recovery and may threaten frost operation; unexpected run consumes the drive/pump power and may recover energy when bypass was intended."
  climate_sensitivity: both
  runtime_estimation: "PROXY: unexpected_run_kwh = active_device_kw × yUnexpectedRun_hours. Fail-to-start recovery loss requires airflow, temperatures, and expected effectiveness from the host; the boolean pair alone cannot quantify it."
emissions:
  scope: "1+2"
  method: PROXY_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:0c006939620ca8d334e3d521c711d2cc"
  date: 2026-10-05
---

## Description

An active recovery device has to do more than receive an enable. This rule
compares the final command reaching the wheel drive or runaround pump with an
independent indication that the component actually operates. It detects both a
device that fails to start and one that continues running after its command is
removed. Passive plate cores have no command/status pair and are outside scope.

## Detection Logic

```text
yFailToStart   = (erv_recovery_cmd AND NOT erv_recovery_status)
                 sustained for start_proof_time
yUnexpectedRun = (erv_recovery_status AND NOT erv_recovery_cmd)
                 sustained for stop_proof_time

yFault = yFailToStart OR yUnexpectedRun
```

Block graph (`rule.cxf.jsonld`):

![ERV-0004 block graph](diagram.svg)

Each direction owns its delay, and both delays use `delayOnInit = true`. A
direction flip clears the old flag immediately and starts the other timer from
zero; the two diagnostic outputs cannot overlap because their command terms are
opposites. Agreement clears without an off-delay.

## Possible Diagnoses

`yFailToStart`:

1. Broken wheel belt/coupling, stalled motor, tripped overload, or failed drive
2. Runaround pump locked out, isolated, air-bound, or mechanically failed
3. Status switch/threshold failed even though the device operates
4. Final command point not reaching the starter or drive

`yUnexpectedRun`:

5. HOA/local switch left in HAND or drive left in local mode
6. Software override, welded contactor, or a second controller still commanding
7. Status point stuck true or sourced from the wrong component

## Energy Impact

PROTECTIVE, HIGH confidence, PROXY_ESTIMATION. A fail-to-start can remove most
of the intended heat recovery while downstream coils silently make up the load;
an unexpected run adds motor/pump power and can oppose bypass or frost intent.
Duration is known, but power and lost recovered heat are host-side quantities.

## Emissions Impact

Scope 1 + 2, PROXY_EMISSIONS. Device power is usually scope 2. Lost recovery
shifts load to electric cooling and either electric or fuel heating, so the
emissions scope follows the downstream plant and operating season.

## Deviations

- **Library-authored for active recovery only.** The HVAC FDD Reference has no
  ERV proof card; this is the established AHU/pump/boiler topology applied only
  where a real command and independent proof exist.
- **Both proof times are 120 s ADOPTED_TUNABLE defaults.** No cited source gives
  one portable wheel/runaround value; separate parameters preserve legitimate
  acceleration and coast-down differences.
- **Automatic jog belongs in the final command.** Greenheck documents wheel jog
  as normal control behavior; binding an upstream enable or excluding jog from
  the command would turn intentional operation into `yUnexpectedRun`.
- **Composed logic replaces `CDL.Logical.Proof`.** At engine pin `e2ff2f8`, that
  block has one timing contract and unsuitable initialization/chatter behavior;
  `Not` + `And` + two `TrueDelay`s preserves mutual exclusivity and full startup
  delays exactly like AHU-0039.
- **No evaluability output.** Evaluability is freshness, independence, and
  applicability of the two bindings, all host knowledge that cannot be derived
  from their boolean values.
- **No suppression.** A proof failure can cause ERV-0001's real effectiveness
  loss, and a wheel can fail while airflow stays balanced; operators benefit
  from both findings.
- **No ERV cluster.** The shared order belongs in `erv-effectiveness` and
  `proof-of-operation`; a three-rule delivery batch is not a causal taxonomy.
- **No empirical validation claim.** Command and independent wheel/pump proof
  are not available in the current simulation mapping; synthetic vectors cover
  all truth-table and timing behavior.

## Notes

Command echo is the dangerous binding: it agrees perfectly with the command
even when a belt is broken or a pump is seized. If the two points share the same
controller object or relay source, omit the rule until independent proof exists.
