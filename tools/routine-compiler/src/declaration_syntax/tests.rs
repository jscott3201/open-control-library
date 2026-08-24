use std::collections::HashSet;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

use num_bigint::BigInt;

use super::*;
use crate::declaration_requirements::{
    ConnectorDeclarationRequirement, ParameterDeclarationRequirement,
    project_declaration_requirements,
};
use crate::resolution::{
    ConnectorDefinition, ConnectorDirection, ConnectorPresence, FiniteReal, ParameterDefinition,
    ParameterSource, ParameterValue, PrimitiveType, ResolutionLimits, ScalarValue, Shape, TypeUse,
    ValidatedResolutionInput, resolve_validated,
};
use crate::scalar_abi::project_scalar_abi;
use crate::scalar_names::{allocate_scalar_names, build_scalar_name};
use crate::scalar_source_claims::{
    SourceClassClaim, SourceInventory, SourceInventoryFile, SourceInventoryLicense,
    SourceInventorySnapshot, SourceMemberBinding, SourceOwnerKind, SourcePin,
    project_scalar_source_claims,
};

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const TRIM_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";
const TRIM_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const OTHER_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.Other";
const OTHER_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/Other.mo";

const VALID_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block TrimAndRespond
  parameter Real samplePeriod;
  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;
end TrimAndRespond;
"#;

const OTHER_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
class Other
  parameter Unresolved.Qualified.Type gain;
end Other;
"#;

fn locator(path: &str, blob: &str) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: blob.to_owned(),
    }
}

fn scalar_name(prefix: &str, owner_id: &str) -> String {
    build_scalar_name(prefix, owner_id, &[]).expect("fixture scalar name builds")
}

fn parameter_requirement(
    parameter_id: &str,
    canonical_class_path: &str,
    source_member: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    file: SourceFileLocator,
) -> ParameterDeclarationRequirement {
    ParameterDeclarationRequirement {
        parameter_id: parameter_id.to_owned(),
        canonical_class_path: canonical_class_path.to_owned(),
        source_member: source_member.to_owned(),
        snapshot,
        revision: revision.to_owned(),
        file,
        scalar_names: vec![scalar_name("p_", parameter_id)],
    }
}

fn connector_requirement(
    connector_id: &str,
    canonical_class_path: &str,
    source_member: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    file: SourceFileLocator,
) -> ConnectorDeclarationRequirement {
    ConnectorDeclarationRequirement {
        connector_id: connector_id.to_owned(),
        canonical_class_path: canonical_class_path.to_owned(),
        source_member: source_member.to_owned(),
        snapshot,
        revision: revision.to_owned(),
        file,
        scalar_names: vec![scalar_name("c_", connector_id)],
    }
}

fn projection(
    parameters: Vec<ParameterDeclarationRequirement>,
    connectors: Vec<ConnectorDeclarationRequirement>,
) -> DeclarationRequirementProjection {
    DeclarationRequirementProjection {
        canonical_id: "typed-boundary-id".to_owned(),
        revision: BigInt::from(7_u8),
        parameters,
        connectors,
    }
}

fn document(
    source: &[u8],
    snapshot: SourceSnapshotRole,
    revision: &str,
    path: &str,
) -> DeclarationSourceDocument {
    DeclarationSourceDocument {
        snapshot,
        revision: revision.to_owned(),
        file: locator(path, &git_blob_sha1(source)),
        bytes: source.to_vec(),
    }
}

fn trim_fixture(
    source: &[u8],
) -> (
    DeclarationRequirementProjection,
    Vec<DeclarationSourceDocument>,
) {
    let document = document(
        source,
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
    );
    let file = document.file.clone();
    (
        projection(
            vec![parameter_requirement(
                "sample_period_s",
                TRIM_CLASS,
                "samplePeriod",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                file.clone(),
            )],
            vec![connector_requirement(
                "requests",
                TRIM_CLASS,
                "numOfReq",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                file,
            )],
        ),
        vec![document],
    )
}

