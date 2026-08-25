use std::env;
use std::path::PathBuf;

use cap_std::{ambient_authority, fs::Dir};
use num_bigint::BigInt;
use ocl_routine_compiler::bound_scalars::{BoundScalarProjection, BoundSourceClaim};
use ocl_routine_compiler::compiler_pipeline::compile_validated_from_roots;
use ocl_routine_compiler::declaration_pipeline::DeclarationRootPipelineLimits;
use ocl_routine_compiler::declaration_source::DeclarationSourceRoots;
use ocl_routine_compiler::resolution::{
    ConnectorDefinition, ConnectorDirection, ConnectorPresence, FiniteReal, ParameterDefinition,
    ParameterSource, ParameterValue, PrimitiveType, ResolutionLimits, ScalarValue, Shape, TypeUse,
    ValidatedResolutionInput,
};
use ocl_routine_compiler::scalar_abi::{EnumAbiMapping, ScalarAbiType, ScalarAbiValue};
use ocl_routine_compiler::scalar_source_claims::{
    SourceClassClaim, SourceFileLocator, SourceInventory, SourceInventoryFile,
    SourceInventoryLicense, SourceInventorySnapshot, SourceMemberBinding, SourceOwnerKind,
    SourcePin, SourceSnapshotRole,
};

const RELEASE_ROOT_ENV: &str = "OCL_G36_RELEASE_ROOT";
const DEVELOPMENT_ROOT_ENV: &str = "OCL_G36_DEVELOPMENT_ROOT";
const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const RELEASE_ROOT_TREE: &str = "sha1:8bc765b009ed0dcf6907021f073ea77160088bd8";
const DEVELOPMENT_ROOT_TREE: &str = "sha1:6286a3d534846a187444d3b4bee72a1ae944a458";
const TRIM_AND_RESPOND_CLASS: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";
const TRIM_AND_RESPOND_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const TRIM_AND_RESPOND_BYTES: usize = 25_452;
const TRIM_AND_RESPOND_BLOB: &str = "sha1:028439a4fb478fc041d703a092d5186f5861eb03";
const TRIM_AND_RESPOND_SHA256: &str =
    "sha256:1bf9ab68904baa00553d6b43ecd0d04411e6106d196881ad67c9885827507981";
const TEST_CANONICAL_ID: &str = "TEST-ONLY-G36-TRIM-AND-RESPOND";

#[derive(Clone, Debug, PartialEq)]
struct IntegrationInputs {
    resolution: ValidatedResolutionInput,
    enum_mappings: Vec<EnumAbiMapping>,
    inventory: SourceInventory,
    pins: Vec<SourcePin>,
    claims: Vec<SourceClassClaim>,
    bindings: Vec<SourceMemberBinding>,
}

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("test value is finite")
}

fn source_file() -> SourceInventoryFile {
    SourceInventoryFile {
        path: TRIM_AND_RESPOND_PATH.to_owned(),
        mode: "100644".to_owned(),
        bytes: BigInt::from(TRIM_AND_RESPOND_BYTES),
        git_blob_sha1: TRIM_AND_RESPOND_BLOB.to_owned(),
        sha256: TRIM_AND_RESPOND_SHA256.to_owned(),
    }
}

fn snapshot(
    role: SourceSnapshotRole,
    revision: &str,
    root_tree_sha1: &str,
) -> SourceInventorySnapshot {
    SourceInventorySnapshot {
        role,
        revision: revision.to_owned(),
        root_tree_sha1: root_tree_sha1.to_owned(),
        file_count: BigInt::from(1_u8),
        total_bytes: BigInt::from(TRIM_AND_RESPOND_BYTES),
        modelica_file_count: BigInt::from(1_u8),
        package_order_count: BigInt::from(0_u8),
        files: vec![source_file()],
    }
}

fn integration_inputs() -> IntegrationInputs {
    IntegrationInputs {
        resolution: ValidatedResolutionInput {
            canonical_id: TEST_CANONICAL_ID.to_owned(),
            revision: BigInt::from(1_u8),
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
                connector_id: "num_of_req".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Always,
            }],
        },
        enum_mappings: Vec::new(),
        // This one-file subset drives the integration. The preceding Python CI gate
        // validates checkout provenance and every row in the checked-in inventory.
        inventory: SourceInventory {
            schema: "cxf-library/g36-source-inventory/v1".to_owned(),
            repository: "https://github.com/lbl-srg/modelica-buildings.git".to_owned(),
            source_root: "Buildings/Controls/OBC/ASHRAE/G36".to_owned(),
            inventory_scope: "source-root-regular-files".to_owned(),
            dependency_closure: "not-inventoried".to_owned(),
            license: SourceInventoryLicense {
                upstream_path: "Buildings/legal.html".to_owned(),
                retained_path: "routines/g36/LICENSE-BUILDINGS.html".to_owned(),
                git_blob_sha1: "sha1:b542af56c10d3769d42a79ad45d78329ad2dbd5f".to_owned(),
                sha256: "sha256:79ad0f2d053b92d93a7e7b200b9d7ef4c1bb2097aad695cda31ad9a23a57e921"
                    .to_owned(),
            },
            snapshots: vec![
                snapshot(
                    SourceSnapshotRole::Release,
                    RELEASE_REVISION,
                    RELEASE_ROOT_TREE,
                ),
                snapshot(
                    SourceSnapshotRole::Development,
                    DEVELOPMENT_REVISION,
                    DEVELOPMENT_ROOT_TREE,
                ),
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
        claims: vec![SourceClassClaim {
            canonical_class_path: TRIM_AND_RESPOND_CLASS.to_owned(),
            snapshot: SourceSnapshotRole::Release,
            revision: RELEASE_REVISION.to_owned(),
            file: SourceFileLocator {
                path: TRIM_AND_RESPOND_PATH.to_owned(),
                git_blob_sha1: TRIM_AND_RESPOND_BLOB.to_owned(),
            },
        }],
        bindings: vec![
            SourceMemberBinding {
                owner_kind: SourceOwnerKind::Parameter,
                owner_id: "sample_period_s".to_owned(),
                canonical_class_path: TRIM_AND_RESPOND_CLASS.to_owned(),
                source_member: "samplePeriod".to_owned(),
            },
            SourceMemberBinding {
                owner_kind: SourceOwnerKind::Connector,
                owner_id: "num_of_req".to_owned(),
                canonical_class_path: TRIM_AND_RESPOND_CLASS.to_owned(),
                source_member: "numOfReq".to_owned(),
            },
        ],
    }
}

