use std::collections::HashSet;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cap_std::{ambient_authority, fs::Dir};
use num_bigint::BigInt;
use sha1::{Digest, Sha1};

use super::*;
use crate::declaration_pipeline::{DeclarationPipelineError, DeclarationRootPipelineError};
use crate::declaration_requirements::project_declaration_requirements;
use crate::declaration_source::{
    DeclarationSourceDiagnostic, DeclarationSourceError, DeclarationSourceLimits,
    read_declaration_sources,
};
use crate::resolution::{
    ComparisonOperator, ConnectorDefinition, ConnectorDirection, ConnectorPresence,
    DimensionDefinition, DimensionKind, EnumInputValue, EnumMemberDefinition, FiniteReal, Guard,
    GuardOperand, NamedTypeDefinition, ParameterDefinition, ParameterSource, ParameterValue,
    PrimitiveType, ResolutionError, ScalarValue, Shape, TypeDefinition, TypeUse,
};
use crate::scalar_abi::{EnumAbiMemberMapping, ScalarAbiDiagnostic, ScalarAbiType, ScalarAbiValue};
use crate::scalar_names::{ScalarNameDiagnostic, ScalarNameError};
use crate::scalar_source_claims::{
    SourceClaimDiagnostic, SourceFileLocator, SourceInventoryFile, SourceInventoryLicense,
    SourceInventorySnapshot, SourceOwnerKind, SourceSnapshotRole,
};

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const PARAMETER_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.ParameterOwner";
const PARAMETER_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/ParameterOwner.mo";
const CONNECTOR_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.ConnectorOwner";
const CONNECTOR_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/ConnectorOwner.mo";
const ENUM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes";

const PARAMETER_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block ParameterOwner
  parameter Real samplePeriod;
  parameter Real gains[2];
  parameter Real matrixWeights[2, 2];
  parameter Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes initialMode;
  parameter Boolean enableOptional;
end ParameterOwner;
"#;

const CONNECTOR_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block ConnectorOwner
  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput requests[2];
  Buildings.Controls.OBC.CDL.Interfaces.RealOutput matrixFeedback[2, 2];
end ConnectorOwner;
"#;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ocl-compiler-pipeline-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("fixture directory is unique");
        Self { path }
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().expect("fixture path has a parent"))
            .expect("fixture parent builds");
        fs::write(path, bytes).expect("fixture source writes");
    }

    fn read(&self, relative: &str) -> Vec<u8> {
        fs::read(self.path.join(relative)).expect("fixture source reads")
    }

    fn open(&self) -> Dir {
        Dir::open_ambient_dir(&self.path, ambient_authority()).expect("fixture root opens")
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Fixture {
    resolution_input: ValidatedResolutionInput,
    enum_mappings: Vec<EnumAbiMapping>,
    inventory: SourceInventory,
    pins: Vec<SourcePin>,
    claims: Vec<SourceClassClaim>,
    bindings: Vec<SourceMemberBinding>,
}

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("fixture real is finite")
}

fn real(value: f64) -> ScalarValue {
    ScalarValue::Real(finite(value))
}

fn git_blob_sha1(source: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", source.len()).as_bytes());
    hasher.update(source);
    format!("sha1:{:x}", hasher.finalize())
}

fn inventory_file(path: &str, source: &[u8]) -> SourceInventoryFile {
    SourceInventoryFile {
        path: path.to_owned(),
        mode: "100644".to_owned(),
        bytes: BigInt::from(source.len()),
        git_blob_sha1: git_blob_sha1(source),
        sha256: format!("sha256:{}", "1".repeat(64)),
    }
}

fn locator(path: &str, source: &[u8]) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: git_blob_sha1(source),
    }
}

fn resolution_input() -> ValidatedResolutionInput {
    ValidatedResolutionInput {
        canonical_id: "G36-RUST-TYPED-COMPILER-PIPELINE".to_owned(),
        revision: BigInt::from(10_u8).pow(80),
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
                source: ParameterSource::Assignment,
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
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Enum(EnumInputValue {
                    type_id: "operating_mode".to_owned(),
                    member_id: "occupied".to_owned(),
                })),
            },
            ParameterDefinition {
                parameter_id: "enable_optional".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Boolean(false)),
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
            ConnectorDefinition {
                connector_id: "optional_signal".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Guarded(Guard::Compare {
                    operator: ComparisonOperator::Eq,
                    left: GuardOperand::Parameter("enable_optional".to_owned()),
                    right: GuardOperand::Literal {
                        type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                        value: ScalarValue::Boolean(true),
                    },
                }),
            },
        ],
    }
}

