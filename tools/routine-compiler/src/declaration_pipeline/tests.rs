use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cap_std::{ambient_authority, fs::Dir};
use num_bigint::BigInt;
use sha1::{Digest, Sha1};

use super::*;
use crate::bound_scalars::bind_scalar_source_claims;
use crate::declaration_requirements::project_declaration_requirements;
use crate::resolution::{ConnectorDirection, ParameterSource, PrimitiveType};
use crate::scalar_abi::{ScalarAbiType, ScalarAbiValue};
use crate::scalar_names::{NamedScalarConnectorRow, NamedScalarParameterRow, build_scalar_name};
use crate::scalar_source_claims::{
    ScalarConnectorSourceClaim, ScalarParameterSourceClaim, SourceFileLocator, SourceSnapshotRole,
};

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const PARAMETER_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.ParameterOwner";
const PARAMETER_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/ParameterOwner.mo";
const CONNECTOR_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.ConnectorOwner";
const CONNECTOR_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/ConnectorOwner.mo";

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const PARAMETER_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
model ParameterOwner
  parameter Unresolved.Qualified.Type gain;
end ParameterOwner;
"#;

const CONNECTOR_SOURCE: &str = r#"within Buildings.Controls.OBC.ASHRAE.G36.Generic;
class ConnectorOwner
  Unresolved.Qualified.Type signal;
end ConnectorOwner;
"#;

struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ocl-declaration-pipeline-{label}-{}-{sequence}",
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

    fn open(&self) -> Dir {
        Dir::open_ambient_dir(&self.path, ambient_authority()).expect("fixture root opens")
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git_blob_sha1(source: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", source.len()).as_bytes());
    hasher.update(source);
    format!("sha1:{:x}", hasher.finalize())
}

fn locator(path: &str, source: &[u8]) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: git_blob_sha1(source),
    }
}

fn document(
    snapshot: SourceSnapshotRole,
    revision: &str,
    path: &str,
    source: &[u8],
) -> DeclarationSourceDocument {
    DeclarationSourceDocument {
        snapshot,
        revision: revision.to_owned(),
        file: locator(path, source),
        bytes: source.to_vec(),
    }
}

fn limits() -> DeclarationSyntaxLimits {
    DeclarationSyntaxLimits {
        max_documents: 4,
        max_requirements: 4,
        max_source_bytes: 4096,
        max_total_source_bytes: 8192,
        max_direct_members: 8,
    }
}

fn root_limits() -> DeclarationRootPipelineLimits {
    DeclarationRootPipelineLimits {
        max_documents: 4,
        max_requirements: 4,
        max_source_bytes: 4096,
        max_total_source_bytes: 8192,
        max_direct_members: 8,
    }
}

fn write_fixture_sources(release: &FixtureDir, development: &FixtureDir) {
    release.write(PARAMETER_PATH, PARAMETER_SOURCE.as_bytes());
    release.write(CONNECTOR_PATH, b"wrong release snapshot");
    development.write(PARAMETER_PATH, b"wrong development snapshot");
    development.write(CONNECTOR_PATH, CONNECTOR_SOURCE.as_bytes());
}

