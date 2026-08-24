use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

use num_bigint::BigInt;

use super::*;
use crate::resolution::{
    ConnectorDefinition, ConnectorDirection, ConnectorPresence, DimensionDefinition, DimensionKind,
    EnumInputValue, EnumMemberDefinition, FiniteReal, NamedTypeDefinition, ParameterDefinition,
    ParameterSource, ParameterValue, PrimitiveType, ResolutionLimits, ScalarValue, Shape,
    TypeDefinition, TypeUse, ValidatedResolutionInput, resolve_validated,
};
use crate::scalar_abi::{
    EnumAbiMapping, EnumAbiMemberMapping, ScalarAbiProjection, ScalarAbiType, ScalarAbiValue,
    ScalarConnectorAbiRow, ScalarCoordinate, ScalarParameterAbiRow, project_scalar_abi,
};
use crate::scalar_names::allocate_scalar_names;
use crate::scalar_source_claims::{
    SourceClassClaim, SourceInventory, SourceInventoryFile, SourceInventoryLicense,
    SourceInventorySnapshot, SourceMemberBinding, SourceOwnerKind, SourcePin,
    project_scalar_source_claims,
};

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const TRIM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";
const ENUM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes";
const TRIM_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const TRIM_BLOB: &str = "sha1:028439a4fb478fc041d703a092d5186f5861eb03";
const DEVELOPMENT_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Plants/Chillers/Controller.mo";
const DEVELOPMENT_BLOB: &str = "sha1:8948cbcf1642d3456dece92832dc1cc2eb6f6fe7";

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("fixture real is finite")
}

fn coordinate(dimension_id: &str, member_id: &str, ordinal: usize) -> ScalarCoordinate {
    ScalarCoordinate {
        dimension_id: dimension_id.to_owned(),
        member_id: member_id.to_owned(),
        ordinal,
    }
}

fn locator() -> SourceFileLocator {
    SourceFileLocator {
        path: TRIM_PATH.to_owned(),
        git_blob_sha1: TRIM_BLOB.to_owned(),
    }
}

fn inventory_file(path: &str, blob: &str) -> SourceInventoryFile {
    SourceInventoryFile {
        path: path.to_owned(),
        mode: "100644".to_owned(),
        bytes: BigInt::from(1_u8),
        git_blob_sha1: blob.to_owned(),
        sha256: format!("sha256:{}", "1".repeat(64)),
    }
}

fn source_inventory() -> SourceInventory {
    SourceInventory {
        schema: "cxf-library/g36-source-inventory/v1".to_owned(),
        repository: "https://github.com/lbl-srg/modelica-buildings.git".to_owned(),
        source_root: "Buildings/Controls/OBC/ASHRAE/G36".to_owned(),
        inventory_scope: "source-root-regular-files".to_owned(),
        dependency_closure: "not-inventoried".to_owned(),
        license: SourceInventoryLicense {
            upstream_path: "Buildings/legal.html".to_owned(),
            retained_path: "routines/g36/LICENSE-BUILDINGS.html".to_owned(),
            git_blob_sha1: format!("sha1:{}", "2".repeat(40)),
            sha256: format!("sha256:{}", "3".repeat(64)),
        },
        snapshots: vec![
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                root_tree_sha1: format!("sha1:{}", "4".repeat(40)),
                file_count: BigInt::from(1_u8),
                total_bytes: BigInt::from(1_u8),
                modelica_file_count: BigInt::from(1_u8),
                package_order_count: BigInt::from(0_u8),
                files: vec![inventory_file(TRIM_PATH, TRIM_BLOB)],
            },
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
                root_tree_sha1: format!("sha1:{}", "5".repeat(40)),
                file_count: BigInt::from(1_u8),
                total_bytes: BigInt::from(1_u8),
                modelica_file_count: BigInt::from(1_u8),
                package_order_count: BigInt::from(0_u8),
                files: vec![inventory_file(DEVELOPMENT_PATH, DEVELOPMENT_BLOB)],
            },
        ],
    }
}