fn enum_mapping() -> EnumAbiMapping {
    EnumAbiMapping {
        type_id: "operating_mode".to_owned(),
        canonical_class_path: ENUM_CLASS.to_owned(),
        source_members: vec!["Unoccupied".to_owned(), "Occupied".to_owned()],
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

fn fixture(parameter_source: &[u8], connector_source: &[u8]) -> Fixture {
    let parameter_file = inventory_file(PARAMETER_PATH, parameter_source);
    let connector_file = inventory_file(CONNECTOR_PATH, connector_source);
    Fixture {
        resolution_input: resolution_input(),
        enum_mappings: vec![enum_mapping()],
        inventory: SourceInventory {
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
                    total_bytes: BigInt::from(parameter_source.len()),
                    modelica_file_count: BigInt::from(1_u8),
                    package_order_count: BigInt::from(0_u8),
                    files: vec![parameter_file],
                },
                SourceInventorySnapshot {
                    role: SourceSnapshotRole::Development,
                    revision: DEVELOPMENT_REVISION.to_owned(),
                    root_tree_sha1: format!("sha1:{}", "5".repeat(40)),
                    file_count: BigInt::from(1_u8),
                    total_bytes: BigInt::from(connector_source.len()),
                    modelica_file_count: BigInt::from(1_u8),
                    package_order_count: BigInt::from(0_u8),
                    files: vec![connector_file],
                },
            ],
        },
        pins: vec![
            SourcePin {
                role: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
            },
            SourcePin {
                role: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
            },
        ],
        claims: vec![
            SourceClassClaim {
                canonical_class_path: PARAMETER_CLASS.to_owned(),
                snapshot: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                file: locator(PARAMETER_PATH, parameter_source),
            },
            SourceClassClaim {
                canonical_class_path: CONNECTOR_CLASS.to_owned(),
                snapshot: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
                file: locator(CONNECTOR_PATH, connector_source),
            },
        ],
        bindings: vec![
            binding(
                SourceOwnerKind::Parameter,
                "sample_period_s",
                PARAMETER_CLASS,
                "samplePeriod",
            ),
            binding(
                SourceOwnerKind::Parameter,
                "gains",
                PARAMETER_CLASS,
                "gains",
            ),
            binding(
                SourceOwnerKind::Parameter,
                "matrix_weights",
                PARAMETER_CLASS,
                "matrixWeights",
            ),
            binding(
                SourceOwnerKind::Parameter,
                "initial_mode",
                PARAMETER_CLASS,
                "initialMode",
            ),
            binding(
                SourceOwnerKind::Parameter,
                "enable_optional",
                PARAMETER_CLASS,
                "enableOptional",
            ),
            binding(
                SourceOwnerKind::Connector,
                "requests",
                CONNECTOR_CLASS,
                "requests",
            ),
            binding(
                SourceOwnerKind::Connector,
                "matrix_feedback",
                CONNECTOR_CLASS,
                "matrixFeedback",
            ),
        ],
    }
}

fn valid_fixture() -> Fixture {
    fixture(PARAMETER_SOURCE.as_bytes(), CONNECTOR_SOURCE.as_bytes())
}

fn write_role_isolated_sources(
    release: &FixtureDir,
    development: &FixtureDir,
    parameter_source: &[u8],
    connector_source: &[u8],
) {
    release.write(PARAMETER_PATH, parameter_source);
    release.write(CONNECTOR_PATH, b"wrong release snapshot");
    development.write(PARAMETER_PATH, b"wrong development snapshot");
    development.write(CONNECTOR_PATH, connector_source);
}

fn resolution_limits() -> ResolutionLimits {
    ResolutionLimits {
        max_guard_depth: 1,
        max_guard_nodes: 1,
        max_scalar_leaves: 15,
    }
}

