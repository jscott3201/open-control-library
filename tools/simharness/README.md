# simharness — EnergyPlus → vectors.json FPR harness

Replays a building's simulated week through the library's rules and counts
false positives. The building runs under its own schedule-based baseline
control (no RL, no external controller); `tools/verify` is the replay
engine, consuming generated `cxf-library/vectors/v1` scenarios whose only
expectation is `yFault == false` inside host-gated windows. Every FAIL is a
false positive to explain: a rule-robustness finding, a mapping-proxy
artifact, or a fault genuinely present in the model.

## Usage

```
export ENERGYPLUS_PATH=/path/to/EnergyPlus-25.1.0   # native install
python3 tools/simharness/harness.py run --building <dir> [--step-s 300]
```

`<dir>` holds `building.epjson` + a `*.epw` (the Building2Building dataset's
per-building layout; DOE prototype IDFs convert via `ConvertInputFormat`).
The harness patches the run period (default one July week) and
`Output:Variable` requests, runs EnergyPlus, extracts per-air-loop point
series, emits one replay dir per (rule × loop), and prints a per-rule
clean/FP table.

The harness selects exactly one `RunPeriod`, writes the requested EnergyPlus
`Timestep`, and validates the CSV's first timestamp, last timestamp, sample
count, and every interval against `--begin`, `--end`, and `--step-s` before
replay. A source without a `RunPeriod` receives one. The harness fails on
environment/date resets, cadence mismatch, a different requested period, or a
failed verifier—including `--reuse` against an older CSV. This is load-bearing
for timer rules: neither a July CSV relabeled as January nor the source model's
four timesteps/hour (900 s) relabeled as a 300 s replay is valid evidence. Use
60 s for TOWER-0005's 600 s persistence; 300 s remains the default for legacy
sweeps.

Generated replay directories are deliberately not publishable fault packages,
so the harness invokes `tools/verify --replay-only`: source packages still pass
the normal schema/content lint, while the harness process status reflects the
generated scenarios' replay result instead of expected missing-package lint.

## Point mapping (packaged VAV)

Per air loop, derived from the epJSON topology (see `harness.py` docstring
for the exact node→point table). Two mappings are **proxies** and any
conclusion drawn from them carries that caveat: `clg_vlv_cmd` ← DX cooling
runtime fraction × 100 (packaged units have no CHW valve), and
`oa_dmpr_cmd` ← realized OA flow fraction × 100 (not the damper command
signal).

## Host gating

The graphs compute fault-given-valid-data; NO_EVAL gating is the host's
job, so the harness implements it as expect-window construction:

- **Fan gate**: windows only inside contiguous fan-on intervals
  (fan power > 50 W), with a 3600 s lead margin so delay state accumulated
  while ungated clears — a real host suspends evaluation entirely.
- **Operating-state gate**: each card's `operating_states` frontmatter is
  transcribed into an OS set (`OS_MAP`); per-tick OS is derived
  APAR-style from actuator signatures (burner heating rate, DX runtime
  fraction with 30-min any-window smoothing over compressor cycling,
  E+ economizer status), and windows intersect the rule's set with an
  1800 s lead after each state entry. Vent-only operation (no heating, no
  cooling, no economizing) maps to no OS and is never evaluated.

Ungated replay is meaningless: the first run without OS gating produced
wall-to-wall "false positives" from OS#2-scoped rules correctly detecting
mechanical cooling at night — the gating is part of the deployment
contract, not a fudge.

## Fleet results (sweep v3, 2026-08-18)

**B2B OfficeMedium × 8 ASHRAE climate zones (1–8), one July + one January
week, 3 VAV loops each — 787 host-gated scenario-replays. 17 of 20
auto-eligible AHU rules fully clean.** Recorded on each swept card as a
`validation:` frontmatter block (SCHEMA.md contract). Remaining failures
are two explained findings, not noise:

1. **AHU-0006/AHU-0021 (31 events each, winter-dominant, all buildings)** —
   DCV-driven excess outdoor air: `Controller:MechanicalVentilation` holds
   ventilation flow while VAV turndown shrinks total flow, so OA fraction
   legitimately exceeds any fixed minimum (~86% observed at −7.8 °C OAT —
   freeze-stat territory in a real building). Conclusion: OA-fraction
   rules need a DCV-aware host precondition.
2. **AHU-0033 (8 events, winter only)** — the fleet publishes one
   constant cooling-oriented SAT setpoint, so heating-season tracking
   error is real per the rule's letter while the setpoint no longer means
   the active mode's target. Per-mode setpoint binding is a deployment
   requirement.

Sweep-methodology lessons baked into the harness: OS smoothing is
majority-of-window (an any-window smoother stretched compressor blips into
OS misclassification); heating takes outright precedence in OS derivation;
and an incomplete hand-maintained OS_MAP produced two spurious FP clusters
before being caught — the map should eventually be parsed from each card's
`operating_states` frontmatter (`OS#x` tokens) instead of hand-kept (TODO).

