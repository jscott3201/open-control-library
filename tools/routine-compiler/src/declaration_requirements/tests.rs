use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

use num_bigint::BigInt;

use super::*;
use crate::resolution::{
    ConnectorDefinition, ConnectorDirection, ConnectorPresence, DimensionDefinition, DimensionKind,
    FiniteReal, ParameterDefinition, ParameterSource, ParameterValue, PrimitiveType,
    ResolutionLimits, ScalarValue, Shape, TypeUse, ValidatedResolutionInput, resolve_validated,
};
use crate::scalar_abi::{ScalarAbiValue, project_scalar_abi};
use crate::scalar_names::allocate_scalar_names;
use crate::scalar_source_claims::{
    SourceClassClaim, SourceInventory, SourceInventoryFile, SourceInventoryLicense,
    SourceInventorySnapshot, SourceMemberBinding, SourceOwnerKind, SourcePin,
    project_scalar_source_claims,
};

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const TRIM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";
const TRIM_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const TRIM_BLOB: &str = "sha1:028439a4fb478fc041d703a092d5186f5861eb03";
const TIME_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TimeSuppression";
const TIME_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TimeSuppression.mo";
const DEVELOPMENT_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Plants/Chillers/Controller.mo";
const DEVELOPMENT_BLOB: &str = "sha1:8948cbcf1642d3456dece92832dc1cc2eb6f6fe7";

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("fixture value is finite")
}

fn coordinate(dimension_id: &str, member_id: &str, ordinal: usize) -> ScalarCoordinate {
    ScalarCoordinate {
        dimension_id: dimension_id.to_owned(),
        member_id: member_id.to_owned(),
        ordinal,
    }
}

fn locator(path: &str, blob: &str) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: blob.to_owned(),
    }
}

fn scalar_name(prefix: &str, owner_id: &str, coordinates: &[ScalarCoordinate]) -> String {
    build_scalar_name(prefix, owner_id, coordinates).expect("fixture scalar name builds")
}

fn parameter(
    parameter_id: &str,
    coordinates: Vec<ScalarCoordinate>,
    source_member: &str,
) -> ScalarParameterSourceClaim {
    ScalarParameterSourceClaim {
        scalar_name: scalar_name("p_", parameter_id, &coordinates),
        parameter_id: parameter_id.to_owned(),
        coordinates,
        canonical_class_path: TRIM_CLASS.to_owned(),
        source_member: source_member.to_owned(),
        snapshot: SourceSnapshotRole::Release,
        revision: RELEASE_REVISION.to_owned(),
        file: locator(TRIM_PATH, TRIM_BLOB),
    }
}

fn connector(
    connector_id: &str,
    coordinates: Vec<ScalarCoordinate>,
    source_member: &str,
) -> ScalarConnectorSourceClaim {
    ScalarConnectorSourceClaim {
        scalar_name: scalar_name("c_", connector_id, &coordinates),
        connector_id: connector_id.to_owned(),
        coordinates,
        canonical_class_path: TRIM_CLASS.to_owned(),
        source_member: source_member.to_owned(),
        snapshot: SourceSnapshotRole::Release,
        revision: RELEASE_REVISION.to_owned(),
        file: locator(TRIM_PATH, TRIM_BLOB),
    }
}