fn declaration_limits() -> DeclarationRootPipelineLimits {
    DeclarationRootPipelineLimits {
        max_documents: 2,
        max_requirements: 7,
        max_source_bytes: PARAMETER_SOURCE.len().max(CONNECTOR_SOURCE.len()),
        max_total_source_bytes: PARAMETER_SOURCE.len() + CONNECTOR_SOURCE.len(),
        max_direct_members: 5,
    }
}

fn named_and_claims(
    fixture: &Fixture,
    limits: ResolutionLimits,
) -> (
    crate::scalar_names::NamedScalarProjection,
    crate::scalar_source_claims::ScalarSourceClaimProjection,
) {
    let resolved =
        resolve_validated(&fixture.resolution_input, limits).expect("resolution succeeds");
    let abi =
        project_scalar_abi(&resolved, &fixture.enum_mappings).expect("ABI projection succeeds");
    let named = allocate_scalar_names(&abi).expect("name allocation succeeds");
    let claims = project_scalar_source_claims(
        &named,
        &fixture.inventory,
        &fixture.pins,
        &fixture.claims,
        &fixture.bindings,
    )
    .expect("source claim projection succeeds");
    (named, claims)
}

fn compile(
    fixture: &Fixture,
    roots: DeclarationSourceRoots<'_>,
    resolution_limits: ResolutionLimits,
    declaration_limits: DeclarationRootPipelineLimits,
) -> Result<BoundScalarProjection, CompilerPipelineError> {
    compile_validated_from_roots(
        &fixture.resolution_input,
        resolution_limits,
        &fixture.enum_mappings,
        &fixture.inventory,
        &fixture.pins,
        &fixture.claims,
        &fixture.bindings,
        roots,
        declaration_limits,
    )
}

#[test]
fn full_chain_matches_manual_composition_and_preserves_borrowed_inputs() {
    let release_fixture = FixtureDir::new("success-release");
    let development_fixture = FixtureDir::new("success-development");
    write_role_isolated_sources(
        &release_fixture,
        &development_fixture,
        PARAMETER_SOURCE.as_bytes(),
        CONNECTOR_SOURCE.as_bytes(),
    );
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let fixture = valid_fixture();
    let before = fixture.clone();

    let resolved = resolve_validated(&fixture.resolution_input, resolution_limits())
        .expect("manual resolution succeeds");
    assert!(
        !resolved
            .connectors
            .iter()
            .find(|connector| connector.connector_id == "optional_signal")
            .expect("optional connector is retained by resolution")
            .active
    );
    let abi = project_scalar_abi(&resolved, &fixture.enum_mappings)
        .expect("manual ABI projection succeeds");
    let named = allocate_scalar_names(&abi).expect("manual name allocation succeeds");
    let source_claims = project_scalar_source_claims(
        &named,
        &fixture.inventory,
        &fixture.pins,
        &fixture.claims,
        &fixture.bindings,
    )
    .expect("manual source claim projection succeeds");
    let expected =
        check_declaration_pipeline_from_roots(&named, &source_claims, roots, declaration_limits())
            .expect("manual declaration pipeline succeeds");

    let first = compile(&fixture, roots, resolution_limits(), declaration_limits())
        .expect("compiler pipeline succeeds");
    let second = compile(&fixture, roots, resolution_limits(), declaration_limits())
        .expect("borrowed roots remain reusable");

    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert_eq!(fixture, before);
    assert_eq!(
        release_fixture.read(PARAMETER_PATH),
        PARAMETER_SOURCE.as_bytes()
    );
    assert_eq!(
        development_fixture.read(CONNECTOR_PATH),
        CONNECTOR_SOURCE.as_bytes()
    );
    assert_eq!(
        first
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
            "enable_optional",
        ]
    );
    assert_eq!(
        first
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
    assert!(
        first
            .connectors
            .iter()
            .all(|row| row.connector_id != "optional_signal")
    );
    assert_eq!(
        first
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
    let enum_row = first
        .parameters
        .iter()
        .find(|row| row.parameter_id == "initial_mode")
        .expect("enum row exists");
    assert_eq!(
        enum_row.abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: ENUM_CLASS.to_owned(),
        }
    );
    assert_eq!(enum_row.value, ScalarAbiValue::Enum { ordinal: 2 });
    assert!(first.parameters.iter().all(|row| {
        row.scalar_name.starts_with("p_")
            && row.source_claim.snapshot == SourceSnapshotRole::Release
            && row.source_claim.file.path == PARAMETER_PATH
    }));
    assert!(first.connectors.iter().all(|row| {
        row.scalar_name.starts_with("c_")
            && row.source_claim.snapshot == SourceSnapshotRole::Development
            && row.source_claim.file.path == CONNECTOR_PATH
    }));
    let scalar_names = first
        .parameters
        .iter()
        .map(|row| row.scalar_name.as_str())
        .chain(first.connectors.iter().map(|row| row.scalar_name.as_str()))
        .collect::<HashSet<_>>();
    assert_eq!(
        scalar_names.len(),
        first.parameters.len() + first.connectors.len()
    );
}