fn limits() -> DeclarationSyntaxLimits {
    DeclarationSyntaxLimits {
        max_documents: 16,
        max_requirements: 32,
        max_source_bytes: 64 * 1024,
        max_total_source_bytes: 256 * 1024,
        max_direct_members: 128,
    }
}

fn codes(error: &DeclarationSyntaxError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn assert_code(
    requirements: &DeclarationRequirementProjection,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
    expected: &str,
) -> DeclarationSyntaxError {
    let error = check_owner_declaration_syntax(requirements, documents, limits)
        .expect_err("fixture must fail atomically");
    assert!(
        codes(&error).contains(expected),
        "missing {expected} in {error:?}"
    );
    assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    error
}

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("fixture value is finite")
}

fn inventory_file(path: &str, bytes: usize, blob: &str) -> SourceInventoryFile {
    SourceInventoryFile {
        path: path.to_owned(),
        mode: "100644".to_owned(),
        bytes: BigInt::from(bytes),
        git_blob_sha1: blob.to_owned(),
        sha256: format!("sha256:{}", "1".repeat(64)),
    }
}

#[test]
fn git_blob_hash_matches_independent_vector_and_mismatch_stops_parsing() {
    const HELLO_BLOB: &str = "sha1:ce013625030ba8dba906f756967f9e9ca394464a";
    assert_eq!(git_blob_sha1(b"hello\n"), HELLO_BLOB);
    assert_eq!(
        git_blob_sha1(b"hello world\n"),
        "sha1:3b18e512dba79e4c8300dd08aeb37f8e728b8dad"
    );
    assert_ne!(git_blob_sha1(b"hello!\n"), HELLO_BLOB);

    let file = locator(TRIM_PATH, HELLO_BLOB);
    let requirements = projection(
        vec![parameter_requirement(
            "sample_period_s",
            TRIM_CLASS,
            "samplePeriod",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            file.clone(),
        )],
        Vec::new(),
    );
    let documents = vec![DeclarationSourceDocument {
        snapshot: SourceSnapshotRole::Release,
        revision: RELEASE_REVISION.to_owned(),
        file,
        bytes: b"hello!\n".to_vec(),
    }];
    let mut parse_count = 0_usize;
    let error = check_owner_declaration_syntax_with_parser(
        &requirements,
        &documents,
        limits(),
        |source, path| {
            parse_count += 1;
            rumoca_phase_parse::parse_to_ast(source, path)
        },
    )
    .expect_err("changed bytes must not authenticate");
    assert_eq!(parse_count, 0);
    assert_eq!(codes(&error), HashSet::from(["source_blob_mismatch"]));
}