fn open_root(variable: &str) -> Dir {
    let value = env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} must be set for this integration test"));
    assert!(
        !value.is_empty(),
        "{variable} must not be empty for this integration test"
    );
    Dir::open_ambient_dir(PathBuf::from(value), ambient_authority())
        .unwrap_or_else(|error| panic!("{variable} must name an openable directory: {error}"))
}

fn compile(inputs: &IntegrationInputs, roots: DeclarationSourceRoots<'_>) -> BoundScalarProjection {
    compile_validated_from_roots(
        &inputs.resolution,
        ResolutionLimits {
            max_guard_depth: 0,
            max_guard_nodes: 0,
            max_scalar_leaves: 2,
        },
        &inputs.enum_mappings,
        &inputs.inventory,
        &inputs.pins,
        &inputs.claims,
        &inputs.bindings,
        roots,
        DeclarationRootPipelineLimits {
            max_documents: 1,
            max_requirements: 2,
            max_source_bytes: TRIM_AND_RESPOND_BYTES,
            max_total_source_bytes: TRIM_AND_RESPOND_BYTES,
            max_direct_members: 64,
        },
    )
    .expect("real pinned TrimAndRespond compiles through the typed pipeline")
}

fn assert_release_claim(claim: &BoundSourceClaim, source_member: &str) {
    assert_eq!(claim.canonical_class_path, TRIM_AND_RESPOND_CLASS);
    assert_eq!(claim.source_member, source_member);
    assert_eq!(claim.snapshot, SourceSnapshotRole::Release);
    assert_eq!(claim.revision, RELEASE_REVISION);
    assert_eq!(claim.file.path, TRIM_AND_RESPOND_PATH);
    assert_eq!(claim.file.git_blob_sha1, TRIM_AND_RESPOND_BLOB);
}

#[test]
#[ignore = "requires caller-supplied pinned Modelica Buildings roots"]
fn real_pinned_trim_and_respond_compiles_through_public_pipeline() {
    let release = open_root(RELEASE_ROOT_ENV);
    let development = open_root(DEVELOPMENT_ROOT_ENV);
    let roots = DeclarationSourceRoots::new(&release, &development);
    let inputs = integration_inputs();
    let before = inputs.clone();

    // Generic syntax checks direct identity and public members, not declared types,
    // connector direction, inheritance, dependencies, compilation, or behavior.
    // The legacy verifier separately checks exact declared member types.
    let first = compile(&inputs, roots);
    let second = compile(&inputs, roots);

    assert_eq!(first, second);
    assert_eq!(inputs, before);
    assert_eq!(first.canonical_id, TEST_CANONICAL_ID);
    assert_eq!(first.revision, BigInt::from(1_u8));
    assert_eq!(first.parameters.len(), 1);
    assert_eq!(first.connectors.len(), 1);

    let parameter = &first.parameters[0];
    assert_eq!(parameter.parameter_id, "sample_period_s");
    assert!(parameter.coordinates.is_empty());
    assert_eq!(parameter.scalar_name, "p_73616d706c655f706572696f645f73");
    assert_eq!(
        parameter.abi_type,
        ScalarAbiType::Primitive(PrimitiveType::Real)
    );
    assert_eq!(parameter.source, ParameterSource::Default);
    assert_eq!(parameter.value, ScalarAbiValue::Real(finite(60.0)));
    assert_release_claim(&parameter.source_claim, "samplePeriod");

    let connector = &first.connectors[0];
    assert_eq!(connector.connector_id, "num_of_req");
    assert!(connector.coordinates.is_empty());
    assert_eq!(connector.scalar_name, "c_6e756d5f6f665f726571");
    assert_eq!(
        connector.abi_type,
        ScalarAbiType::Primitive(PrimitiveType::Integer)
    );
    assert_eq!(connector.direction, ConnectorDirection::Input);
    assert_release_claim(&connector.source_claim, "numOfReq");

    assert_eq!(inputs.claims.len(), 1);
    assert_eq!(inputs.claims[0].snapshot, SourceSnapshotRole::Release);
    assert_eq!(
        inputs.inventory.snapshots[1].role,
        SourceSnapshotRole::Development
    );
}
