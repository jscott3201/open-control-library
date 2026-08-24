use std::error::Error;

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
fn implementation_stays_typed_and_in_memory() {
    let source_text = include_str!("../declaration_pipeline.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "std::env",
        "std::net",
        "std::process",
        "std::io",
        "serde",
        "serde_json",
        "cxf_json",
        "cxf-json",
        "Command::",
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