#[test]
fn full_typed_chain_reaches_valid_in_memory_syntax_check() {
    let input = ValidatedResolutionInput {
        canonical_id: "typed-full-chain-id".to_owned(),
        revision: BigInt::from(10_u8).pow(40),
        types: Vec::new(),
        dimensions: Vec::new(),
        parameters: vec![ParameterDefinition {
            parameter_id: "sample_period_s".to_owned(),
            type_use: TypeUse::Primitive(PrimitiveType::Real),
            shape: Shape::Scalar,
            source: ParameterSource::Default,
            value: ParameterValue::Scalar(ScalarValue::Real(finite(60.0))),
        }],
        connectors: vec![ConnectorDefinition {
            connector_id: "requests".to_owned(),
            direction: ConnectorDirection::Input,
            type_use: TypeUse::Primitive(PrimitiveType::Integer),
            shape: Shape::Scalar,
            presence: ConnectorPresence::Always,
        }],
    };
    let resolved =
        resolve_validated(&input, ResolutionLimits::default()).expect("resolution succeeds");
    let abi = project_scalar_abi(&resolved, &[]).expect("ABI projection succeeds");
    let named = allocate_scalar_names(&abi).expect("scalar naming succeeds");
    let blob = git_blob_sha1(VALID_SOURCE.as_bytes());
    let development_blob = format!("sha1:{}", "2".repeat(40));
    let inventory = SourceInventory {
        schema: "cxf-library/g36-source-inventory/v1".to_owned(),
        repository: "https://github.com/lbl-srg/modelica-buildings.git".to_owned(),
        source_root: "Buildings/Controls/OBC/ASHRAE/G36".to_owned(),
        inventory_scope: "source-root-regular-files".to_owned(),
        dependency_closure: "not-inventoried".to_owned(),
        license: SourceInventoryLicense {
            upstream_path: "Buildings/legal.html".to_owned(),
            retained_path: "routines/g36/LICENSE-BUILDINGS.html".to_owned(),
            git_blob_sha1: format!("sha1:{}", "3".repeat(40)),
            sha256: format!("sha256:{}", "4".repeat(64)),
        },
        snapshots: vec![
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Release,
                revision: RELEASE_REVISION.to_owned(),
                root_tree_sha1: format!("sha1:{}", "5".repeat(40)),
                file_count: BigInt::from(1_u8),
                total_bytes: BigInt::from(VALID_SOURCE.len()),
                modelica_file_count: BigInt::from(1_u8),
                package_order_count: BigInt::from(0_u8),
                files: vec![inventory_file(TRIM_PATH, VALID_SOURCE.len(), &blob)],
            },
            SourceInventorySnapshot {
                role: SourceSnapshotRole::Development,
                revision: DEVELOPMENT_REVISION.to_owned(),
                root_tree_sha1: format!("sha1:{}", "6".repeat(40)),
                file_count: BigInt::from(1_u8),
                total_bytes: BigInt::from(1_u8),
                modelica_file_count: BigInt::from(1_u8),
                package_order_count: BigInt::from(0_u8),
                files: vec![inventory_file(OTHER_PATH, 1, &development_blob)],
            },
        ],
    };
    let claims = project_scalar_source_claims(
        &named,
        &inventory,
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
            file: locator(TRIM_PATH, &blob),
        }],
        &[
            SourceMemberBinding {
                owner_kind: SourceOwnerKind::Parameter,
                owner_id: "sample_period_s".to_owned(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: "samplePeriod".to_owned(),
            },
            SourceMemberBinding {
                owner_kind: SourceOwnerKind::Connector,
                owner_id: "requests".to_owned(),
                canonical_class_path: TRIM_CLASS.to_owned(),
                source_member: "numOfReq".to_owned(),
            },
        ],
    )
    .expect("source claim projection succeeds");
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let documents = vec![document(
        VALID_SOURCE.as_bytes(),
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
    )];

    check_owner_declaration_syntax(&requirements, &documents, limits())
        .expect("typed chain declaration syntax conforms");
}

#[test]
fn shared_document_is_parsed_once_and_document_order_is_irrelevant() {
    let (requirements, documents) = trim_fixture(VALID_SOURCE.as_bytes());
    let mut parse_count = 0_usize;
    check_owner_declaration_syntax_with_parser(
        &requirements,
        &documents,
        limits(),
        |source, path| {
            parse_count += 1;
            rumoca_phase_parse::parse_to_ast(source, path)
        },
    )
    .expect("shared source conforms");
    assert_eq!(parse_count, 1);

    let other_document = document(
        OTHER_SOURCE.as_bytes(),
        SourceSnapshotRole::Development,
        DEVELOPMENT_REVISION,
        OTHER_PATH,
    );
    let other_parameter = parameter_requirement(
        "gain",
        OTHER_CLASS,
        "gain",
        SourceSnapshotRole::Development,
        DEVELOPMENT_REVISION,
        other_document.file.clone(),
    );
    let mut two_requirements = requirements;
    two_requirements.parameters.push(other_parameter);
    let mut forward = vec![documents[0].clone(), other_document];
    let forward_result = check_owner_declaration_syntax(&two_requirements, &forward, limits());
    forward.reverse();
    let reverse_result = check_owner_declaration_syntax(&two_requirements, &forward, limits());
    assert_eq!(forward_result, reverse_result);
    forward_result.expect("both document orders conform");

    forward[0].bytes.push(b' ');
    forward[1].bytes.push(b' ');
    let forward_error = check_owner_declaration_syntax(&two_requirements, &forward, limits());
    forward.reverse();
    let reverse_error = check_owner_declaration_syntax(&two_requirements, &forward, limits());
    assert_eq!(forward_error, reverse_error);
}