fn fixture() -> (
    NamedScalarProjection,
    ScalarSourceClaimProjection,
    Vec<DeclarationSourceDocument>,
) {
    let parameter_name = build_scalar_name("p_", "gain", &[]).expect("parameter name builds");
    let connector_name = build_scalar_name("c_", "signal", &[]).expect("connector name builds");
    let named = NamedScalarProjection {
        canonical_id: "G36-RUST-DECLARATION-PIPELINE".to_owned(),
        revision: BigInt::from(7_u8),
        parameters: vec![NamedScalarParameterRow {
            scalar_name: parameter_name.clone(),
            parameter_id: "gain".to_owned(),
            coordinates: Vec::new(),
            abi_type: ScalarAbiType::Primitive(PrimitiveType::Integer),
            source: ParameterSource::Default,
            value: ScalarAbiValue::Integer(BigInt::from(3_u8)),
        }],
        connectors: vec![NamedScalarConnectorRow {
            scalar_name: connector_name.clone(),
            connector_id: "signal".to_owned(),
            coordinates: Vec::new(),
            abi_type: ScalarAbiType::Primitive(PrimitiveType::Integer),
            direction: ConnectorDirection::Input,
        }],
    };
    let claims = ScalarSourceClaimProjection {
        canonical_id: named.canonical_id.clone(),
        revision: named.revision.clone(),
        parameters: vec![ScalarParameterSourceClaim {
            scalar_name: parameter_name,
            parameter_id: "gain".to_owned(),
            coordinates: Vec::new(),
            canonical_class_path: PARAMETER_CLASS.to_owned(),
            source_member: "gain".to_owned(),
            snapshot: SourceSnapshotRole::Release,
            revision: RELEASE_REVISION.to_owned(),
            file: locator(PARAMETER_PATH, PARAMETER_SOURCE.as_bytes()),
        }],
        connectors: vec![ScalarConnectorSourceClaim {
            scalar_name: connector_name,
            connector_id: "signal".to_owned(),
            coordinates: Vec::new(),
            canonical_class_path: CONNECTOR_CLASS.to_owned(),
            source_member: "signal".to_owned(),
            snapshot: SourceSnapshotRole::Development,
            revision: DEVELOPMENT_REVISION.to_owned(),
            file: locator(CONNECTOR_PATH, CONNECTOR_SOURCE.as_bytes()),
        }],
    };
    let documents = vec![
        document(
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PARAMETER_PATH,
            PARAMETER_SOURCE.as_bytes(),
        ),
        document(
            SourceSnapshotRole::Development,
            DEVELOPMENT_REVISION,
            CONNECTOR_PATH,
            CONNECTOR_SOURCE.as_bytes(),
        ),
    ];
    (named, claims, documents)
}

#[test]
fn success_matches_direct_binding_and_preserves_inputs_and_repeatability() {
    let (named, claims, documents) = fixture();
    let named_before = named.clone();
    let claims_before = claims.clone();
    let documents_before = documents.clone();
    let expected = bind_scalar_source_claims(&named, &claims).expect("direct binding succeeds");

    let first = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect("pipeline succeeds");
    let second = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect("pipeline is repeatable");

    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert_eq!(named, named_before);
    assert_eq!(claims, claims_before);
    assert_eq!(documents, documents_before);
}

#[test]
fn document_order_and_deliberate_syntax_non_claims_do_not_change_output() {
    let (named, claims, mut documents) = fixture();
    let forward = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect("class kinds, declared types, and unresolved types are not checked");

    documents.reverse();
    let reverse = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect("document order is irrelevant");

    assert_eq!(forward, reverse);
}

#[test]
fn requirements_failure_precedes_binding_and_syntax_and_preserves_error() {
    let (named, mut claims, mut documents) = fixture();
    claims.canonical_id.clear();
    documents[0].bytes = b"not valid Modelica".to_vec();
    let expected = project_declaration_requirements(&claims)
        .expect_err("invalid source claims must fail requirement projection");

    let error = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect_err("requirements failure must stop the pipeline");

    assert_eq!(
        error,
        DeclarationPipelineError::Requirements(expected.clone())
    );
    assert_eq!(
        Error::source(&error)
            .expect("requirements error is exposed as the source")
            .to_string(),
        expected.to_string()
    );
}

#[test]
fn binding_failure_precedes_syntax_and_preserves_error() {
    let (mut named, claims, mut documents) = fixture();
    named.canonical_id = "G36-RUST-DECLARATION-PIPELINE-MISMATCH".to_owned();
    documents[0].bytes = b"not valid Modelica".to_vec();
    let expected = bind_scalar_source_claims(&named, &claims)
        .expect_err("incompatible projections must fail binding");

    let error = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect_err("binding failure must stop before syntax checking");

    assert_eq!(error, DeclarationPipelineError::Binding(expected.clone()));
    assert_eq!(
        Error::source(&error)
            .expect("binding error is exposed as the source")
            .to_string(),
        expected.to_string()
    );
}

#[test]
fn syntax_failure_is_atomic_and_preserves_error() {
    let (named, claims, mut documents) = fixture();
    documents[0].bytes = b"not valid Modelica".to_vec();
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let expected = check_owner_declaration_syntax(&requirements, &documents, limits())
        .expect_err("changed source bytes must fail syntax checking");

    let error = check_declaration_pipeline(&named, &claims, &documents, limits())
        .expect_err("syntax failure must not expose bound output");

    assert_eq!(error, DeclarationPipelineError::Syntax(expected.clone()));
    assert_eq!(
        Error::source(&error)
            .expect("syntax error is exposed as the source")
            .to_string(),
        expected.to_string()
    );
}