fn source_projection_for(named: &NamedScalarProjection) -> ScalarSourceClaimProjection {
    let mut bindings = Vec::new();
    let mut owners = HashSet::new();
    for row in &named.parameters {
        if owners.insert((SourceOwnerKind::Parameter, row.parameter_id.as_str())) {
            bindings.push(SourceMemberBinding {
                owner_kind: SourceOwnerKind::Parameter,
                owner_id: row.parameter_id.clone(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: row.parameter_id.clone(),
            });
        }
    }
    for row in &named.connectors {
        if owners.insert((SourceOwnerKind::Connector, row.connector_id.as_str())) {
            bindings.push(SourceMemberBinding {
                owner_kind: SourceOwnerKind::Connector,
                owner_id: row.connector_id.clone(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: row.connector_id.clone(),
            });
        }
    }
    project_scalar_source_claims(
        named,
        &source_inventory(),
        &[
            SourcePin {
                role: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
            },
            SourcePin {
                role: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
            },
        ],
        &[SourceClassClaim {
            canonical_class_path: TRIM_CLASS.to_owned(),
            snapshot: SourceSnapshotRole::Release,
            revision: RELEASE_REVISION.to_owned(),
            file: locator(),
        }],
        &bindings,
    )
    .expect("source claim projection succeeds")
}

fn full_named_projection() -> NamedScalarProjection {
    let input = ValidatedResolutionInput {
        canonical_id: "G36-RUST-BOUND-SCALAR-CHAIN".to_owned(),
        revision: BigInt::from(10_u8).pow(80),
        types: vec![
            TypeDefinition {
                type_id: "duration".to_owned(),
                definition: NamedTypeDefinition::Alias {
                    primitive: PrimitiveType::Real,
                    quantity: Some("time".to_owned()),
                    unit: Some("s".to_owned()),
                    display_unit: None,
                },
            },
            TypeDefinition {
                type_id: "operating_mode".to_owned(),
                definition: NamedTypeDefinition::Enum {
                    members: vec![
                        EnumMemberDefinition {
                            member_id: "occupied".to_owned(),
                            symbol: "OCCUPIED".to_owned(),
                        },
                        EnumMemberDefinition {
                            member_id: "unoccupied".to_owned(),
                            symbol: "UNOCCUPIED".to_owned(),
                        },
                    ],
                },
            },
        ],
        dimensions: vec![
            DimensionDefinition {
                dimension_id: "zones".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["north".to_owned(), "south".to_owned()],
                },
            },
            DimensionDefinition {
                dimension_id: "pair".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["first".to_owned(), "second".to_owned()],
                },
            },
        ],
        parameters: vec![
            ParameterDefinition {
                parameter_id: "sample_period_s".to_owned(),
                type_use: TypeUse::Named("duration".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Default,
                value: ParameterValue::Scalar(ScalarValue::Real(finite(60.0))),
            },
            ParameterDefinition {
                parameter_id: "gains".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Rank1 {
                    dimension_id: "pair".to_owned(),
                },
                source: ParameterSource::Assignment,
                value: ParameterValue::Rank1(vec![
                    ScalarValue::Integer(BigInt::from(1_u8)),
                    ScalarValue::Integer(BigInt::from(2_u8)),
                ]),
            },
            ParameterDefinition {
                parameter_id: "matrix_weights".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "pair".to_owned(),
                },
                source: ParameterSource::Default,
                value: ParameterValue::Rank2(vec![
                    vec![
                        ScalarValue::Integer(BigInt::from(1_u8)),
                        ScalarValue::Integer(BigInt::from(2_u8)),
                    ],
                    vec![
                        ScalarValue::Integer(BigInt::from(3_u8)),
                        ScalarValue::Integer(BigInt::from(4_u8)),
                    ],
                ]),
            },
            ParameterDefinition {
                parameter_id: "initial_mode".to_owned(),
                type_use: TypeUse::Named("operating_mode".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Enum(EnumInputValue {
                    type_id: "operating_mode".to_owned(),
                    member_id: "unoccupied".to_owned(),
                })),
            },
        ],
        connectors: vec![
            ConnectorDefinition {
                connector_id: "enable".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "matrix_feedback".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "pair".to_owned(),
                },
                presence: ConnectorPresence::Always,
            },
        ],
    };
    let resolved =
        resolve_validated(&input, ResolutionLimits::default()).expect("typed fixture resolves");
    let abi = project_scalar_abi(
        &resolved,
        &[EnumAbiMapping {
            type_id: "operating_mode".to_owned(),
            canonical_class_path: ENUM_CLASS.to_owned(),
            source_members: vec!["Occupied".to_owned(), "Unoccupied".to_owned()],
            member_mappings: vec![
                EnumAbiMemberMapping {
                    member_id: "occupied".to_owned(),
                    source_literal: "Occupied".to_owned(),
                },
                EnumAbiMemberMapping {
                    member_id: "unoccupied".to_owned(),
                    source_literal: "Unoccupied".to_owned(),
                },
            ],
        }],
    )
    .expect("typed fixture projects to scalar ABI");
    allocate_scalar_names(&abi).expect("name allocation succeeds")
}