#[test]
fn document_coverage_rejects_duplicates_missing_unused_and_role_fallback() {
    let (requirements, documents) = trim_fixture(VALID_SOURCE.as_bytes());
    let duplicate = vec![documents[0].clone(), documents[0].clone()];
    assert_code(
        &requirements,
        &duplicate,
        limits(),
        "duplicate_source_document",
    );
    assert_code(&requirements, &[], limits(), "missing_source_document");

    let mut extra = documents.clone();
    extra.push(document(
        OTHER_SOURCE.as_bytes(),
        SourceSnapshotRole::Development,
        DEVELOPMENT_REVISION,
        OTHER_PATH,
    ));
    assert_code(&requirements, &extra, limits(), "unused_source_document");

    let mut wrong_role = documents;
    wrong_role[0].snapshot = SourceSnapshotRole::Development;
    let error = assert_code(
        &requirements,
        &wrong_role,
        limits(),
        "missing_source_document",
    );
    assert!(codes(&error).contains("unused_source_document"));
}

#[test]
fn matching_blob_precedes_utf8_and_parser_failures() {
    let invalid_utf8 = [0xff_u8, 0xfe];
    let (requirements, documents) = trim_fixture(&invalid_utf8);
    let mut parse_count = 0_usize;
    let error = check_owner_declaration_syntax_with_parser(
        &requirements,
        &documents,
        limits(),
        |source, path| {
            parse_count += 1;
            rumoca_phase_parse::parse_to_ast(source, path)
        },
    )
    .expect_err("matching non-UTF-8 blob must fail");
    assert_eq!(parse_count, 0);
    assert_eq!(codes(&error), HashSet::from(["source_not_utf8"]));

    let malformed = b"within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond";
    let (requirements, documents) = trim_fixture(malformed);
    let mut parse_count = 0_usize;
    let error = check_owner_declaration_syntax_with_parser(
        &requirements,
        &documents,
        limits(),
        |source, path| {
            parse_count += 1;
            rumoca_phase_parse::parse_to_ast(source, path)
        },
    )
    .expect_err("malformed Modelica must fail");
    assert_eq!(parse_count, 1);
    assert_eq!(codes(&error), HashSet::from(["modelica_parse_failed"]));
}

#[test]
fn direct_class_identity_is_exact() {
    let cases = [
        VALID_SOURCE.replace(
            "within Buildings.Controls.OBC.ASHRAE.G36.Generic;",
            "within Buildings.Controls.OBC.ASHRAE.G36.Types;",
        ),
        VALID_SOURCE.replace("TrimAndRespond", "Wrong"),
        format!("{VALID_SOURCE}\nclass Extra end Extra;\n"),
    ];
    for source in cases {
        let (requirements, documents) = trim_fixture(source.as_bytes());
        assert_code(&requirements, &documents, limits(), "invalid_direct_class");
    }
}

#[test]
fn direct_member_publicity_and_parameter_variability_are_required() {
    let inherited_only = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block TrimAndRespond
  extends ExternalBase;
  Integer numOfReq;
end TrimAndRespond;
"#;
    let (requirements, documents) = trim_fixture(inherited_only.as_bytes());
    assert_code(&requirements, &documents, limits(), "missing_direct_member");

    let protected = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block TrimAndRespond
protected
  parameter Real samplePeriod;
public
  Integer numOfReq;
end TrimAndRespond;
"#;
    let (requirements, documents) = trim_fixture(protected.as_bytes());
    assert_code(&requirements, &documents, limits(), "protected_member");

    let non_parameter = VALID_SOURCE.replace("parameter Real samplePeriod", "Real samplePeriod");
    let (requirements, documents) = trim_fixture(non_parameter.as_bytes());
    assert_code(&requirements, &documents, limits(), "parameter_variability");
}