#[test]
fn from_roots_matches_in_memory_with_role_selection_and_reusable_inputs() {
    let release_fixture = FixtureDir::new("success-release");
    let development_fixture = FixtureDir::new("success-development");
    write_fixture_sources(&release_fixture, &development_fixture);
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let (named, claims, _) = fixture();
    let named_before = named.clone();
    let claims_before = claims.clone();
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let documents = read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(root_limits()),
    )
    .expect("role-specific sources are acquired");
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].snapshot, SourceSnapshotRole::Release);
    assert_eq!(documents[0].bytes, PARAMETER_SOURCE.as_bytes());
    assert_eq!(documents[1].snapshot, SourceSnapshotRole::Development);
    assert_eq!(documents[1].bytes, CONNECTOR_SOURCE.as_bytes());
    let expected = check_declaration_pipeline(
        &named,
        &claims,
        &documents,
        DeclarationSyntaxLimits::from(root_limits()),
    )
    .expect("the acquired bytes pass the in-memory pipeline");

    let first = check_declaration_pipeline_from_roots(&named, &claims, roots, root_limits())
        .expect("the root pipeline succeeds");
    let second = check_declaration_pipeline_from_roots(&named, &claims, roots, root_limits())
        .expect("borrowed roots can be reused deterministically");

    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert_eq!(named, named_before);
    assert_eq!(claims, claims_before);
}