fn direct_projection(
    parameters: Vec<ScalarParameterSourceClaim>,
    connectors: Vec<ScalarConnectorSourceClaim>,
) -> ScalarSourceClaimProjection {
    ScalarSourceClaimProjection {
        canonical_id: "typed-boundary-id".to_owned(),
        revision: BigInt::from(10_u8).pow(80),
        parameters,
        connectors,
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

fn full_chain_source_projection() -> ScalarSourceClaimProjection {
    let input = ValidatedResolutionInput {
        canonical_id: "typed-full-chain-id".to_owned(),
        revision: BigInt::from(10_u8).pow(80),
        types: Vec::new(),
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
                type_use: TypeUse::Primitive(PrimitiveType::Real),
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
                        ScalarValue::Real(finite(1.0)),
                        ScalarValue::Real(finite(2.0)),
                    ],
                    vec![
                        ScalarValue::Real(finite(3.0)),
                        ScalarValue::Real(finite(4.0)),
                    ],
                ]),
            },
        ],
        connectors: vec![
            ConnectorDefinition {
                connector_id: "requests".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Rank1 {
                    dimension_id: "pair".to_owned(),
                },
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
        resolve_validated(&input, ResolutionLimits::default()).expect("fixture resolves");
    let abi = project_scalar_abi(&resolved, &[]).expect("fixture projects to scalar ABI");
    assert!(matches!(abi.parameters[0].value, ScalarAbiValue::Real(_)));
    let named = allocate_scalar_names(&abi).expect("fixture names allocate");
    let bindings = [
        (
            SourceOwnerKind::Parameter,
            "sample_period_s",
            "samplePeriod",
        ),
        (SourceOwnerKind::Parameter, "gains", "gains"),
        (
            SourceOwnerKind::Parameter,
            "matrix_weights",
            "matrixWeights",
        ),
        (SourceOwnerKind::Connector, "requests", "numOfReq"),
        (
            SourceOwnerKind::Connector,
            "matrix_feedback",
            "matrixFeedback",
        ),
    ]
    .into_iter()
    .map(
        |(owner_kind, owner_id, source_member)| SourceMemberBinding {
            owner_kind,
            owner_id: owner_id.to_owned(),
            canonical_class_path: TRIM_CLASS.to_owned(),
            source_member: source_member.to_owned(),
        },
    )
    .collect::<Vec<_>>();
    project_scalar_source_claims(
        &named,
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
            file: locator(TRIM_PATH, TRIM_BLOB),
        }],
        &bindings,
    )
    .expect("fixture projects to source claims")
}

fn error_codes(error: &DeclarationRequirementError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn assert_error_code(
    projection: &ScalarSourceClaimProjection,
    expected_code: &str,
) -> DeclarationRequirementError {
    let mut attempts = Vec::new();
    for _ in 0..2 {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            project_declaration_requirements(projection)
        }));
        let result = outcome.expect("malformed typed input must not panic");
        let error = result.expect_err("malformed typed input must fail atomically");
        assert!(!error.diagnostics.is_empty());
        assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(
            error_codes(&error).contains(expected_code),
            "missing {expected_code} in {error:?}"
        );
        assert!(!error.to_string().contains("panicked"));
        attempts.push(error);
    }
    assert_eq!(attempts[0], attempts[1]);
    attempts.remove(0)
}