The B2B baseline also runs its fans 24/7 (no setback) — an after-hours
fault by construction; excluded from FPR claims, useful as a
known-positive. Reset-family rules are likewise excluded (constant-setpoint
baselines read "reset absent" by construction).

## TPR: sensor-bias campaigns (`--bias point=delta`, `--reuse`)

Injects a bias into the replayed inputs of an already-simulated healthy run
(the faulted-sensor-as-seen-by-FDD case; zero EnergyPlus time). FAILs are
DETECTIONS; attribution requires differencing against the healthy baseline
(rules that fail on the healthy run — the DCV pair — are confounded and
excluded). First campaign, +/-3 degC OAT bias (OfficeMedium-4004, July):
the envelope family and the direction-appropriate economizer rule catch the
bias in both directions — +3: AHU-0028 (3/3), 002 (3/3), 068 (3/3,
~2.3 h); -3: 051 (3/3, ~40 min), 003 (2/3), 062 (2/3) — while a dozen
non-OAT rules stay correctly silent. Empirical demonstration of the CLU-09
biased-OAT cascade; recorded as `simulation_tpr` validation blocks
(failures = missed detections).

## Tower groundwork (condenser-loop stats)

Plant mode also maps the condenser loop (tower-leaving/entering water
temperatures, per-tower fan power, EnergyPlus's native outdoor wet-bulb)
and emits `tower_stats.json` per run: approach (= leaving − wet-bulb) and
range percentiles over tower-on ticks. Four-climate OfficeLarge result —
healthy-operation approach p50/p95 by run: Miami Jul 1.6/2.3, Atlanta Jul
2.5/3.2, Tucson Jul 4.6/7.7, Buffalo Jul 4.8/8.5, Miami Jan 3.6/6.9,
Tucson Jan 11.5/13.3 °C. **Design consequence for the future TOWER
family: a fixed approach-high threshold is indefensible** — variable-speed
fans legitimately let approach ride high at part load — so the rule form
must gate on fan at/near full speed (approach high DESPITE maximum fan =
capability degradation: fouling, scale, airflow blockage). At design-like
conditions (Miami July, fans loaded) healthy p95 is ~2.3 °C, so a
fan-gated threshold of roughly 2× design approach is the starting band;
per-site commissioning still required.

## TPR: physics-level FaultModel campaigns (`--faultmodel kind=delta`)

Patches EnergyPlus `FaultModel:*` objects into the epJSON before
simulation, so the CONTROLLER acts on the faulted sensor while FDD replays
true node values — the complement of the `--bias` campaign (where FDD's
own input lies). First campaign, OAT sensor offset +/-4 degC on all OA
controllers (OfficeMedium-4004, July): +4 (economizer locks out early) is
caught by AHU-0017 (3/3 loops); -4 (economizes past true changeover) by
AHU-0034 (3/3, one loop in ~5 min); each direction's mirror rule and the
entire envelope family stay correctly silent, since every replayed sensor
is physically consistent. Where FDD and the controller share a sensor,
real deployments see BOTH signatures — the input-bias envelope detections
and the behavioral detections — which is what the cluster grouping is for.

The OS gating map is now parsed from each card's `operating_states`
frontmatter (`parse_operating_states`; ranges, comma lists, "all");
unparseable prose is a hard error requiring an explicit `OS_OVERRIDE`
entry — never a silent all-states default.

## Plant mode (DOE prototypes)

`--mode plant` maps CHW/HW plant loops instead of air loops: loop
supply/return node temperatures and setpoints from `PlantLoop`, chiller/
boiler part-load ratios for load and status, same-loop pump mass flow for
`hw_pump_status`, and `hw_pump_vfd_speed` as a pump mass-flow fraction
(affinity-law proxy). Boiler and pump fleet membership is traversed from each
`PlantLoop` supply-side `BranchList`; name matching is not accepted because it
can silently include equipment on another loop. DOE prototype IDFs enter via the E+ transition chain
(`PreProcess/IDFVersionUpdater`, e.g. 22.1 → 25.1 in six steps) +
`ConvertInputFormat`. Rules replay per family with rule-specific gates
(`PLANT_GATE`: CHW-0004 needs chiller load > 40%; boiler rules need both a
firing-status proxy and same-loop circulation; HW-0010 additionally splits
windows on active-setpoint changes and applies an 1800 s settling margin;
unnecessary-operation rules replay ungated — gating on the
equipment they accuse would mask them). An ungated rule is asserted from the
end of the 3600 s lead margin to the last sample; a run that ends inside that
margin is refused with the reason, because its window would hold no tick and
the verifier refuses a window that compares nothing. `PLANT_EXCLUDE` names the
baseline-fitted, reset-class, and by-construction rules with reasons.

Historical multi-climate plant outputs made before the cadence/timeline guard
remain useful exploratory statistics but are no longer card validation claims.
The current validation blocks use a fresh Denver OfficeLarge rerun at a real
60 s cadence and one RunPeriod: CHW-0004, HW-0003/HW-0004/HW-0005 are clean in
their actual gated windows; January has no CHW-0004 loaded window and July has
no HW-0004 boiler-active window.

PR 04 adds a separate **per-chiller adapter** for CHW-0007/CHW-0009 instead
of reusing the plant `max(PLR)` and mixed-header temperature. Each chiller
keeps its own part-load ratio, electricity rate, and direct evaporator outlet
temperature. `chiller_status` is disclosed as strictly positive machine
electricity; PLR × 100 is the per-machine load proxy. CHW-0007 compares the
individual outlet with the prototype's common supply setpoint only because its
parallel constant-flow chillers share that EnergyPlus plant target, and its FPR
windows begin 1800 s after the same machine is both running and above 20% load.
CHW-0009 copies are cadence-coupled to `count_scale=3600/step_s`; the current
60 s rerun uses 60 and can still miss cycles completed inside 120 seconds.
CHW-0008 is not replayed: EnergyPlus exposes operating outputs but no
independent final BAS per-chiller stage command, and deriving command from
status would make proof-of-operation tautological. In the Denver OfficeLarge
campaign, CHW-0007 is clean for both chillers in July's evaluable loaded
windows; January has no evaluable loaded windows. CHW-0009 is clear for both
machines in January and one in July; the other July status series contains
repeated sampled starts and raises the raw finding. It is classified as real
prototype cycling, not hidden as unexplained FPR or tuned away.

PR 03 adds a separate **per-pump adapter** for PMP-0004/PMP-0005 instead of
reusing those aggregate HW proxies. Each `Pump:VariableSpeed` instance keeps its
own series: `pump_status` is disclosed as strictly positive pump active power
(the prototype's variable-speed pump legitimately draws single-digit watts),
`pump_flow` is native branch mass-flow magnitude converted to L/s at 997 kg/m³,
and `pump_kw` is W/1000. At the current 60 s replay tick PMP-0004's copied graph
uses `count_scale=3600/60=60`; its run checks healthy FPR for starts that survive
sampling, but cycles completed inside 120 seconds remain invisible. EnergyPlus pump flow is
nonnegative, so PMP-0005 replay checks `yFault` healthy behavior only—it cannot
validate signed direction or reverse leakage. PMP-0006 remains excluded until a
frozen expected-power model can be trained on known-good per-pump data and
scored on a disjoint period; aggregate power or fitting the evaluation week
would not be a valid baseline comparison.

PR 05 adds a **per-tower-object TOWER-0005 adapter** for legacy
`CoolingTower:VariableSpeed`: native `Cooling Tower Outlet Temperature`, the
condenser-loop outlet `System Node Setpoint Temperature`, strictly positive fan
electricity as run proof, and native `Cooling Tower Air Flow Rate Ratio × 100`
as an explicitly effective-airflow proxy—not mechanical VFD feedback. Host-valid
expectations require positive mass flow and heat rejection, continuous fan PLR,
stable cell count/setpoint, speed above the graph's 30% placeholder, and an
1800 s lead after state changes. The Denver July 60 s campaign is clean for two
tower objects over 7,610 evaluated loaded/settled ticks across 16 windows;
January has no evaluable tower window and is excluded. Each EnergyPlus object
contains two internal cells, so this is aggregate object-level FPR only, not
per-cell proof and not an induced-fault TPR.

## First results (single building, superseded by the fleet sweep above) (B2B OfficeMedium-4004, Albuquerque, July week, 3 loops)

**16 of 18 auto-eligible AHU rules replayed clean across all three loops
for the full week** — the envelope pair (002/003), SAT families
(011/012/013/015/056/057/066/067), economizer directions (051/068), staging
(010), and plausibility (062) among them.

Two findings:

1. **AHU-0006 and AHU-0021 fire together during hot afternoons** (e.g.
   day 2 ~11:20, implied OA fraction (mat−rat)/(oat−rat) ≈ 66% at
   OAT 30.6 °C, economizer locked out). Root cause is real, not an
   artifact: `Controller:MechanicalVentilation` holds ventilation *flow*
   while VAV turndown shrinks total flow, so OA *fraction* balloons.
   Field-relevant conclusion: **OA-fraction rules need a DCV-aware host
   precondition** (suspend evaluation while ventilation demand overrides
   the minimum-OA state) — a documented noise source in the FDD
   literature, reproduced here with ground truth. Candidate card edit
   pending the owner's decision on recording validation results.
2. **The B2B baseline runs its fans 24/7** (no night setback): the fleet
   carries an after-hours fault by construction. Useful as a
   known-positive for schedule rules; excluded from FPR claims.

Reset-family rules (AHU-0023/AHU-0024-style) read "reset absent" against
constant-setpoint baselines by construction and are excluded from FPR
claims, per the validation plan.
