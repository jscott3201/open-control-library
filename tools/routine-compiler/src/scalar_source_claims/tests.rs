use std::collections::HashSet;
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
    EnumAbiMapping, EnumAbiMemberMapping, ScalarAbiType, ScalarAbiValue, project_scalar_abi,
};
use crate::scalar_names::{
    NamedScalarConnectorRow, NamedScalarParameterRow, allocate_scalar_names,
};

// These identities are checked-in fixture constants. Tests construct typed
// inventories directly and never read the inventory or pin files.
const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const TRIM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";
const TRIM_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const TRIM_BLOB: &str = "sha1:028439a4fb478fc041d703a092d5186f5861eb03";
const TRIM_SHA256: &str = "sha256:1bf9ab68904baa00553d6b43ecd0d04411e6106d196881ad67c9885827507981";
const TIME_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TimeSuppression";
const TIME_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TimeSuppression.mo";
const TIME_BLOB: &str = "sha1:0c398e16de20f3f8ac9ff1ad95f8b1e8c0e0a2d1";
const TIME_SHA256: &str = "sha256:b37611c86d55c3508c9991c7d9e35c4059b7a596421c5888a3217cbc40e55b62";
const DEVELOPMENT_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Plants.Chillers.Controller";
const DEVELOPMENT_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Plants/Chillers/Controller.mo";
const DEVELOPMENT_BLOB: &str = "sha1:8948cbcf1642d3456dece92832dc1cc2eb6f6fe7";
const DEVELOPMENT_SHA256: &str =
    "sha256:5a064a0904f470ccd18bb799febaa34cc94f425e93f874e5a9b3dac5cba485e3";

fn integer(value: usize) -> BigInt {
    BigInt::from(value)
}

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("fixture real is finite")
}

fn inventory_file(
    path: &str,
    bytes: usize,
    git_blob_sha1: &str,
    sha256: &str,
) -> SourceInventoryFile {
    SourceInventoryFile {
        path: path.to_owned(),
        mode: "100644".to_owned(),
        bytes: integer(bytes),
        git_blob_sha1: git_blob_sha1.to_owned(),
        sha256: sha256.to_owned(),
    }
}

fn source_inventory() -> SourceInventory {
    SourceInventory {
        schema: INVENTORY_SCHEMA.to_owned(),
        repository: UPSTREAM_REPOSITORY.to_owned(),
        source_root: SOURCE_ROOT.to_owned(),
        inventory_scope: INVENTORY_SCOPE.to_owned(),
        dependency_closure: DEPENDENCY_CLOSURE.to_owned(),
        license: SourceInventoryLicense {
            upstream_path: LICENSE_UPSTREAM_PATH.to_owned(),
            retained_path: LICENSE_RETAINED_PATH.to_owned(),
            git_blob_sha1: "sha1:b542af56c10d3769d42a79ad45d78329ad2dbd5f".to_owned(),
            sha256: "sha256:79ad0f2d053b92d93a7e7b200b9d7ef4c1bb2097aad695cda31ad9a23a57e921"
                .to_owned(),
        },
        snapshots: vec![
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                root_tree_sha1: "sha1:8bc765b009ed0dcf6907021f073ea77160088bd8".to_owned(),
                file_count: integer(2),
                total_bytes: integer(39_600),
                modelica_file_count: integer(2),
                package_order_count: integer(0),
                files: vec![
                    inventory_file(TIME_PATH, 14_148, TIME_BLOB, TIME_SHA256),
                    inventory_file(TRIM_PATH, 25_452, TRIM_BLOB, TRIM_SHA256),
                ],
            },
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
                root_tree_sha1: "sha1:6286a3d534846a187444d3b4bee72a1ae944a458".to_owned(),
                file_count: integer(1),
                total_bytes: integer(153_219),
                modelica_file_count: integer(1),
                package_order_count: integer(0),
                files: vec![inventory_file(
                    DEVELOPMENT_PATH,
                    153_219,
                    DEVELOPMENT_BLOB,
                    DEVELOPMENT_SHA256,
                )],
            },
        ],
    }
}

