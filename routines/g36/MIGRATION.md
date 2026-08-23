# G36 L0 catalog migration

Status: L0 catalog cutover; no routine is executable.

## Cutover

The pre-cutover state is commit
`566363528f5d3331d30a2c07acfc65dbabc51a8b`. It contained ten fixed
AirEconomizerHighLimits deployment directories, executable registry rows,
`DONOR_PIN`, and the single `SOURCE_PIN` source identity.

This cutover removes those directories, rows, and pins. The replacement source
identity is split between:

- release baseline `55abf579598ca81cae0a82f337350375958e6722`; and
- reviewed development baseline `eccb40b3974bb10eef120c5670a6454e43ca36e3`.

The scope manifest records 22 planning anchors. It does not preserve the fixed
variants, define canonical classes, or claim implementation coverage.

## Archive and rollback

Git history is the archive; no duplicate archive directory or tag is part of
the migration. Roll back by reverting the atomic catalog-cutover commit. For a
forensic view or path-level restore of the retired experiment, use the exact
pre-cutover commit
`566363528f5d3331d30a2c07acfc65dbabc51a8b`. Restoring those files does not make
them valid under the L0 catalog contract.