#[test]
fn class_kind_declared_types_and_unresolved_types_are_deliberate_non_claims() {
    let source = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
model TrimAndRespond
  parameter Boolean samplePeriod;
  parameter Unresolved.Qualified.Type numOfReq;
end TrimAndRespond;
"#;
    let (requirements, documents) = trim_fixture(source.as_bytes());
    check_owner_declaration_syntax(&requirements, &documents, limits())
        .expect("kind, declared type, resolution, and connector variability are not checked");

    let (mut shared_key_requirements, shared_key_documents) = trim_fixture(VALID_SOURCE.as_bytes());
    shared_key_requirements.connectors[0].source_member = "samplePeriod".to_owned();
    check_owner_declaration_syntax(&shared_key_requirements, &shared_key_documents, limits())
        .expect("parameter and connector namespaces may share one source key");
}

#[test]
fn every_resource_limit_accepts_its_boundary_and_rejects_excess() {
    let (requirements, documents) = trim_fixture(VALID_SOURCE.as_bytes());
    let source_bytes = documents[0].bytes.len();
    let exact = DeclarationSyntaxLimits {
        max_documents: 1,
        max_requirements: 2,
        max_source_bytes: source_bytes,
        max_total_source_bytes: source_bytes,
        max_direct_members: 2,
    };
    check_owner_declaration_syntax(&requirements, &documents, exact)
        .expect("every exact limit boundary passes");

    for limited in [
        DeclarationSyntaxLimits {
            max_documents: 0,
            ..exact
        },
        DeclarationSyntaxLimits {
            max_requirements: 1,
            ..exact
        },
        DeclarationSyntaxLimits {
            max_source_bytes: source_bytes - 1,
            ..exact
        },
        DeclarationSyntaxLimits {
            max_total_source_bytes: source_bytes - 1,
            ..exact
        },
        DeclarationSyntaxLimits {
            max_direct_members: 1,
            ..exact
        },
    ] {
        assert_code(&requirements, &documents, limited, "resource_limit");
    }
    assert_eq!(checked_total_count([usize::MAX, 1]), None);
    assert_eq!(checked_total_count([1, 2, 3]), Some(6));

    let mut parse_count = 0_usize;
    let outcome = check_owner_declaration_syntax_with_parser(
        &requirements,
        &documents,
        DeclarationSyntaxLimits {
            max_documents: 0,
            ..exact
        },
        |source, path| {
            parse_count += 1;
            rumoca_phase_parse::parse_to_ast(source, path)
        },
    );
    assert!(outcome.is_err());
    assert_eq!(parse_count, 0);
}

#[test]
fn direct_member_limit_precedes_member_lookup() {
    let source = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block TrimAndRespond
  Real first;
  Real second;
end TrimAndRespond;
"#;
    let document = document(
        source.as_bytes(),
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
    );
    let requirements = projection(
        vec![parameter_requirement(
            "missing",
            TRIM_CLASS,
            "missing",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            document.file.clone(),
        )],
        Vec::new(),
    );
    let mut limited = limits();
    limited.max_direct_members = 1;
    let error = assert_code(&requirements, &[document], limited, "resource_limit");
    assert!(!codes(&error).contains("missing_direct_member"));
}

fn hostile_requirements() -> (
    DeclarationRequirementProjection,
    Vec<DeclarationSourceDocument>,
) {
    let document = document(
        VALID_SOURCE.as_bytes(),
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        TRIM_PATH,
    );
    let file = document.file.clone();
    let mut first = parameter_requirement(
        "first",
        TRIM_CLASS,
        "sharedMember",
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        file.clone(),
    );
    let mut second = parameter_requirement(
        "second",
        TRIM_CLASS,
        "sharedMember",
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        file.clone(),
    );
    second.scalar_names = first.scalar_names.clone();
    let duplicate_owner = parameter_requirement(
        "second",
        TRIM_CLASS,
        "otherMember",
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        file.clone(),
    );
    let mut duplicate_owner = duplicate_owner;
    duplicate_owner.scalar_names.clear();
    let mut malformed = parameter_requirement(
        "malformed",
        TRIM_CLASS,
        "member",
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        file.clone(),
    );
    malformed.parameter_id.clear();
    malformed.scalar_names = vec!["p_ABC".to_owned()];
    malformed.canonical_class_path = "Bad.Class".to_owned();
    malformed.source_member = "bad.member".to_owned();
    malformed.revision = "BAD".to_owned();
    malformed.file.path = "../bad.txt".to_owned();
    malformed.file.git_blob_sha1 = "sha1:BAD".to_owned();
    let mut connector = connector_requirement(
        "connector",
        TRIM_CLASS,
        "connectorMember",
        SourceSnapshotRole::Release,
        RELEASE_REVISION,
        file,
    );
    connector.scalar_names = first.scalar_names.clone();
    first.scalar_names.push(first.scalar_names[0].clone());
    let mut requirements = projection(
        vec![first, second, duplicate_owner, malformed],
        vec![connector],
    );
    requirements.canonical_id.clear();
    requirements.revision = BigInt::from(0_u8);
    (requirements, vec![document])
}