#[test]
fn resolution_failures_precede_downstream_work_and_forward_limits() {
    let release_fixture = FixtureDir::new("resolution-release");
    let development_fixture = FixtureDir::new("resolution-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let mut invalid = valid_fixture();
    invalid.resolution_input.canonical_id.clear();
    invalid.inventory.schema.clear();
    let expected = resolve_validated(&invalid.resolution_input, resolution_limits())
        .expect_err("invalid resolution input fails");

    let error = compile(&invalid, roots, resolution_limits(), declaration_limits())
        .expect_err("resolution fails before invalid source inputs or root access");
    assert_eq!(error, CompilerPipelineError::Resolution(expected));

    let fixture = valid_fixture();
    let tight_limits = ResolutionLimits {
        max_scalar_leaves: resolution_limits().max_scalar_leaves - 1,
        ..resolution_limits()
    };
    let expected = resolve_validated(&fixture.resolution_input, tight_limits)
        .expect_err("direct resolver enforces the scalar limit");
    let error = compile(&fixture, roots, tight_limits, declaration_limits())
        .expect_err("compiler forwards the exact resolution limits");
    assert_eq!(error, CompilerPipelineError::Resolution(expected));
}

#[test]
fn scalar_abi_failure_precedes_naming_source_validation_and_root_access() {
    let release_fixture = FixtureDir::new("abi-release");
    let development_fixture = FixtureDir::new("abi-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let mut fixture = valid_fixture();
    fixture.enum_mappings.clear();
    fixture.inventory.schema.clear();
    let resolved = resolve_validated(&fixture.resolution_input, resolution_limits())
        .expect("resolution succeeds");
    let expected = project_scalar_abi(&resolved, &fixture.enum_mappings)
        .expect_err("missing enum mapping fails ABI projection");

    let error = compile(&fixture, roots, resolution_limits(), declaration_limits())
        .expect_err("ABI projection fails before downstream stages");
    assert_eq!(error, CompilerPipelineError::ScalarAbi(expected));
}

#[test]
fn source_claim_failure_precedes_root_access() {
    let release_fixture = FixtureDir::new("claim-release");
    let development_fixture = FixtureDir::new("claim-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let mut fixture = valid_fixture();
    fixture.inventory.schema.clear();
    let resolved = resolve_validated(&fixture.resolution_input, resolution_limits())
        .expect("resolution succeeds");
    let abi =
        project_scalar_abi(&resolved, &fixture.enum_mappings).expect("ABI projection succeeds");
    let named = allocate_scalar_names(&abi).expect("name allocation succeeds");
    let expected = project_scalar_source_claims(
        &named,
        &fixture.inventory,
        &fixture.pins,
        &fixture.claims,
        &fixture.bindings,
    )
    .expect_err("invalid inventory fails source claims");

    let error = compile(&fixture, roots, resolution_limits(), declaration_limits())
        .expect_err("source claims fail before missing roots are read");
    assert_eq!(error, CompilerPipelineError::SourceClaims(expected));
}

#[test]
fn declaration_source_acquisition_failure_is_exact_and_atomic() {
    let release_fixture = FixtureDir::new("missing-release");
    let development_fixture = FixtureDir::new("missing-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let fixture = valid_fixture();
    let (named, claims) = named_and_claims(&fixture, resolution_limits());
    let expected =
        check_declaration_pipeline_from_roots(&named, &claims, roots, declaration_limits())
            .expect_err("missing source files fail acquisition");

    let error = compile(&fixture, roots, resolution_limits(), declaration_limits())
        .expect_err("compiler returns no partial projection on acquisition failure");
    assert_eq!(error, CompilerPipelineError::Declaration(expected));
}

#[test]
fn declaration_syntax_failure_follows_successful_acquisition() {
    let invalid_parameter_source = b"not valid Modelica";
    let release_fixture = FixtureDir::new("syntax-release");
    let development_fixture = FixtureDir::new("syntax-development");
    write_role_isolated_sources(
        &release_fixture,
        &development_fixture,
        invalid_parameter_source,
        CONNECTOR_SOURCE.as_bytes(),
    );
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let fixture = fixture(invalid_parameter_source, CONNECTOR_SOURCE.as_bytes());
    let (named, claims) = named_and_claims(&fixture, resolution_limits());
    let requirements = project_declaration_requirements(&claims)
        .expect("declaration requirements project before acquisition");
    let limits = DeclarationRootPipelineLimits {
        max_source_bytes: invalid_parameter_source.len().max(CONNECTOR_SOURCE.len()),
        max_total_source_bytes: invalid_parameter_source.len() + CONNECTOR_SOURCE.len(),
        ..declaration_limits()
    };
    let documents =
        read_declaration_sources(&requirements, roots, DeclarationSourceLimits::from(limits))
            .expect("opaque invalid syntax bytes are acquired successfully");
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].bytes, invalid_parameter_source);
    let expected = check_declaration_pipeline_from_roots(&named, &claims, roots, limits)
        .expect_err("manual declaration pipeline rejects invalid syntax");
    assert!(matches!(
        expected,
        DeclarationRootPipelineError::Pipeline(DeclarationPipelineError::Syntax(_))
    ));

    let error = compile(&fixture, roots, resolution_limits(), limits)
        .expect_err("compiler returns no partial projection on syntax failure");
    assert_eq!(error, CompilerPipelineError::Declaration(expected));
}

#[test]
fn declaration_limits_are_forwarded_without_reinterpretation() {
    let release_fixture = FixtureDir::new("limits-release");
    let development_fixture = FixtureDir::new("limits-development");
    write_role_isolated_sources(
        &release_fixture,
        &development_fixture,
        PARAMETER_SOURCE.as_bytes(),
        CONNECTOR_SOURCE.as_bytes(),
    );
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let fixture = valid_fixture();
    let (named, claims) = named_and_claims(&fixture, resolution_limits());

    let document_limited = DeclarationRootPipelineLimits {
        max_documents: declaration_limits().max_documents - 1,
        ..declaration_limits()
    };
    let expected = check_declaration_pipeline_from_roots(&named, &claims, roots, document_limited)
        .expect_err("direct declaration pipeline enforces the document limit");
    let error = compile(&fixture, roots, resolution_limits(), document_limited)
        .expect_err("compiler forwards the document limit");
    assert_eq!(error, CompilerPipelineError::Declaration(expected));

    let syntax_limited = DeclarationRootPipelineLimits {
        max_direct_members: declaration_limits().max_direct_members - 1,
        ..declaration_limits()
    };
    let requirements = project_declaration_requirements(&claims).expect("requirements project");
    read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(syntax_limited),
    )
    .expect("source acquisition passes before the syntax-only limit");
    let expected = check_declaration_pipeline_from_roots(&named, &claims, roots, syntax_limited)
        .expect_err("direct declaration pipeline enforces the member limit");
    assert!(matches!(
        expected,
        DeclarationRootPipelineError::Pipeline(DeclarationPipelineError::Syntax(_))
    ));
    let error = compile(&fixture, roots, resolution_limits(), syntax_limited)
        .expect_err("compiler forwards the syntax limit");
    assert_eq!(error, CompilerPipelineError::Declaration(expected));
}

fn assert_source<T>(wrapper: &CompilerPipelineError, expected: &T, prefix: &str)
where
    T: Error + PartialEq + Debug + 'static,
{
    assert_eq!(wrapper.to_string(), format!("{prefix}: {expected}"));
    assert_eq!(
        Error::source(wrapper)
            .expect("wrapper exposes its inner error")
            .downcast_ref::<T>(),
        Some(expected)
    );
}

#[test]
fn error_variants_preserve_equality_display_and_source_chains() {
    let resolution = ResolutionError::InvalidInput {
        detail: "fixture".to_owned(),
    };
    let wrapper = CompilerPipelineError::Resolution(resolution.clone());
    assert_eq!(
        wrapper,
        CompilerPipelineError::Resolution(resolution.clone())
    );
    assert_source(&wrapper, &resolution, "resolution failed");

    let scalar_abi = ScalarAbiError {
        diagnostics: vec![ScalarAbiDiagnostic {
            code: "fixture".to_owned(),
            owner_kind: "projection".to_owned(),
            owner_id: "$".to_owned(),
            type_id: String::new(),
            message: "fixture".to_owned(),
        }],
    };
    let wrapper = CompilerPipelineError::ScalarAbi(scalar_abi.clone());
    assert_eq!(
        wrapper,
        CompilerPipelineError::ScalarAbi(scalar_abi.clone())
    );
    assert_source(&wrapper, &scalar_abi, "scalar ABI projection failed");

    let scalar_names = ScalarNameError {
        diagnostics: vec![ScalarNameDiagnostic {
            code: "fixture".to_owned(),
            owner_kind: "projection".to_owned(),
            owner_id: "$".to_owned(),
            location: "$".to_owned(),
            message: "fixture".to_owned(),
        }],
    };
    let wrapper = CompilerPipelineError::ScalarNames(scalar_names.clone());
    assert_eq!(
        wrapper,
        CompilerPipelineError::ScalarNames(scalar_names.clone())
    );
    assert_source(&wrapper, &scalar_names, "scalar name allocation failed");

    let source_claims = SourceClaimError {
        diagnostics: vec![SourceClaimDiagnostic {
            code: "fixture".to_owned(),
            owner_kind: "projection".to_owned(),
            owner_id: "$".to_owned(),
            location: "$".to_owned(),
            message: "fixture".to_owned(),
        }],
    };
    let wrapper = CompilerPipelineError::SourceClaims(source_claims.clone());
    assert_eq!(
        wrapper,
        CompilerPipelineError::SourceClaims(source_claims.clone())
    );
    assert_source(&wrapper, &source_claims, "source claim projection failed");

    let declaration_source = DeclarationSourceError {
        diagnostics: vec![DeclarationSourceDiagnostic {
            code: "fixture".to_owned(),
            location: "$".to_owned(),
            message: "fixture".to_owned(),
        }],
    };
    let declaration = DeclarationRootPipelineError::Source(declaration_source.clone());
    let wrapper = CompilerPipelineError::Declaration(declaration.clone());
    assert_eq!(
        wrapper,
        CompilerPipelineError::Declaration(declaration.clone())
    );
    assert_source(&wrapper, &declaration, "declaration pipeline failed");
    assert_eq!(
        Error::source(&declaration)
            .expect("declaration wrapper exposes source acquisition error")
            .downcast_ref::<DeclarationSourceError>(),
        Some(&declaration_source)
    );
}

#[test]
fn implementation_has_no_ambient_or_deferred_integrations() {
    let source_text = include_str!("../compiler_pipeline.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "std::env",
        "std::net",
        "std::process",
        "std::io",
        "File::open",
        "open_ambient_dir",
        "ambient_authority",
        "serde",
        "serde_json",
        "raw JSON",
        "schema",
        "clap",
        "Command::",
        "git2",
        "Engine",
        "Studio",
        "cxf_json",
        "cxf-json",
        "registry",
        "CXF",
        "source_map",
        "source-map",
        "provenance",
        "rand::",
        "reqwest",
        "ureq",
        "TcpStream",
    ] {
        assert!(
            !source_text.contains(forbidden),
            "compiler pipeline must not use {forbidden}"
        );
    }
}