fn source_pins() -> Vec<SourcePin> {
    vec![
        SourcePin {
            role: SourceSnapshotRole::Release,
            revision: RELEASE_REVISION.to_owned(),
        },
        SourcePin {
            role: SourceSnapshotRole::Development,
            revision: DEVELOPMENT_REVISION.to_owned(),
        },
    ]
}

fn locator(path: &str, blob: &str) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: blob.to_owned(),
    }
}

fn class_claim(
    canonical_class_path: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    path: &str,
    blob: &str,
) -> SourceClassClaim {
    SourceClassClaim {
        canonical_class_path: canonical_class_path.to_owned(),
        snapshot,
        revision: revision.to_owned(),
        file: locator(path, blob),
    }
}

fn trim_claim() -> SourceClassClaim {
    class_claim(
        TRIM_CLASS,
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
        TRIM_BLOB,
    )
}

fn binding(
    owner_kind: SourceOwnerKind,
    owner_id: &str,
    canonical_class_path: &str,
    source_member: &str,
) -> SourceMemberBinding {
    SourceMemberBinding {
        owner_kind,
        owner_id: owner_id.to_owned(),
        canonical_class_path: canonical_class_path.to_owned(),
        source_member: source_member.to_owned(),
    }
}

fn coordinate(dimension_id: &str, member_id: &str, ordinal: usize) -> ScalarCoordinate {
    ScalarCoordinate {
        dimension_id: dimension_id.to_owned(),
        member_id: member_id.to_owned(),
        ordinal,
    }
}

fn named_parameter(
    parameter_id: &str,
    scalar_name: &str,
    coordinates: Vec<ScalarCoordinate>,
) -> NamedScalarParameterRow {
    NamedScalarParameterRow {
        scalar_name: scalar_name.to_owned(),
        parameter_id: parameter_id.to_owned(),
        coordinates,
        abi_type: ScalarAbiType::Primitive(PrimitiveType::Real),
        source: ParameterSource::Default,
        value: ScalarAbiValue::Real(finite(1.0)),
    }
}

fn named_connector(
    connector_id: &str,
    scalar_name: &str,
    coordinates: Vec<ScalarCoordinate>,
) -> NamedScalarConnectorRow {
    NamedScalarConnectorRow {
        scalar_name: scalar_name.to_owned(),
        connector_id: connector_id.to_owned(),
        coordinates,
        abi_type: ScalarAbiType::Primitive(PrimitiveType::Real),
        direction: ConnectorDirection::Input,
    }
}