#[test]
fn forged_projection_metadata_names_owners_and_sources_fail_closed() {
    let (requirements, documents) = hostile_requirements();
    let error = assert_code(&requirements, &documents, limits(), "invalid_metadata");
    let actual = codes(&error);
    for expected in [
        "invalid_owner_id",
        "invalid_scalar_name",
        "scalar_name_namespace",
        "duplicate_owner",
        "duplicate_scalar_name",
        "cross_kind_collision",
        "duplicate_source_key",
        "invalid_source_class",
        "invalid_source_member",
        "invalid_source_revision",
        "invalid_source_path",
        "invalid_source_blob",
    ] {
        assert!(actual.contains(expected), "missing {expected} in {error:?}");
    }
}

#[test]
fn forged_document_identity_is_revalidated() {
    let (requirements, mut documents) = trim_fixture(VALID_SOURCE.as_bytes());
    let mut hostile = documents[0].clone();
    hostile.revision = "BAD".to_owned();
    hostile.file.path = "../bad.txt".to_owned();
    hostile.file.git_blob_sha1 = "sha1:BAD".to_owned();
    documents.push(hostile);
    let error = assert_code(
        &requirements,
        &documents,
        limits(),
        "invalid_source_revision",
    );
    let actual = codes(&error);
    assert!(actual.contains("invalid_source_path"));
    assert!(actual.contains("invalid_source_blob"));
    assert!(actual.contains("unused_source_document"));
}

#[test]
fn diagnostics_are_sorted_repeatable_atomic_non_panicking_and_inputs_unchanged() {
    let (requirements, mut documents) = hostile_requirements();
    documents.push(documents[0].clone());
    let requirements_before = requirements.clone();
    let documents_before = documents.clone();
    let mut attempts = Vec::new();
    for _ in 0..2 {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            check_owner_declaration_syntax(&requirements, &documents, limits())
        }));
        let error = outcome
            .expect("forged typed input must not panic")
            .expect_err("forged typed input must not partially succeed");
        assert!(!error.diagnostics.is_empty());
        assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        attempts.push(error);
    }
    assert_eq!(attempts[0], attempts[1]);
    assert_eq!(requirements, requirements_before);
    assert_eq!(documents, documents_before);
    fn assert_error_trait<T: Error>() {}
    assert_error_trait::<DeclarationSyntaxError>();
}

#[test]
fn implementation_stays_in_memory_and_uses_shared_parse_indexes() {
    let source_text = include_str!("../declaration_syntax.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "std::env",
        "std::net",
        "std::process",
        "std::io",
        "Command::",
        "serde",
        "cxf_json",
        "cxf-json",
        "Engine",
        "Studio",
        "PathBuf",
        "SystemTime",
        "rand::",
        "runtime",
        "output evidence",
        "output_evidence",
    ] {
        assert!(
            !source_text.contains(forbidden),
            "declaration syntax checker must not use {forbidden}"
        );
    }
    assert_eq!(
        source_text
            .matches("rumoca_phase_parse::parse_to_ast")
            .count(),
        1
    );
    assert!(source_text.contains("document_index"));
    assert!(source_text.contains("parsed_documents"));
    assert!(!source_text.contains("documents.iter().find"));
}
