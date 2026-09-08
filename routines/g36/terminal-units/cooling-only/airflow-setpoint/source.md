# Source notes: cooling-only VAV airflow setpoint

Primary sequence basis: ASHRAE Guideline 36-2018, Table 5.5.4 and sections
5.5.5.1–5.5.5.2, printed page 33 (PDF page 35); Figure 5.5.5, printed page 34
(PDF page 36). The rendered mode table was inspected, not inferred from the
reheat table. No addenda have been applied. The standard is not redistributed.

Independent graph author: `tools/authoring/g36_reference.py::airflow_setpoint`.
The two source clauses and mode table map to three parts of the graph:
`occupied`/`cooldown`/`setup` and `minimum`/`maximum` select the table limits;
`span`/`modulation`/`mapped_flow` form the linear reset; `warm_air`/`can_cool` and
`select_flow` apply the zone-state and strict warm-supply condition.

Library choices, not additional G36 requirements: fractions use 0..1; flows use
m3/s; temperature uses K; group-mode symbols have the explicit local integer ABI
in reference.json; warm_supply_air is an inspection output; malformed input is
rejected by the offline runner rather than assigned an invented safe fallback.

Section 5.5.6 damper control is deliberately outside this class, as are the
referenced sections 5.2 and 5.3 and the alarms/requests/overrides later in 5.5.
The 2018 notes on cooling-only DCV contain cautions and differing phrasing;
this pass does not resolve them or claim to implement DCV.

Supporting format/verification context:
https://obc.lbl.gov/specification/cxf.html
https://obc.lbl.gov/specification/verification.html
https://obc.lbl.gov/specification/example.html#deadbands-for-hard-switches
OBC's reasons for adding hysteresis are relevant to later field qualification,
not permission to change this literal reference's temperature-equality behavior.