#[test]
fn full_typed_chain_groups_source_claims_without_abi_payload() {
    let source = full_chain_source_projection();
    let result = project_declaration_requirements(&source).expect("projection succeeds");

    assert_eq!(result.canonical_id, source.canonical_id);
    assert_eq!(result.revision, BigInt::from(10_u8).pow(80));
    assert_eq!(
        result
            .parameters
            .iter()
            .map(|requirement| requirement.parameter_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sample_period_s", "gains", "matrix_weights"]
    );
    assert_eq!(
        result
            .parameters
            .iter()
            .map(|requirement| requirement.scalar_names.len())
            .collect::<Vec<_>>(),
        vec![1, 2, 4]
    );
    assert_eq!(
        result
            .connectors
            .iter()
            .map(|requirement| requirement.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec!["requests", "matrix_feedback"]
    );
    assert_eq!(
        result
            .connectors
            .iter()
            .map(|requirement| requirement.scalar_names.len())
            .collect::<Vec<_>>(),
        vec![2, 4]
    );
    assert_eq!(
        result
            .parameters
            .iter()
            .flat_map(|requirement| requirement.scalar_names.iter())
            .collect::<Vec<_>>(),
        source
            .parameters
            .iter()
            .map(|row| &row.scalar_name)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result
            .connectors
            .iter()
            .flat_map(|requirement| requirement.scalar_names.iter())
            .collect::<Vec<_>>(),
        source
            .connectors
            .iter()
            .map(|row| &row.scalar_name)
            .collect::<Vec<_>>()
    );
    for requirement in result
        .parameters
        .iter()
        .map(|item| (&item.canonical_class_path, &item.file))
        .chain(
            result
                .connectors
                .iter()
                .map(|item| (&item.canonical_class_path, &item.file)),
        )
    {
        assert_eq!(requirement.0, TRIM_CLASS);
        assert_eq!(requirement.1, &locator(TRIM_PATH, TRIM_BLOB));
    }
}

#[test]
fn interleaved_scalar_vector_matrix_rows_preserve_both_orders() {
    let north = coordinate("zones", "north", 0);
    let south = coordinate("zones", "south", 1);
    let first = coordinate("pair", "first", 0);
    let second = coordinate("pair", "second", 1);
    let parameters = vec![
        parameter(
            "matrix_weights",
            vec![north.clone(), first.clone()],
            "matrixWeights",
        ),
        parameter("gain", Vec::new(), "gain"),
        parameter(
            "matrix_weights",
            vec![north.clone(), second.clone()],
            "matrixWeights",
        ),
        parameter(
            "matrix_weights",
            vec![south.clone(), first],
            "matrixWeights",
        ),
    ];
    let connectors = vec![
        connector("beta", vec![north.clone()], "betaValue"),
        connector("alpha", vec![north], "alphaValue"),
        connector("beta", vec![south.clone()], "betaValue"),
        connector("alpha", vec![south], "alphaValue"),
    ];
    let projection = direct_projection(parameters.clone(), connectors.clone());
    let result = project_declaration_requirements(&projection).expect("projection succeeds");

    assert_eq!(
        result
            .parameters
            .iter()
            .map(|item| item.parameter_id.as_str())
            .collect::<Vec<_>>(),
        vec!["matrix_weights", "gain"]
    );
    assert_eq!(
        result.parameters[0].scalar_names,
        parameters
            .iter()
            .filter(|row| row.parameter_id == "matrix_weights")
            .map(|row| row.scalar_name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result
            .connectors
            .iter()
            .map(|item| item.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec!["beta", "alpha"]
    );
    assert_eq!(
        result.connectors[0].scalar_names,
        vec![
            connectors[0].scalar_name.clone(),
            connectors[2].scalar_name.clone()
        ]
    );
}

#[test]
fn owner_and_source_key_namespaces_are_independent() {
    let projection = direct_projection(
        vec![parameter("shared", Vec::new(), "sharedMember")],
        vec![connector("shared", Vec::new(), "sharedMember")],
    );
    let result = project_declaration_requirements(&projection).expect("projection succeeds");

    assert_eq!(result.parameters[0].parameter_id, "shared");
    assert_eq!(result.connectors[0].connector_id, "shared");
    assert_eq!(result.parameters[0].source_member, "sharedMember");
    assert_eq!(result.connectors[0].source_member, "sharedMember");
    assert_ne!(
        result.parameters[0].scalar_names,
        result.connectors[0].scalar_names
    );
}

#[test]
fn every_source_identity_component_must_be_coherent_per_owner() {
    let first = connector(
        "requests",
        vec![coordinate("zones", "north", 0)],
        "requests",
    );
    let second = connector(
        "requests",
        vec![coordinate("zones", "south", 1)],
        "requests",
    );
    let mut cases = Vec::new();

    let mut changed = second.clone();
    changed.canonical_class_path = TIME_CLASS.to_owned();
    cases.push(("class", changed));

    let mut changed = second.clone();
    changed.source_member = "otherMember".to_owned();
    cases.push(("member", changed));

    let mut changed = second.clone();
    changed.snapshot = SourceSnapshotRole::Development;
    cases.push(("snapshot", changed));

    let mut changed = second.clone();
    changed.revision = "0".repeat(40);
    cases.push(("revision", changed));

    let mut changed = second.clone();
    changed.file.path = TIME_PATH.to_owned();
    cases.push(("path", changed));

    let mut changed = second;
    changed.file.git_blob_sha1 = format!("sha1:{}", "1".repeat(40));
    cases.push(("blob", changed));

    for (label, changed) in cases {
        let projection = direct_projection(Vec::new(), vec![first.clone(), changed]);
        let error = assert_error_code(&projection, "inconsistent_owner_source");
        assert_eq!(
            error
                .diagnostics
                .iter()
                .filter(|item| item.code == "inconsistent_owner_source")
                .count(),
            1,
            "unexpected diagnostics for {label}: {error:?}"
        );
    }
}

#[test]
fn duplicate_names_source_keys_and_cross_kind_collisions_fail() {
    let repeated = parameter("gain", Vec::new(), "gain");
    assert_error_code(
        &direct_projection(vec![repeated.clone(), repeated.clone()], Vec::new()),
        "duplicate_scalar_name",
    );

    let first = parameter("first", Vec::new(), "sharedMember");
    let second = parameter("second", Vec::new(), "sharedMember");
    assert_error_code(
        &direct_projection(vec![first, second], Vec::new()),
        "duplicate_source_key",
    );

    let mut colliding = connector("signal", Vec::new(), "signal");
    colliding.scalar_name = repeated.scalar_name.clone();
    let error = assert_error_code(
        &direct_projection(vec![repeated], vec![colliding]),
        "cross_kind_collision",
    );
    let codes = error_codes(&error);
    assert!(codes.contains("scalar_name_namespace"));
    assert!(codes.contains("scalar_name_mismatch"));
}

#[test]
fn forged_metadata_ids_coordinates_names_and_source_payload_fail_closed() {
    let mut hostile = parameter("gain", vec![coordinate("zones", "north", 0)], "gain");
    hostile.parameter_id.clear();
    hostile.scalar_name.clear();
    hostile.coordinates[0].dimension_id.clear();
    hostile.coordinates[0].member_id.clear();
    hostile.canonical_class_path = "Bad.Class".to_owned();
    hostile.source_member = "bad.member".to_owned();
    hostile.revision = "BAD".to_owned();
    hostile.file.path = "../bad.txt".to_owned();
    hostile.file.git_blob_sha1 = "sha1:BAD".to_owned();
    let mut projection = direct_projection(vec![hostile], Vec::new());
    projection.canonical_id.clear();
    projection.revision = BigInt::from(0_u8);

    let error = assert_error_code(&projection, "invalid_metadata");
    let codes = error_codes(&error);
    for expected in [
        "invalid_owner_id",
        "invalid_scalar_name",
        "invalid_dimension_id",
        "invalid_member_id",
        "invalid_source_class",
        "invalid_source_member",
        "invalid_source_revision",
        "invalid_source_path",
        "invalid_source_blob",
    ] {
        assert!(codes.contains(expected), "missing {expected} in {error:?}");
    }

    let mut mismatched = parameter("gain", Vec::new(), "gain");
    mismatched.scalar_name = "p_00".to_owned();
    assert_error_code(
        &direct_projection(vec![mismatched], Vec::new()),
        "scalar_name_mismatch",
    );

    let mut non_modelica = parameter("gain", Vec::new(), "gain");
    non_modelica.file.path = "Buildings/Controls/OBC/ASHRAE/G36/Generic/package.order".to_owned();
    assert_error_code(
        &direct_projection(vec![non_modelica], Vec::new()),
        "invalid_source_path",
    );
}

#[test]
fn typed_boundary_requires_nonempty_ids_without_python_pattern_tightening() {
    let coordinates = vec![coordinate("Bad Dimension", "north_zone", usize::MAX)];
    let row = parameter("Bad-ID", coordinates, "validMember");
    let mut projection = direct_projection(vec![row.clone()], Vec::new());
    projection.canonical_id = "not-a-python-g36-id".to_owned();
    let result = project_declaration_requirements(&projection)
        .expect("nonempty typed IDs and arbitrary ordinal remain valid");

    assert_eq!(result.canonical_id, "not-a-python-g36-id");
    assert_eq!(result.parameters[0].parameter_id, "Bad-ID");
    assert_eq!(result.parameters[0].scalar_names, vec![row.scalar_name]);
}

#[test]
fn complete_diagnostics_are_sorted_repeatable_atomic_and_non_panicking() {
    let mut first = parameter("first", Vec::new(), "sharedMember");
    first.scalar_name = "bad".to_owned();
    first.canonical_class_path = "bad".to_owned();
    first.revision = "bad".to_owned();
    first.file.path = "/bad".to_owned();
    first.file.git_blob_sha1 = "bad".to_owned();
    let mut second = parameter("second", Vec::new(), "sharedMember");
    second.scalar_name = first.scalar_name.clone();
    second.source_member = "bad.member".to_owned();
    let mut projection = direct_projection(vec![first, second], Vec::new());
    projection.canonical_id.clear();
    projection.revision = BigInt::from(-1_i8);

    let error = assert_error_code(&projection, "invalid_metadata");
    let codes = error_codes(&error);
    for expected in [
        "scalar_name_namespace",
        "scalar_name_mismatch",
        "invalid_source_class",
        "invalid_source_member",
        "invalid_source_revision",
        "invalid_source_path",
        "invalid_source_blob",
        "duplicate_scalar_name",
    ] {
        assert!(codes.contains(expected), "missing {expected} in {error:?}");
    }
    fn assert_error_trait<T: Error>() {}
    assert_error_trait::<DeclarationRequirementError>();
}

#[test]
fn empty_projection_and_deep_detachment_are_preserved() {
    let empty = direct_projection(Vec::new(), Vec::new());
    let empty_result = project_declaration_requirements(&empty).expect("empty projection succeeds");
    assert!(empty_result.parameters.is_empty());
    assert!(empty_result.connectors.is_empty());

    let north = coordinate("zones", "north", 0);
    let south = coordinate("zones", "south", 1);
    let mut source = direct_projection(
        vec![
            parameter("weights", vec![north], "weights"),
            parameter("weights", vec![south], "weights"),
        ],
        Vec::new(),
    );
    let result = project_declaration_requirements(&source).expect("projection succeeds");
    let expected_revision = source.revision.clone();
    source.canonical_id = "changed".to_owned();
    source.revision = BigInt::from(1_u8);
    source.parameters[0].parameter_id = "changed".to_owned();
    source.parameters[0].scalar_name = "changed".to_owned();
    source.parameters[0].canonical_class_path = TIME_CLASS.to_owned();
    source.parameters[0].source_member = "changed".to_owned();
    source.parameters[0].revision = "0".repeat(40);
    source.parameters[0].file.path = TIME_PATH.to_owned();
    source.parameters[0].file.git_blob_sha1 = format!("sha1:{}", "0".repeat(40));

    assert_eq!(result.canonical_id, "typed-boundary-id");
    assert_eq!(result.revision, expected_revision);
    assert_eq!(result.parameters[0].parameter_id, "weights");
    assert_eq!(result.parameters[0].canonical_class_path, TRIM_CLASS);
    assert_eq!(result.parameters[0].source_member, "weights");
    assert_eq!(result.parameters[0].revision, RELEASE_REVISION);
    assert_eq!(result.parameters[0].file, locator(TRIM_PATH, TRIM_BLOB));
    assert_eq!(result.parameters[0].scalar_names.len(), 2);
    assert!(
        result.parameters[0]
            .scalar_names
            .iter()
            .all(|name| name != "changed")
    );
}

#[test]
fn resource_helpers_fail_without_large_allocations() {
    assert_eq!(checked_total_count([1_usize, 2, 3]), Some(6));
    assert_eq!(checked_total_count([usize::MAX, 1]), None);

    let mut map = HashMap::<usize, usize>::new();
    let error = reserve_map(&mut map, usize::MAX, "$.test", "test index")
        .expect_err("impossible capacity must fail");
    assert!(map.is_empty());
    assert_eq!(error.diagnostics[0].code, "resource_limit");
    assert_eq!(error.diagnostics[0].location, "$.test");
    assert_eq!(error.diagnostics[0].message, "test index allocation failed");

    let mut values = Vec::<usize>::new();
    let error = reserve_vec(&mut values, usize::MAX, "$.test", "test vector")
        .expect_err("impossible capacity must fail");
    assert!(values.is_empty());
    assert_eq!(error.diagnostics[0].code, "resource_limit");
    assert_eq!(
        error.diagnostics[0].message,
        "test vector allocation failed"
    );
}

#[test]
fn output_surface_and_docs_keep_deferred_work_outside_this_stage() {
    let projection = direct_projection(
        vec![parameter("gain", Vec::new(), "gain")],
        vec![connector("signal", Vec::new(), "signal")],
    );
    let result = project_declaration_requirements(&projection).expect("projection succeeds");
    let DeclarationRequirementProjection {
        canonical_id,
        revision,
        mut parameters,
        mut connectors,
    } = result;
    let ParameterDeclarationRequirement {
        parameter_id,
        canonical_class_path,
        source_member,
        snapshot,
        revision: source_revision,
        file,
        scalar_names,
    } = parameters.remove(0);
    let ConnectorDeclarationRequirement {
        connector_id,
        canonical_class_path: connector_class,
        source_member: connector_member,
        snapshot: connector_snapshot,
        revision: connector_revision,
        file: connector_file,
        scalar_names: connector_names,
    } = connectors.remove(0);

    assert_eq!(canonical_id, "typed-boundary-id");
    assert_eq!(revision, BigInt::from(10_u8).pow(80));
    assert_eq!(parameter_id, "gain");
    assert_eq!(canonical_class_path, TRIM_CLASS);
    assert_eq!(source_member, "gain");
    assert_eq!(snapshot, SourceSnapshotRole::Release);
    assert_eq!(source_revision, RELEASE_REVISION);
    assert_eq!(file, locator(TRIM_PATH, TRIM_BLOB));
    assert_eq!(scalar_names.len(), 1);
    assert_eq!(connector_id, "signal");
    assert_eq!(connector_class, TRIM_CLASS);
    assert_eq!(connector_member, "signal");
    assert_eq!(connector_snapshot, SourceSnapshotRole::Release);
    assert_eq!(connector_revision, RELEASE_REVISION);
    assert_eq!(connector_file, locator(TRIM_PATH, TRIM_BLOB));
    assert_eq!(connector_names.len(), 1);

    let source_text = include_str!("../declaration_requirements.rs");
    for required_doc_text in [
        "remain caller claims",
        "no inventory recheck",
        "file access",
        "source parsing",
        "declaration verification",
        "serialization",
        "not a public interchange",
        "persisted contract",
    ] {
        assert!(
            source_text.contains(required_doc_text),
            "module docs must mention {required_doc_text}"
        );
    }
    for forbidden in [
        "BoundScalar",
        "ScalarAbiType",
        "declaration_verified",
        "source_index",
        "enum_locator",
        "use serde",
        "serde_json",
        "cxf_json",
        "std::fs",
        "std::io",
        "std::net",
        "std::process",
        "Command::",
        "Engine",
        "Studio",
    ] {
        assert!(
            !source_text.contains(forbidden),
            "declaration requirements must not use {forbidden}"
        );
    }
}

#[test]
fn grouping_and_materialization_use_indexes_without_repeated_row_scans() {
    let source_text = include_str!("../declaration_requirements.rs");
    let validation = source_text
        .split_once("fn validate_namespace")
        .expect("validation helper exists")
        .1
        .split_once("fn validate_cross_kind_collisions")
        .expect("cross-kind validation follows namespace validation")
        .0;
    assert_eq!(
        validation
            .matches("for (index, row) in rows.iter().enumerate()")
            .count(),
        1
    );
    assert!(validation.contains("owner_indexes.get(owner_id)"));
    assert!(!validation.contains("rows.iter().find"));
    assert!(!validation.contains("rows.iter().filter"));

    let materialization = source_text
        .split_once("fn materialize_parameters")
        .expect("materialization helper exists")
        .1
        .split_once("pub fn project_declaration_requirements")
        .expect("public projection follows materialization")
        .0;
    assert!(materialization.contains("for group in groups"));
    assert!(!materialization.contains(".find("));
    assert!(!materialization.contains(".filter("));

    let projection = source_text
        .split_once("pub fn project_declaration_requirements")
        .expect("public projection exists")
        .1;
    let refusal = projection
        .find("if !diagnostics.is_empty()")
        .expect("validation refusal exists");
    let materialize = projection
        .find("materialize_parameters")
        .expect("materialization call exists");
    assert!(refusal < materialize);
}
