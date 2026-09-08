# Source notes: thermal-zone state

Primary sequence basis: ASHRAE Guideline 36-2018, section 5.3.5, printed page 31
(PDF page 33 of the user-supplied 102-page document). No addenda have been folded
into this reference. The published standard is not redistributed here.

This is an independent block-graph implementation, authored in
`tools/authoring/g36_reference.py::zone_state`. It is not copied from or translated
by the Modelica Buildings source pipeline. It implements the source's three
mutually exclusive states on valid, nonnegative loop inputs. The fourth output,
`loop_conflict`, is our explicit diagnostic extension.

External engineering context, not a change to the source baseline:
https://obc.lbl.gov/specification/example.html#deadbands-for-hard-switches
explains why OBC implementations add hysteresis. The engine's existing
`thermal_zones_zone_states.jsonld` development fixture follows different
hysteresis/competition behavior. Do not describe the literal 2018 reference as
an interchangeable replacement without a reviewed behavioral profile.

CXF format and model-of-computation context:
https://obc.lbl.gov/specification/cxf.html
https://obc.lbl.gov/specification/cdl.html
The emitted graph targets the existing engine's flat composite subset and uses
typed ports, explicit ownership, and elementary algebraic CDL blocks. This is
not a declaration of universal CXF interoperability or Modelica equivalence.