fn direct_pair() -> (NamedScalarProjection, ScalarSourceClaimProjection) {
    let named = allocate_scalar_names(&ScalarAbiProjection {
        canonical_id: "G36-RUST-BOUND-SCALAR-DIRECT".to_owned(),
        revision: BigInt::from(7_u8),
        parameters: vec![ScalarParameterAbiRow {
            parameter_id: "gain".to_owned(),
            coordinates: vec![coordinate("zones", "north", 0)],
            abi_type: ScalarAbiType::Alias {
                type_id: "gain_type".to_owned(),
                primitive: PrimitiveType::Real,
                quantity: Some("dimensionless".to_owned()),
                unit: Some("1".to_owned()),
                display_unit: Some("percent".to_owned()),
            },
            source: ParameterSource::Assignment,
            value: ScalarAbiValue::Real(finite(1.5)),
        }],
        connectors: vec![ScalarConnectorAbiRow {
            connector_id: "signal".to_owned(),
            coordinates: Vec::new(),
            abi_type: ScalarAbiType::Primitive(PrimitiveType::Integer),
            direction: ConnectorDirection::Input,
        }],
    })
    .expect("name allocation succeeds");
    let source = ScalarSourceClaimProjection {
        canonical_id: named.canonical_id.clone(),
        revision: named.revision.clone(),
        parameters: named
            .parameters
            .iter()
            .map(|row| ScalarParameterSourceClaim {
                scalar_name: row.scalar_name.clone(),
                parameter_id: row.parameter_id.clone(),
                coordinates: row.coordinates.clone(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: "gain".to_owned(),
                snapshot: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                file: locator(),
            })
            .collect(),
        connectors: named
            .connectors
            .iter()
            .map(|row| ScalarConnectorSourceClaim {
                scalar_name: row.scalar_name.clone(),
                connector_id: row.connector_id.clone(),
                coordinates: row.coordinates.clone(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: "signal".to_owned(),
                snapshot: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                file: locator(),
            })
            .collect(),
    };
    (named, source)
}

fn error_codes(error: &BoundScalarError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn assert_error_code(
    named: &NamedScalarProjection,
    source: &ScalarSourceClaimProjection,
    expected_code: &str,
) -> BoundScalarError {
    let mut attempts = Vec::new();
    for _ in 0..2 {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            bind_scalar_source_claims(named, source)
        }));
        let result = outcome.expect("malformed typed input must not panic");
        let error = result.expect_err("malformed typed input must fail atomically");
        assert!(!error.diagnostics.is_empty());
        assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(
            error_codes(&error).contains(expected_code),
            "missing {expected_code} in {error:?}"
        );
        attempts.push(error);
    }
    assert_eq!(attempts[0], attempts[1]);
    attempts.remove(0)
}