fn direct_named_projection() -> NamedScalarProjection {
    NamedScalarProjection {
        canonical_id: "G36-05-16-RUST-SOURCE-CLAIM-TEST".to_owned(),
        revision: integer(1),
        parameters: vec![named_parameter("setting", "p_setting", Vec::new())],
        connectors: vec![named_connector("signal", "c_signal", Vec::new())],
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Fixture {
    named: NamedScalarProjection,
    inventory: SourceInventory,
    pins: Vec<SourcePin>,
    claims: Vec<SourceClassClaim>,
    bindings: Vec<SourceMemberBinding>,
}

impl Fixture {
    fn project(&self) -> Result<ScalarSourceClaimProjection, SourceClaimError> {
        project_scalar_source_claims(
            &self.named,
            &self.inventory,
            &self.pins,
            &self.claims,
            &self.bindings,
        )
    }
}

fn valid_fixture() -> Fixture {
    Fixture {
        named: direct_named_projection(),
        inventory: source_inventory(),
        pins: source_pins(),
        claims: vec![trim_claim()],
        bindings: vec![
            binding(
                SourceOwnerKind::Parameter,
                "setting",
                TRIM_CLASS,
                "samplePeriod",
            ),
            binding(SourceOwnerKind::Connector, "signal", TRIM_CLASS, "y"),
        ],
    }
}

fn error_codes(error: &SourceClaimError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn assert_error_code(fixture: &Fixture, expected_code: &str) -> SourceClaimError {
    let mut attempts = Vec::new();
    for _ in 0..2 {
        let outcome = catch_unwind(AssertUnwindSafe(|| fixture.project()));
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

fn real(value: f64) -> ScalarValue {
    ScalarValue::Real(finite(value))
}

fn full_chain_input() -> ValidatedResolutionInput {
    let huge_revision = BigInt::from(10_u8).pow(80);
    ValidatedResolutionInput {
        canonical_id: "G36-05-16-RUST-SOURCE-CHAIN".to_owned(),
        revision: huge_revision,
        types: vec![TypeDefinition {
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
        }],
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
                value: ParameterValue::Scalar(real(60.0)),
            },
            ParameterDefinition {
                parameter_id: "gains".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank1 {
                    dimension_id: "pair".to_owned(),
                },
                source: ParameterSource::Default,
                value: ParameterValue::Rank1(vec![real(1.0), real(2.0)]),
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
                    vec![real(1.0), real(2.0)],
                    vec![real(3.0), real(4.0)],
                ]),
            },
            ParameterDefinition {
                parameter_id: "initial_mode".to_owned(),
                type_use: TypeUse::Named("operating_mode".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Default,
                value: ParameterValue::Scalar(ScalarValue::Enum(EnumInputValue {
                    type_id: "operating_mode".to_owned(),
                    member_id: "occupied".to_owned(),
                })),
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
    }
}

fn enum_mapping() -> EnumAbiMapping {
    EnumAbiMapping {
        type_id: "operating_mode".to_owned(),
        canonical_class_path: "Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes".to_owned(),
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
    }
}

fn full_chain_fixture() -> Fixture {
    let resolved = resolve_validated(&full_chain_input(), ResolutionLimits::default())
        .expect("typed fixture resolves");
    let abi = project_scalar_abi(&resolved, &[enum_mapping()]).expect("ABI projection succeeds");
    let named = allocate_scalar_names(&abi).expect("name allocation succeeds");
    Fixture {
        named,
        inventory: source_inventory(),
        pins: source_pins(),
        claims: vec![trim_claim()],
        bindings: vec![
            binding(
                SourceOwnerKind::Parameter,
                "sample_period_s",
                TRIM_CLASS,
                "samplePeriod",
            ),
            binding(SourceOwnerKind::Parameter, "gains", TRIM_CLASS, "gains"),
            binding(
                SourceOwnerKind::Parameter,
                "matrix_weights",
                TRIM_CLASS,
                "matrixWeights",
            ),
            binding(
                SourceOwnerKind::Parameter,
                "initial_mode",
                TRIM_CLASS,
                "initialMode",
            ),
            binding(
                SourceOwnerKind::Connector,
                "requests",
                TRIM_CLASS,
                "numOfReq",
            ),
            binding(
                SourceOwnerKind::Connector,
                "matrix_feedback",
                TRIM_CLASS,
                "matrixFeedback",
            ),
        ],
    }
}

#[test]
fn full_typed_chain_preserves_scalar_vector_matrix_order_and_owner_claims() {
    let fixture = full_chain_fixture();
    let result = fixture.project().expect("full typed join succeeds");

    assert_eq!(result.canonical_id, fixture.named.canonical_id);
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
            "requests",
            "requests",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
        ]
    );
    for (source, named) in result.parameters.iter().zip(&fixture.named.parameters) {
        assert_eq!(source.scalar_name, named.scalar_name);
        assert_eq!(source.coordinates, named.coordinates);
        assert_eq!(source.canonical_class_path, TRIM_CLASS);
        assert_eq!(source.file.path, TRIM_PATH);
    }
    for (source, named) in result.connectors.iter().zip(&fixture.named.connectors) {
        assert_eq!(source.scalar_name, named.scalar_name);
        assert_eq!(source.coordinates, named.coordinates);
        assert_eq!(source.canonical_class_path, TRIM_CLASS);
    }
    assert_eq!(
        result
            .parameters
            .iter()
            .filter(|row| row.parameter_id == "gains")
            .map(|row| row.source_member.as_str())
            .collect::<Vec<_>>(),
        vec!["gains", "gains"]
    );
    assert_eq!(
        result
            .parameters
            .iter()
            .filter(|row| row.parameter_id == "matrix_weights")
            .map(|row| {
                row.coordinates
                    .iter()
                    .map(|coordinate| coordinate.member_id.as_str())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![
            vec!["north", "first"],
            vec!["north", "second"],
            vec!["south", "first"],
            vec!["south", "second"],
        ]
    );
    let named_enum = fixture
        .named
        .parameters
        .iter()
        .find(|row| row.parameter_id == "initial_mode")
        .expect("enum parameter exists");
    assert_eq!(
        named_enum.abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: "Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes"
                .to_owned()
        }
    );
    let source_enum = result
        .parameters
        .iter()
        .find(|row| row.parameter_id == "initial_mode")
        .expect("source enum row exists");
    assert_eq!(source_enum.canonical_class_path, TRIM_CLASS);
    assert_eq!(source_enum.source_member, "initialMode");
}

#[test]
fn snapshot_membership_is_role_isolated_without_fallback() {
    let mut fixture = valid_fixture();
    fixture.named.connectors.clear();
    fixture.bindings.truncate(1);
    fixture.claims = vec![class_claim(
        DEVELOPMENT_CLASS,
        SourceSnapshotRole::Development,
        DEVELOPMENT_REVISION,
        DEVELOPMENT_PATH,
        DEVELOPMENT_BLOB,
    )];
    fixture.bindings[0].canonical_class_path = DEVELOPMENT_CLASS.to_owned();
    let result = fixture.project().expect("development locator is accepted");
    assert_eq!(
        result.parameters[0].snapshot,
        SourceSnapshotRole::Development
    );
    assert_eq!(result.parameters[0].file.path, DEVELOPMENT_PATH);

    fixture.claims[0].snapshot = SourceSnapshotRole::Release;
    fixture.claims[0].revision = RELEASE_REVISION.to_owned();
    assert_error_code(&fixture, "absent_file_locator");
}

#[test]
fn source_pin_shape_roles_revisions_and_distinctness_fail_closed() {
    let mut cases = Vec::new();

    let mut fixture = valid_fixture();
    fixture.pins.pop();
    cases.push((fixture, "missing_source_pin"));

    let mut fixture = valid_fixture();
    fixture.pins[1].role = SourceSnapshotRole::Release;
    cases.push((fixture, "duplicate_source_pin"));

    let mut fixture = valid_fixture();
    fixture.pins[1].revision = RELEASE_REVISION.to_owned();
    cases.push((fixture, "duplicate_source_revision"));

    let mut fixture = valid_fixture();
    fixture.pins[0].revision = RELEASE_REVISION.to_ascii_uppercase();
    cases.push((fixture, "invalid_source_revision"));

    let mut fixture = valid_fixture();
    fixture.pins[0].revision = "0".repeat(40);
    cases.push((fixture, "inventory_snapshot_revision"));

    for (fixture, code) in cases {
        assert_error_code(&fixture, code);
    }
}

#[test]
fn inventory_constants_license_snapshot_files_order_hashes_and_counts_are_checked() {
    let mut cases = Vec::new();

    let mut fixture = valid_fixture();
    fixture.inventory.schema = "wrong".to_owned();
    cases.push((fixture, "inventory_constant"));

    let mut fixture = valid_fixture();
    fixture.inventory.license.retained_path = "wrong".to_owned();
    cases.push((fixture, "invalid_inventory_license"));

    let mut fixture = valid_fixture();
    fixture.inventory.license.sha256 = "SHA256:bad".to_owned();
    cases.push((fixture, "invalid_inventory_license"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots.pop();
    cases.push((fixture, "invalid_inventory_snapshots"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots.swap(0, 1);
    cases.push((fixture, "inventory_snapshot_role"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].revision = "0".repeat(40);
    cases.push((fixture, "inventory_snapshot_revision"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].root_tree_sha1 = "sha1:BAD".to_owned();
    cases.push((fixture, "invalid_inventory_snapshot"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].files[0].mode = "100755".to_owned();
    cases.push((fixture, "invalid_inventory_file"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].files[0].bytes = BigInt::from(-1_i8);
    cases.push((fixture, "invalid_inventory_file"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].files[0].git_blob_sha1 = "SHA1:bad".to_owned();
    cases.push((fixture, "invalid_inventory_blob"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].files[0].sha256 = "sha256:BAD".to_owned();
    cases.push((fixture, "invalid_inventory_file"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].files.reverse();
    cases.push((fixture, "inventory_file_order"));

    let mut fixture = valid_fixture();
    let duplicate = fixture.inventory.snapshots[0].files[0].clone();
    fixture.inventory.snapshots[0].files.insert(1, duplicate);
    cases.push((fixture, "duplicate_inventory_path"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].file_count += BigInt::from(1_u8);
    cases.push((fixture, "inventory_count_mismatch"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].total_bytes += BigInt::from(1_u8);
    cases.push((fixture, "inventory_count_mismatch"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].modelica_file_count = BigInt::from(-1_i8);
    cases.push((fixture, "invalid_inventory_count"));

    let mut fixture = valid_fixture();
    fixture.inventory.snapshots[0].package_order_count = BigInt::from(1_u8);
    cases.push((fixture, "inventory_count_mismatch"));

    for (fixture, code) in cases {
        assert_error_code(&fixture, code);
    }
}

#[test]
fn hostile_inventory_and_locator_paths_are_rejected() {
    let hostile = [
        "",
        "/Buildings/Controls/OBC/ASHRAE/G36/Bad.mo",
        "Buildings\\Controls\\OBC\\ASHRAE\\G36\\Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/Bad\nName.mo",
        "Buildings/Controls/OBC/ASHRAE/G36//Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/./Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/../Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/Other/Bad.mo",
    ];
    for path in hostile {
        let mut fixture = valid_fixture();
        fixture.inventory.snapshots[0].files[0].path = path.to_owned();
        assert_error_code(&fixture, "unsafe_inventory_path");

        let mut fixture = valid_fixture();
        fixture.claims[0].file.path = path.to_owned();
        assert_error_code(&fixture, "unsafe_class_path");
    }
}

#[test]
fn class_paths_and_source_members_enforce_ascii_modelica_bounds() {
    let class_paths = [
        "",
        "Buildings.Controls.OBC.ASHRAE.G36",
        "Buildings.Controls.OBC.ASHRAE.G36..Bad",
        "Buildings.Controls.OBC.ASHRAE.G36.9Bad",
        "Buildings.Controls.OBC.ASHRAE.G36.Café",
        "Buildings.Controls.OBC.CDL.Reals.Add",
    ];
    for class_path in class_paths {
        let mut fixture = valid_fixture();
        fixture.claims[0].canonical_class_path = class_path.to_owned();
        assert_error_code(&fixture, "invalid_class_path");
    }

    let mut fixture = valid_fixture();
    fixture.claims[0].canonical_class_path = format!("{CLASS_PATH_PREFIX}{}", "A".repeat(256));
    assert_error_code(&fixture, "invalid_class_path");

    let mut fixture = valid_fixture();
    fixture.claims[0].canonical_class_path = format!("{CLASS_PATH_PREFIX}{}", "A.".repeat(600));
    assert_error_code(&fixture, "invalid_class_path");

    for source_member in ["", "9bad", "u.Hol", "mø", &"u".repeat(256)] {
        let mut fixture = valid_fixture();
        fixture.bindings[0].source_member = source_member.to_owned();
        assert_error_code(&fixture, "invalid_source_member");
    }
}

#[test]
fn class_claim_revision_membership_duplicates_ambiguity_and_usage_are_checked() {
    let mut fixture = valid_fixture();
    fixture.claims[0].revision = "bad".to_owned();
    assert_error_code(&fixture, "invalid_class_revision");

    let mut fixture = valid_fixture();
    fixture.claims[0].revision = "0".repeat(40);
    assert_error_code(&fixture, "class_revision_mismatch");

    let mut fixture = valid_fixture();
    fixture.claims[0].file.path = TIME_PATH.trim_end_matches(".mo").to_owned();
    assert_error_code(&fixture, "non_modelica_locator");

    let mut fixture = valid_fixture();
    fixture.claims[0].file.path = "Buildings/Controls/OBC/ASHRAE/G36/Generic/Absent.mo".to_owned();
    fixture.claims[0].file.git_blob_sha1 = format!("sha1:{}", "0".repeat(40));
    assert_error_code(&fixture, "absent_file_locator");

    let mut fixture = valid_fixture();
    fixture.claims[0].file.git_blob_sha1 = format!("sha1:{}", "0".repeat(40));
    assert_error_code(&fixture, "file_blob_mismatch");

    let mut fixture = valid_fixture();
    fixture.claims.push(fixture.claims[0].clone());
    let error = assert_error_code(&fixture, "duplicate_class_claim");
    let codes = error_codes(&error);
    assert!(codes.contains("duplicate_file_locator"));
    assert!(codes.contains("ambiguous_class_claim"));

    let mut fixture = valid_fixture();
    fixture.claims.clear();
    assert_error_code(&fixture, "missing_class_claim");

    let mut fixture = valid_fixture();
    fixture.claims.push(class_claim(
        TIME_CLASS,
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TIME_PATH,
        TIME_BLOB,
    ));
    let error = assert_error_code(&fixture, "unused_class_claim");
    assert!(error_codes(&error).contains("extra_file_locator"));

    let mut fixture = valid_fixture();
    let alias_class = "Buildings.Controls.OBC.ASHRAE.G36.Generic.CallerAlias";
    fixture.claims.push(class_claim(
        alias_class,
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
        TRIM_BLOB,
    ));
    fixture.bindings[1].canonical_class_path = alias_class.to_owned();
    assert_error_code(&fixture, "duplicate_file_locator");
}

#[test]
fn bindings_are_complete_unique_namespace_safe_and_source_key_unique() {
    let mut fixture = valid_fixture();
    fixture.bindings.pop();
    assert_error_code(&fixture, "missing_member_binding");

    let mut fixture = valid_fixture();
    fixture.bindings.push(binding(
        SourceOwnerKind::Connector,
        "extra",
        TRIM_CLASS,
        "extraMember",
    ));
    assert_error_code(&fixture, "extra_member_binding");

    let mut fixture = valid_fixture();
    fixture.bindings.push(fixture.bindings[0].clone());
    assert_error_code(&fixture, "duplicate_member_binding");

    let mut fixture = valid_fixture();
    fixture.bindings[1].owner_kind = SourceOwnerKind::Parameter;
    assert_error_code(&fixture, "cross_namespace_binding");

    let mut fixture = valid_fixture();
    fixture
        .named
        .parameters
        .push(named_parameter("other_setting", "p_other", Vec::new()));
    fixture.bindings.push(binding(
        SourceOwnerKind::Parameter,
        "other_setting",
        TRIM_CLASS,
        "samplePeriod",
    ));
    assert_error_code(&fixture, "duplicate_source_key");
}

#[test]
fn manual_named_projection_checks_only_naming_relevant_fields() {
    let mut cases = Vec::new();

    let mut fixture = valid_fixture();
    fixture.named.canonical_id.clear();
    cases.push((fixture, "invalid_named_metadata"));

    let mut fixture = valid_fixture();
    fixture.named.revision = BigInt::from(0_u8);
    cases.push((fixture, "invalid_named_metadata"));

    let mut fixture = valid_fixture();
    fixture.named.parameters[0].parameter_id.clear();
    cases.push((fixture, "invalid_owner_id"));

    let mut fixture = valid_fixture();
    fixture.named.parameters[0].scalar_name.clear();
    cases.push((fixture, "invalid_scalar_name"));

    let mut fixture = valid_fixture();
    fixture.named.parameters[0].scalar_name = "c_wrong".to_owned();
    cases.push((fixture, "scalar_name_namespace"));

    let mut fixture = valid_fixture();
    fixture.named.parameters[0].coordinates = vec![coordinate("", "", usize::MAX)];
    cases.push((fixture, "invalid_dimension_id"));

    let mut fixture = valid_fixture();
    fixture
        .named
        .parameters
        .push(fixture.named.parameters[0].clone());
    cases.push((fixture, "duplicate_scalar_name"));

    for (fixture, code) in cases {
        assert_error_code(&fixture, code);
    }

    let mut fixture = valid_fixture();
    fixture.named.parameters[0].abi_type = ScalarAbiType::Alias {
        type_id: "temperature".to_owned(),
        primitive: PrimitiveType::Real,
        quantity: Some(String::new()),
        unit: Some(" untrimmed ".to_owned()),
        display_unit: None,
    };
    fixture.named.parameters[0].value = ScalarAbiValue::Real(finite(1.0));
    fixture
        .project()
        .expect("deferred optional alias metadata is outside this validation boundary");
}

#[test]
fn malformed_inputs_return_sorted_repeatable_atomic_errors_without_panicking() {
    let mut fixture = valid_fixture();
    fixture.pins.clear();
    fixture.inventory.schema.clear();
    fixture.inventory.snapshots[0].files[0].path = "../bad".to_owned();
    fixture.named.canonical_id.clear();
    fixture.named.parameters[0].scalar_name = "bad".to_owned();
    fixture.claims[0].canonical_class_path = "bad".to_owned();
    fixture.bindings[0].owner_id.clear();
    fixture.bindings[1].source_member = "bad.member".to_owned();

    let error = assert_error_code(&fixture, "inventory_constant");
    let codes = error_codes(&error);
    for expected in [
        "missing_source_pin",
        "unsafe_inventory_path",
        "invalid_named_metadata",
        "scalar_name_namespace",
        "invalid_class_path",
        "invalid_owner_id",
        "invalid_source_member",
    ] {
        assert!(codes.contains(expected), "missing {expected} in {error:?}");
    }
}

#[test]
fn projection_is_order_independent_detached_and_does_not_mutate_inputs() {
    let mut fixture = full_chain_fixture();
    let before = fixture.clone();
    let first = fixture.project().expect("projection succeeds");
    assert_eq!(fixture, before);

    fixture.pins.reverse();
    fixture.bindings.reverse();
    let reordered = fixture
        .project()
        .expect("claim input order is non-semantic");
    assert_eq!(first, reordered);

    fixture.named.canonical_id = "changed".to_owned();
    fixture.named.revision = BigInt::from(1_u8);
    fixture.named.parameters[1].scalar_name = "p_changed".to_owned();
    fixture.named.parameters[1].coordinates[0].member_id = "changed".to_owned();
    fixture.inventory.snapshots[0].files[1].git_blob_sha1 = format!("sha1:{}", "0".repeat(40));
    fixture.pins[0].revision = "0".repeat(40);
    fixture.claims[0].canonical_class_path = TIME_CLASS.to_owned();
    fixture.claims[0].file.path = TIME_PATH.to_owned();
    fixture.bindings[0].source_member = "changed".to_owned();

    assert_eq!(first.canonical_id, "G36-05-16-RUST-SOURCE-CHAIN");
    assert_eq!(first.revision, BigInt::from(10_u8).pow(80));
    assert_ne!(first.parameters[1].scalar_name, "p_changed");
    assert_eq!(first.parameters[1].coordinates[0].member_id, "first");
    assert_eq!(first.parameters[0].canonical_class_path, TRIM_CLASS);
    assert_eq!(first.parameters[0].file.path, TRIM_PATH);
    assert_eq!(first.parameters[0].file.git_blob_sha1, TRIM_BLOB);
}

#[test]
fn forward_and_derived_reverse_lookups_handle_known_and_unknown_keys() {
    let mut fixture = valid_fixture();
    fixture.bindings[0].source_member = "sharedMember".to_owned();
    fixture.bindings[1].source_member = "sharedMember".to_owned();
    let result = fixture.project().expect("projection succeeds");

    assert_eq!(
        result.claim_for_scalar(&result.parameters[0].scalar_name),
        Some(ScalarSourceClaimRef::Parameter(&result.parameters[0]))
    );
    assert_eq!(
        result.claim_for_scalar(&result.connectors[0].scalar_name),
        Some(ScalarSourceClaimRef::Connector(&result.connectors[0]))
    );
    assert!(result.claim_for_scalar("missing").is_none());
    assert_eq!(
        result
            .scalar_names_for_source(SourceOwnerKind::Parameter, TRIM_CLASS, "sharedMember")
            .expect("parameter source key exists")
            .collect::<Vec<_>>(),
        vec![result.parameters[0].scalar_name.as_str()]
    );
    assert_eq!(
        result
            .scalar_names_for_source(SourceOwnerKind::Connector, TRIM_CLASS, "sharedMember")
            .expect("connector source key exists")
            .collect::<Vec<_>>(),
        vec![result.connectors[0].scalar_name.as_str()]
    );
    assert!(
        result
            .scalar_names_for_source(SourceOwnerKind::Parameter, TRIM_CLASS, "missing")
            .is_none()
    );
    assert!(
        result
            .scalar_names_for_source(SourceOwnerKind::Connector, TIME_CLASS, "sharedMember")
            .is_none()
    );
}

#[test]
fn output_surface_contains_claims_without_abi_evidence_persistence_or_runtime_payload() {
    let result = valid_fixture().project().expect("projection succeeds");
    let ScalarSourceClaimProjection {
        canonical_id,
        revision,
        mut parameters,
        mut connectors,
    } = result;
    let ScalarParameterSourceClaim {
        scalar_name: parameter_name,
        parameter_id,
        coordinates: parameter_coordinates,
        canonical_class_path: parameter_class,
        source_member: parameter_member,
        snapshot: parameter_snapshot,
        revision: parameter_revision,
        file: parameter_file,
    } = parameters.remove(0);
    let ScalarConnectorSourceClaim {
        scalar_name: connector_name,
        connector_id,
        coordinates: connector_coordinates,
        canonical_class_path: connector_class,
        source_member: connector_member,
        snapshot: connector_snapshot,
        revision: connector_revision,
        file: connector_file,
    } = connectors.remove(0);

    assert_eq!(canonical_id, "G36-05-16-RUST-SOURCE-CLAIM-TEST");
    assert_eq!(revision, integer(1));
    assert_eq!(parameter_name, "p_setting");
    assert_eq!(parameter_id, "setting");
    assert!(parameter_coordinates.is_empty());
    assert_eq!(parameter_class, TRIM_CLASS);
    assert_eq!(parameter_member, "samplePeriod");
    assert_eq!(parameter_snapshot, SourceSnapshotRole::Release);
    assert_eq!(parameter_revision, RELEASE_REVISION);
    assert_eq!(parameter_file, locator(TRIM_PATH, TRIM_BLOB));
    assert_eq!(connector_name, "c_signal");
    assert_eq!(connector_id, "signal");
    assert!(connector_coordinates.is_empty());
    assert_eq!(connector_class, TRIM_CLASS);
    assert_eq!(connector_member, "y");
    assert_eq!(connector_snapshot, SourceSnapshotRole::Release);
    assert_eq!(connector_revision, RELEASE_REVISION);
    assert_eq!(connector_file, locator(TRIM_PATH, TRIM_BLOB));
}

#[test]
fn resource_helpers_fail_without_giant_allocations() {
    assert_eq!(checked_total_count([1_usize, 2, 3]), Some(6));
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