#[test]
fn from_roots_requirements_failure_precedes_missing_sources() {
    let release_fixture = FixtureDir::new("requirements-release");
    let development_fixture = FixtureDir::new("requirements-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let (named, mut claims, _) = fixture();
    claims.canonical_id.clear();
    let expected = project_declaration_requirements(&claims)
        .expect_err("invalid claims fail requirement projection");

    let error = check_declaration_pipeline_from_roots(
        &named,
        &claims,
        DeclarationSourceRoots::new(&release, &development),
        root_limits(),
    )
    .expect_err("requirements must fail before missing files are opened");

    let expected_pipeline = DeclarationPipelineError::Requirements(expected);
    assert_eq!(
        error,
        DeclarationRootPipelineError::Pipeline(expected_pipeline.clone())
    );
    assert_eq!(
        Error::source(&error)
            .expect("the pipeline error is exposed as the source")
            .to_string(),
        expected_pipeline.to_string()
    );
}

#[test]
fn from_roots_binding_failure_precedes_missing_sources() {
    let release_fixture = FixtureDir::new("binding-release");
    let development_fixture = FixtureDir::new("binding-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let (mut named, claims, _) = fixture();
    named.canonical_id = "G36-RUST-DECLARATION-PIPELINE-MISMATCH".to_owned();
    let expected = bind_scalar_source_claims(&named, &claims)
        .expect_err("incompatible projections fail binding");

    let error = check_declaration_pipeline_from_roots(
        &named,
        &claims,
        DeclarationSourceRoots::new(&release, &development),
        root_limits(),
    )
    .expect_err("binding must fail before missing files are opened");

    assert_eq!(
        error,
        DeclarationRootPipelineError::Pipeline(DeclarationPipelineError::Binding(expected))
    );
}

#[test]
fn from_roots_source_failure_precedes_syntax_and_preserves_error() {
    let release_fixture = FixtureDir::new("source-release");
    let development_fixture = FixtureDir::new("source-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let (named, claims, _) = fixture();
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let expected = read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(root_limits()),
    )
    .expect_err("fixture sources are absent");

    let error = check_declaration_pipeline_from_roots(&named, &claims, roots, root_limits())
        .expect_err("source acquisition must fail before syntax checking");

    assert_eq!(
        error,
        DeclarationRootPipelineError::Source(expected.clone())
    );
    assert_eq!(
        Error::source(&error)
            .expect("the source error is exposed")
            .to_string(),
        expected.to_string()
    );
}

#[test]
fn from_roots_syntax_failure_after_acquisition_preserves_error() {
    let release_fixture = FixtureDir::new("syntax-release");
    let development_fixture = FixtureDir::new("syntax-development");
    write_fixture_sources(&release_fixture, &development_fixture);
    release_fixture.write(PARAMETER_PATH, b"not valid Modelica");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let (named, claims, _) = fixture();
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let documents = read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(root_limits()),
    )
    .expect("opaque source acquisition succeeds");
    let expected = check_owner_declaration_syntax(
        &requirements,
        &documents,
        DeclarationSyntaxLimits::from(root_limits()),
    )
    .expect_err("changed bytes fail syntax checking");

    let error = check_declaration_pipeline_from_roots(&named, &claims, roots, root_limits())
        .expect_err("syntax failure must not expose bound output");

    assert_eq!(
        error,
        DeclarationRootPipelineError::Pipeline(DeclarationPipelineError::Syntax(expected))
    );
}

#[test]
fn combined_limits_drive_source_and_syntax_stages() {
    let release_fixture = FixtureDir::new("limits-release");
    let development_fixture = FixtureDir::new("limits-development");
    write_fixture_sources(&release_fixture, &development_fixture);
    let release = release_fixture.open();
    let development = development_fixture.open();
    let roots = DeclarationSourceRoots::new(&release, &development);
    let (named, claims, _) = fixture();
    let exact = DeclarationRootPipelineLimits {
        max_documents: 2,
        max_requirements: 2,
        max_source_bytes: PARAMETER_SOURCE.len().max(CONNECTOR_SOURCE.len()),
        max_total_source_bytes: PARAMETER_SOURCE.len() + CONNECTOR_SOURCE.len(),
        max_direct_members: 1,
    };
    assert_eq!(
        DeclarationSourceLimits::from(exact),
        DeclarationSourceLimits {
            max_documents: exact.max_documents,
            max_source_bytes: exact.max_source_bytes,
            max_total_source_bytes: exact.max_total_source_bytes,
        }
    );
    assert_eq!(
        DeclarationSyntaxLimits::from(exact),
        DeclarationSyntaxLimits {
            max_documents: exact.max_documents,
            max_requirements: exact.max_requirements,
            max_source_bytes: exact.max_source_bytes,
            max_total_source_bytes: exact.max_total_source_bytes,
            max_direct_members: exact.max_direct_members,
        }
    );
    check_declaration_pipeline_from_roots(&named, &claims, roots, exact)
        .expect("all five exact inclusive boundaries pass");

    let source_limited = DeclarationRootPipelineLimits {
        max_source_bytes: exact.max_source_bytes - 1,
        ..exact
    };
    let requirements =
        project_declaration_requirements(&claims).expect("requirement projection succeeds");
    let expected_source = read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(source_limited),
    )
    .expect_err("the direct reader rejects the tighter per-source bound");
    let source_error =
        check_declaration_pipeline_from_roots(&named, &claims, roots, source_limited)
            .expect_err("the acquisition stage enforces the shared per-source bound");
    assert_eq!(
        source_error,
        DeclarationRootPipelineError::Source(expected_source)
    );

    let syntax_limited = DeclarationRootPipelineLimits {
        max_direct_members: 0,
        ..exact
    };
    let documents = read_declaration_sources(
        &requirements,
        roots,
        DeclarationSourceLimits::from(syntax_limited),
    )
    .expect("acquisition passes before the syntax-only bound");
    let expected_syntax = check_owner_declaration_syntax(
        &requirements,
        &documents,
        DeclarationSyntaxLimits::from(syntax_limited),
    )
    .expect_err("the direct syntax checker rejects the tighter member bound");
    let syntax_error =
        check_declaration_pipeline_from_roots(&named, &claims, roots, syntax_limited)
            .expect_err("the syntax stage enforces the shared direct-member bound");
    assert_eq!(
        syntax_error,
        DeclarationRootPipelineError::Pipeline(DeclarationPipelineError::Syntax(expected_syntax))
    );
}

#[test]
fn implementation_adds_no_ambient_or_deferred_integrations() {
    let source_text = include_str!("../declaration_pipeline.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "std::env",
        "std::net",
        "std::process",
        "std::io",
        "open_ambient_dir",
        "ambient_authority",
        "serde",
        "serde_json",
        "cxf_json",
        "cxf-json",
        "Command::",
        "git2",
        "clap",
        "Engine",
        "Studio",
        "rand::",
    ] {
        assert!(
            !source_text.contains(forbidden),
            "declaration pipeline must not use {forbidden}"
        );
    }
}