#[test]
fn full_typed_chain_preserves_scalar_vector_matrix_abi_and_source_payload() {
    let named = full_named_projection();
    let source = source_projection_for(&named);
    let result = bind_scalar_source_claims(&named, &source).expect("binding succeeds");

    assert_eq!(result.canonical_id, named.canonical_id);
    assert_eq!(result.revision, BigInt::from(10_u8).pow(80));
    assert_eq!(
        result
            .parameters
            .iter()
            .map(|row| row.parameter_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "sample_period_s",
            "gains",
            "gains",
            "matrix_weights",
            "matrix_weights",
            "matrix_weights",
            "matrix_weights",
            "initial_mode",
        ]
    );
    assert_eq!(
        result
            .connectors
            .iter()
            .map(|row| row.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "enable",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
        ]
    );
    for (bound, original) in result.parameters.iter().zip(&named.parameters) {
        assert_eq!(bound.scalar_name, original.scalar_name);
        assert_eq!(bound.parameter_id, original.parameter_id);
        assert_eq!(bound.coordinates, original.coordinates);
        assert_eq!(bound.abi_type, original.abi_type);
        assert_eq!(bound.source, original.source);
        assert_eq!(bound.value, original.value);
        assert_eq!(bound.source_claim.canonical_class_path, TRIM_CLASS);
        assert_eq!(bound.source_claim.source_member, original.parameter_id);
        assert_eq!(bound.source_claim.snapshot, SourceSnapshotRole::Release);
        assert_eq!(bound.source_claim.revision, RELEASE_REVISION);
        assert_eq!(bound.source_claim.file, locator());
    }
    for (bound, original) in result.connectors.iter().zip(&named.connectors) {
        assert_eq!(bound.scalar_name, original.scalar_name);
        assert_eq!(bound.connector_id, original.connector_id);
        assert_eq!(bound.coordinates, original.coordinates);
        assert_eq!(bound.abi_type, original.abi_type);
        assert_eq!(bound.direction, original.direction);
        assert_eq!(bound.source_claim.source_member, original.connector_id);
    }
    let matrix_members = result
        .parameters
        .iter()
        .filter(|row| row.parameter_id == "matrix_weights")
        .map(|row| {
            row.coordinates
                .iter()
                .map(|coordinate| coordinate.member_id.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matrix_members,
        vec![
            vec!["north", "first"],
            vec!["north", "second"],
            vec!["south", "first"],
            vec!["south", "second"],
        ]
    );
    let enum_row = result
        .parameters
        .iter()
        .find(|row| row.parameter_id == "initial_mode")
        .expect("enum row exists");
    assert_eq!(
        enum_row.abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: ENUM_CLASS.to_owned()
        }
    );
    assert_eq!(enum_row.value, ScalarAbiValue::Enum { ordinal: 2 });
    assert_eq!(enum_row.source_claim.canonical_class_path, TRIM_CLASS);
    assert_ne!(ENUM_CLASS, TRIM_CLASS);
}

#[test]
fn reversed_source_rows_produce_identical_output() {
    let named = full_named_projection();
    let mut source = source_projection_for(&named);
    let expected = bind_scalar_source_claims(&named, &source).expect("binding succeeds");
    source.parameters.reverse();
    source.connectors.reverse();
    assert_eq!(
        bind_scalar_source_claims(&named, &source).expect("reordered binding succeeds"),
        expected
    );
}

#[test]
fn projection_metadata_must_be_valid_and_match_exactly() {
    let (named, source) = direct_pair();

    let mut changed = source.clone();
    changed.canonical_id = "G36-RUST-OTHER".to_owned();
    assert_error_code(&named, &changed, "canonical_id_mismatch");

    let mut changed = source.clone();
    changed.revision = BigInt::from(8_u8);
    assert_error_code(&named, &changed, "revision_mismatch");

    let mut changed_named = named.clone();
    changed_named.canonical_id.clear();
    changed_named.revision = BigInt::from(0_u8);
    let mut changed_source = source.clone();
    changed_source.canonical_id.clear();
    changed_source.revision = BigInt::from(-1_i8);
    let error = assert_error_code(&changed_named, &changed_source, "invalid_metadata");
    assert_eq!(
        error
            .diagnostics
            .iter()
            .filter(|item| item.code == "invalid_metadata")
            .count(),
        4
    );
}

#[test]
fn join_rejects_missing_extra_duplicate_wrong_kind_and_colliding_rows() {
    let (named, source) = direct_pair();

    let mut changed = source.clone();
    changed.parameters.clear();
    assert_error_code(&named, &changed, "missing_source_claim");

    let mut changed = source.clone();
    let extra_name = build_scalar_name("p_", "extra", &[]).expect("name builds");
    changed.parameters.push(ScalarParameterSourceClaim {
        scalar_name: extra_name,
        parameter_id: "extra".to_owned(),
        coordinates: Vec::new(),
        canonical_class_path: TRIM_CLASS.to_owned(),
        source_member: "extra".to_owned(),
        snapshot: SourceSnapshotRole::Release,
        revision: RELEASE_REVISION.to_owned(),
        file: locator(),
    });
    assert_error_code(&named, &changed, "extra_source_claim");

    let mut changed = source.clone();
    changed.parameters.push(changed.parameters[0].clone());
    assert_error_code(&named, &changed, "duplicate_scalar_name");

    let mut changed_named = named.clone();
    changed_named
        .connectors
        .push(changed_named.connectors[0].clone());
    assert_error_code(&changed_named, &source, "duplicate_scalar_name");

    let mut wrong_kind = source.clone();
    let parameter = wrong_kind.parameters.remove(0);
    wrong_kind.connectors.push(ScalarConnectorSourceClaim {
        scalar_name: parameter.scalar_name,
        connector_id: parameter.parameter_id,
        coordinates: parameter.coordinates,
        canonical_class_path: parameter.canonical_class_path,
        source_member: parameter.source_member,
        snapshot: parameter.snapshot,
        revision: parameter.revision,
        file: parameter.file,
    });
    assert_error_code(&named, &wrong_kind, "namespace_confusion");

    let mut colliding = source.clone();
    colliding.connectors[0].scalar_name = colliding.parameters[0].scalar_name.clone();
    let error = assert_error_code(&named, &colliding, "cross_kind_collision");
    assert!(error_codes(&error).contains("scalar_name_namespace"));

    let mut colliding_named = named.clone();
    colliding_named.connectors[0].scalar_name = colliding_named.parameters[0].scalar_name.clone();
    assert_error_code(&colliding_named, &source, "cross_kind_collision");
}

#[test]
fn join_compares_owner_and_every_coordinate_field() {
    let (named, source) = direct_pair();

    let mut changed = source.clone();
    changed.parameters[0].parameter_id = "other".to_owned();
    assert_error_code(&named, &changed, "owner_mismatch");

    let mut changed = source.clone();
    changed.parameters[0].coordinates.clear();
    assert_error_code(&named, &changed, "coordinate_count_mismatch");

    let mut changed = source.clone();
    changed.parameters[0].coordinates[0].dimension_id = "other_dimension".to_owned();
    assert_error_code(&named, &changed, "dimension_mismatch");

    let mut changed = source.clone();
    changed.parameters[0].coordinates[0].member_id = "other_member".to_owned();
    assert_error_code(&named, &changed, "member_mismatch");

    let mut changed = source.clone();
    changed.parameters[0].coordinates[0].ordinal = 99;
    let error = assert_error_code(&named, &changed, "ordinal_mismatch");
    assert!(!error_codes(&error).contains("scalar_name_mismatch"));
}

#[test]
fn canonical_names_and_complete_abi_payload_are_revalidated() {
    let (named, source) = direct_pair();

    let mut changed = named.clone();
    changed.parameters[0].scalar_name = "p_forged".to_owned();
    assert_error_code(&changed, &source, "scalar_name_mismatch");

    let mut changed = named.clone();
    let ScalarAbiType::Alias { type_id, .. } = &mut changed.parameters[0].abi_type else {
        panic!("fixture parameter is an alias")
    };
    type_id.clear();
    assert_error_code(&changed, &source, "invalid_abi_payload");

    for field in ["quantity", "unit", "display_unit"] {
        for invalid in ["", " untrimmed "] {
            let mut changed = named.clone();
            let ScalarAbiType::Alias {
                quantity,
                unit,
                display_unit,
                ..
            } = &mut changed.parameters[0].abi_type
            else {
                panic!("fixture parameter is an alias")
            };
            *match field {
                "quantity" => quantity,
                "unit" => unit,
                "display_unit" => display_unit,
                _ => unreachable!(),
            } = Some(invalid.to_owned());
            assert_error_code(&changed, &source, "invalid_abi_payload");
        }
    }

    let mut changed = named.clone();
    changed.parameters[0].value = ScalarAbiValue::Boolean(true);
    assert_error_code(&changed, &source, "invalid_abi_payload");

    let mut changed = named.clone();
    changed.parameters[0].abi_type = ScalarAbiType::Enum {
        canonical_class_path: ENUM_CLASS.to_owned(),
    };
    changed.parameters[0].value = ScalarAbiValue::Enum { ordinal: 0 };
    assert_error_code(&changed, &source, "invalid_abi_payload");

    let mut changed = named.clone();
    changed.parameters[0].abi_type = ScalarAbiType::Enum {
        canonical_class_path: String::new(),
    };
    changed.parameters[0].value = ScalarAbiValue::Enum { ordinal: 1 };
    assert_error_code(&changed, &source, "invalid_abi_payload");
}

#[test]
fn source_class_member_revision_path_and_blob_are_revalidated() {
    let (named, source) = direct_pair();

    for class_path in [
        "",
        "Buildings.Controls.OBC.ASHRAE.G36",
        "Buildings.Controls.OBC.ASHRAE.G36.9Bad",
        "Buildings.Controls.OBC.ASHRAE.G36.Café",
    ] {
        let mut changed = source.clone();
        changed.parameters[0].canonical_class_path = class_path.to_owned();
        assert_error_code(&named, &changed, "invalid_source_class");
    }
    let mut changed = source.clone();
    changed.parameters[0].canonical_class_path = format!("{CLASS_PATH_PREFIX}{}", "A.".repeat(600));
    assert_error_code(&named, &changed, "invalid_source_class");

    for member in ["", "9bad", "bad.member", "mø"] {
        let mut changed = source.clone();
        changed.parameters[0].source_member = member.to_owned();
        assert_error_code(&named, &changed, "invalid_source_member");
    }
    let mut changed = source.clone();
    changed.parameters[0].source_member = "m".repeat(256);
    assert_error_code(&named, &changed, "invalid_source_member");

    let mut changed = source.clone();
    changed.parameters[0].revision = RELEASE_REVISION.to_ascii_uppercase();
    assert_error_code(&named, &changed, "invalid_source_revision");

    for path in [
        "",
        "/Buildings/Controls/OBC/ASHRAE/G36/Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/../Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/Bad.txt",
        "Buildings/Controls/OBC/Other/Bad.mo",
    ] {
        let mut changed = source.clone();
        changed.parameters[0].file.path = path.to_owned();
        assert_error_code(&named, &changed, "invalid_source_path");
    }

    let mut changed = source.clone();
    changed.parameters[0].file.git_blob_sha1 = "sha1:ABC".to_owned();
    assert_error_code(&named, &changed, "invalid_source_blob");
}

#[test]
fn lexical_source_claims_do_not_recheck_inventory_or_pin_membership() {
    let (named, mut source) = direct_pair();
    let absent_path = "Buildings/Controls/OBC/ASHRAE/G36/Generic/Absent.mo";
    let unlisted_revision = "0".repeat(40);
    let unlisted_blob = format!("sha1:{}", "0".repeat(40));
    source.parameters[0].snapshot = SourceSnapshotRole::Development;
    source.parameters[0].revision = unlisted_revision.clone();
    source.parameters[0].file.path = absent_path.to_owned();
    source.parameters[0].file.git_blob_sha1 = unlisted_blob.clone();

    let result = bind_scalar_source_claims(&named, &source)
        .expect("lexically valid caller claims do not require inventory inputs");
    let claim = &result.parameters[0].source_claim;
    assert_eq!(claim.snapshot, SourceSnapshotRole::Development);
    assert_eq!(claim.revision, unlisted_revision);
    assert_eq!(claim.file.path, absent_path);
    assert_eq!(claim.file.git_blob_sha1, unlisted_blob);
}

#[test]
fn malformed_inputs_return_complete_sorted_repeatable_atomic_errors() {
    let (mut named, mut source) = direct_pair();
    named.canonical_id.clear();
    named.revision = BigInt::from(0_u8);
    named.parameters[0].scalar_name = "bad".to_owned();
    named.parameters[0].coordinates[0].dimension_id.clear();
    named.parameters[0].abi_type = ScalarAbiType::Alias {
        type_id: String::new(),
        primitive: PrimitiveType::Boolean,
        quantity: Some(" bad ".to_owned()),
        unit: None,
        display_unit: None,
    };
    source.canonical_id.clear();
    source.revision = BigInt::from(-1_i8);
    source.parameters[0].canonical_class_path = "bad".to_owned();
    source.parameters[0].source_member = "bad.member".to_owned();
    source.parameters[0].revision = "BAD".to_owned();
    source.parameters[0].file.path = "../bad".to_owned();
    source.parameters[0].file.git_blob_sha1 = "bad".to_owned();

    let error = assert_error_code(&named, &source, "invalid_metadata");
    let codes = error_codes(&error);
    for expected in [
        "scalar_name_namespace",
        "invalid_dimension_id",
        "invalid_abi_payload",
        "invalid_source_class",
        "invalid_source_member",
        "invalid_source_revision",
        "invalid_source_path",
        "invalid_source_blob",
    ] {
        assert!(codes.contains(expected), "missing {expected} in {error:?}");
    }
}

#[test]
fn output_is_deeply_detached_and_lookup_is_linear_and_optional() {
    let (mut named, mut source) = direct_pair();
    let result = bind_scalar_source_claims(&named, &source).expect("binding succeeds");
    let parameter_name = result.parameters[0].scalar_name.clone();
    let connector_name = result.connectors[0].scalar_name.clone();
    assert_eq!(
        result.row_for_scalar(&parameter_name),
        Some(BoundScalarRef::Parameter(&result.parameters[0]))
    );
    assert_eq!(
        result.row_for_scalar(&connector_name),
        Some(BoundScalarRef::Connector(&result.connectors[0]))
    );
    assert!(result.row_for_scalar("missing").is_none());
    let empty_named = NamedScalarProjection {
        canonical_id: "empty".to_owned(),
        revision: BigInt::from(1_u8),
        parameters: Vec::new(),
        connectors: Vec::new(),
    };
    let empty_source = ScalarSourceClaimProjection {
        canonical_id: "empty".to_owned(),
        revision: BigInt::from(1_u8),
        parameters: Vec::new(),
        connectors: Vec::new(),
    };
    let empty = bind_scalar_source_claims(&empty_named, &empty_source)
        .expect("empty projections bind without an index");
    assert!(empty.row_for_scalar("missing").is_none());

    named.canonical_id = "changed".to_owned();
    named.revision = BigInt::from(1_u8);
    named.parameters[0].scalar_name = "changed".to_owned();
    named.parameters[0].parameter_id = "changed".to_owned();
    named.parameters[0].coordinates[0].member_id = "changed".to_owned();
    let ScalarAbiType::Alias {
        type_id, quantity, ..
    } = &mut named.parameters[0].abi_type
    else {
        panic!("fixture parameter is an alias")
    };
    *type_id = "changed".to_owned();
    *quantity = None;
    named.parameters[0].value = ScalarAbiValue::Integer(BigInt::from(99_u8));
    source.parameters[0].canonical_class_path = "changed".to_owned();
    source.parameters[0].source_member = "changed".to_owned();
    source.parameters[0].revision = "0".repeat(40);
    source.parameters[0].file.path = "changed".to_owned();
    source.parameters[0].file.git_blob_sha1 = format!("sha1:{}", "0".repeat(40));

    let bound = &result.parameters[0];
    assert_eq!(result.canonical_id, "G36-RUST-BOUND-SCALAR-DIRECT");
    assert_eq!(result.revision, BigInt::from(7_u8));
    assert_eq!(bound.scalar_name, parameter_name);
    assert_eq!(bound.parameter_id, "gain");
    assert_eq!(bound.coordinates[0].member_id, "north");
    assert_eq!(bound.value, ScalarAbiValue::Real(finite(1.5)));
    let ScalarAbiType::Alias {
        type_id, quantity, ..
    } = &bound.abi_type
    else {
        panic!("bound parameter is an alias")
    };
    assert_eq!(type_id, "gain_type");
    assert_eq!(quantity.as_deref(), Some("dimensionless"));
    assert_eq!(bound.source_claim.canonical_class_path, TRIM_CLASS);
    assert_eq!(bound.source_claim.source_member, "gain");
    assert_eq!(bound.source_claim.revision, RELEASE_REVISION);
    assert_eq!(bound.source_claim.file, locator());
}

#[test]
fn resource_helpers_fail_without_large_allocations() {
    assert_eq!(checked_total_count([1_usize, 2, 3, 4]), Some(10));
    assert_eq!(checked_total_count([usize::MAX, 1]), None);

    let mut map = HashMap::<usize, usize>::new();
    let mut diagnostics = Vec::new();
    assert!(!reserve_map(
        &mut map,
        usize::MAX,
        "$.test",
        "test index",
        &mut diagnostics,
    ));
    assert!(map.is_empty());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "resource_limit");
    assert_eq!(diagnostics[0].location, "$.test");
    assert_eq!(diagnostics[0].message, "test index allocation failed");

    let error = output_resource_error("$.parameters", "test row");
    assert_eq!(error.diagnostics[0].code, "resource_limit");
    assert_eq!(error.diagnostics[0].message, "test row allocation failed");
}

#[test]
fn materialization_uses_transient_source_indexes_instead_of_row_scans() {
    let source_text = include_str!("../bound_scalars.rs");
    let prepare_rows = source_text
        .split_once("fn prepare_rows")
        .expect("prepare_rows exists")
        .1
        .split_once("fn clone_text")
        .expect("clone_text follows prepare_rows")
        .0;

    assert!(prepare_rows.contains("parameter_claims"));
    assert!(prepare_rows.contains("connector_claims"));
    assert_eq!(prepare_rows.matches(".try_reserve(").count(), 2);
    assert_eq!(
        prepare_rows
            .matches(".get(row.scalar_name.as_str())")
            .count(),
        2
    );
    assert!(!prepare_rows.contains(".find("));
}

#[test]
fn output_surface_excludes_deferred_io_serialization_and_runtime_contracts() {
    let (named, source) = direct_pair();
    let result = bind_scalar_source_claims(&named, &source).expect("binding succeeds");
    let BoundScalarProjection {
        canonical_id,
        revision,
        mut parameters,
        mut connectors,
    } = result;
    let BoundScalarParameterRow {
        scalar_name: parameter_name,
        parameter_id,
        coordinates: parameter_coordinates,
        abi_type: parameter_type,
        source: parameter_source,
        value,
        source_claim: parameter_claim,
    } = parameters.remove(0);
    let BoundScalarConnectorRow {
        scalar_name: connector_name,
        connector_id,
        coordinates: connector_coordinates,
        abi_type: connector_type,
        direction,
        source_claim: connector_claim,
    } = connectors.remove(0);
    let BoundSourceClaim {
        canonical_class_path,
        source_member,
        snapshot,
        revision: source_revision,
        file,
    } = parameter_claim;

    assert_eq!(canonical_id, "G36-RUST-BOUND-SCALAR-DIRECT");
    assert_eq!(revision, BigInt::from(7_u8));
    assert!(!parameter_name.is_empty());
    assert_eq!(parameter_id, "gain");
    assert_eq!(parameter_coordinates.len(), 1);
    assert!(matches!(parameter_type, ScalarAbiType::Alias { .. }));
    assert_eq!(parameter_source, ParameterSource::Assignment);
    assert_eq!(value, ScalarAbiValue::Real(finite(1.5)));
    assert_eq!(canonical_class_path, TRIM_CLASS);
    assert_eq!(source_member, "gain");
    assert_eq!(snapshot, SourceSnapshotRole::Release);
    assert_eq!(source_revision, RELEASE_REVISION);
    assert_eq!(file, locator());
    assert!(!connector_name.is_empty());
    assert_eq!(connector_id, "signal");
    assert!(connector_coordinates.is_empty());
    assert_eq!(
        connector_type,
        ScalarAbiType::Primitive(PrimitiveType::Integer)
    );
    assert_eq!(direction, ConnectorDirection::Input);
    assert_eq!(connector_claim.source_member, "signal");

    let source_text = include_str!("../bound_scalars.rs");
    for forbidden in [
        "std::fs",
        "std::process",
        "std::env",
        "std::net",
        "serde",
        "serde_json",
        "Command::",
        "reqwest",
    ] {
        assert!(
            !source_text.contains(forbidden),
            "bound scalar stage must not use {forbidden}"
        );
    }
}
