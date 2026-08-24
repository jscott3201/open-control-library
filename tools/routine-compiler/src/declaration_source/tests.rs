use std::collections::HashSet;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use cap_std::ambient_authority;
use num_bigint::BigInt;
use sha1::{Digest, Sha1};

use super::*;
use crate::declaration_requirements::{
    ConnectorDeclarationRequirement, ParameterDeclarationRequirement,
};
use crate::declaration_syntax::{DeclarationSyntaxLimits, check_owner_declaration_syntax};
use crate::scalar_names::build_scalar_name;

const RELEASE_REVISION: &str = "55abf579598ca81cae0a82f337350375958e6722";
const DEVELOPMENT_REVISION: &str = "eccb40b3974bb10eef120c5670a6454e43ca36e3";
const OTHER_REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PATH_A: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/A.mo";
const PATH_B: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/B.mo";
const PATH_C: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/C.mo";
const CLASS_PATH: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.Test";
const BLOB_A: &str = "sha1:1111111111111111111111111111111111111111";
const BLOB_B: &str = "sha1:2222222222222222222222222222222222222222";

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ocl-declaration-source-{label}-{}-{sequence}",
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

    fn create_dir(&self, relative: &str) {
        fs::create_dir_all(self.path.join(relative)).expect("fixture directory builds");
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

fn locator(path: &str, blob: &str) -> SourceFileLocator {
    SourceFileLocator {
        path: path.to_owned(),
        git_blob_sha1: blob.to_owned(),
    }
}

fn parameter(
    parameter_id: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    path: &str,
    blob: &str,
) -> ParameterDeclarationRequirement {
    ParameterDeclarationRequirement {
        parameter_id: parameter_id.to_owned(),
        canonical_class_path: CLASS_PATH.to_owned(),
        source_member: "gain".to_owned(),
        snapshot,
        revision: revision.to_owned(),
        file: locator(path, blob),
        scalar_names: vec![
            build_scalar_name("p_", parameter_id, &[]).expect("fixture scalar name builds"),
        ],
    }
}

fn connector(
    connector_id: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    path: &str,
    blob: &str,
) -> ConnectorDeclarationRequirement {
    ConnectorDeclarationRequirement {
        connector_id: connector_id.to_owned(),
        canonical_class_path: CLASS_PATH.to_owned(),
        source_member: "signal".to_owned(),
        snapshot,
        revision: revision.to_owned(),
        file: locator(path, blob),
        scalar_names: vec![
            build_scalar_name("c_", connector_id, &[]).expect("fixture scalar name builds"),
        ],
    }
}

fn projection(
    parameters: Vec<ParameterDeclarationRequirement>,
    connectors: Vec<ConnectorDeclarationRequirement>,
) -> DeclarationRequirementProjection {
    DeclarationRequirementProjection {
        canonical_id: "source-reader-fixture".to_owned(),
        revision: BigInt::from(1_u8),
        parameters,
        connectors,
    }
}

fn limits() -> DeclarationSourceLimits {
    DeclarationSourceLimits {
        max_documents: 16,
        max_source_bytes: 64 * 1024,
        max_total_source_bytes: 256 * 1024,
    }
}

fn roots<'a>(release: &'a Dir, development: &'a Dir) -> DeclarationSourceRoots<'a> {
    DeclarationSourceRoots::new(release, development)
}

fn codes(error: &DeclarationSourceError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn syntax_codes(error: &crate::declaration_syntax::DeclarationSyntaxError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn syntax_limits() -> DeclarationSyntaxLimits {
    DeclarationSyntaxLimits {
        max_documents: 16,
        max_requirements: 16,
        max_source_bytes: 64 * 1024,
        max_total_source_bytes: 256 * 1024,
        max_direct_members: 128,
    }
}

fn git_blob(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"blob ");
    hasher.update(bytes.len().to_string().as_bytes());
    hasher.update([0_u8]);
    hasher.update(bytes);
    format!("sha1:{:x}", hasher.finalize())
}

fn identity() -> SourceIdentity<'static> {
    SourceIdentity {
        snapshot: SourceSnapshotRole::Release,
        revision: RELEASE_REVISION,
        path: PATH_A,
        blob: BLOB_A,
    }
}

#[test]
fn snapshot_roles_are_isolated_and_never_fall_back() {
    let release_fixture = FixtureDir::new("role-release");
    let development_fixture = FixtureDir::new("role-development");
    release_fixture.write(PATH_A, b"release bytes");
    development_fixture.write(PATH_A, b"development bytes");
    development_fixture.write(PATH_B, b"development only");
    let release = release_fixture.open();
    let development = development_fixture.open();

    let requirements = projection(
        vec![parameter(
            "release",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            BLOB_A,
        )],
        vec![connector(
            "development",
            SourceSnapshotRole::Development,
            DEVELOPMENT_REVISION,
            PATH_A,
            BLOB_A,
        )],
    );
    let documents =
        read_declaration_sources(&requirements, roots(&release, &development), limits())
            .expect("both role-specific files are present");
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].snapshot, SourceSnapshotRole::Release);
    assert_eq!(documents[0].bytes, b"release bytes");
    assert_eq!(documents[1].snapshot, SourceSnapshotRole::Development);
    assert_eq!(documents[1].bytes, b"development bytes");

    let no_fallback = projection(
        vec![parameter(
            "release_missing",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_B,
            BLOB_B,
        )],
        Vec::new(),
    );
    let error = read_declaration_sources(&no_fallback, roots(&release, &development), limits())
        .expect_err("the development copy must not satisfy a release identity");
    assert_eq!(codes(&error), HashSet::from(["source_not_found"]));
}

#[test]
fn full_identity_deduplicates_exactly_and_preserves_conflicting_claims() {
    let release_fixture = FixtureDir::new("identity-release");
    let development_fixture = FixtureDir::new("identity-development");
    let source = b"within Buildings.Controls.OBC.ASHRAE.G36.Generic;\nblock Test\n  parameter Real gain;\nend Test;\n";
    release_fixture.write(PATH_A, source);
    let release = release_fixture.open();
    let development = development_fixture.open();
    let actual_blob = git_blob(source);
    let wrong_blob = format!("sha1:{}", "0".repeat(40));
    let requirements = projection(
        vec![
            parameter(
                "first",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                &actual_blob,
            ),
            parameter(
                "duplicate",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                &actual_blob,
            ),
            parameter(
                "conflicting_blob",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                &wrong_blob,
            ),
        ],
        Vec::new(),
    );

    let documents = read_declaration_sources(
        &requirements,
        roots(&release, &development),
        DeclarationSourceLimits {
            max_documents: 2,
            ..limits()
        },
    )
    .expect("exact duplicates do not consume the document bound twice");
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].file.git_blob_sha1, wrong_blob);
    assert_eq!(documents[1].file.git_blob_sha1, actual_blob);
    assert!(documents.iter().all(|document| document.bytes == source));

    let downstream_requirements = projection(
        vec![parameter(
            "conflicting_blob",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            &wrong_blob,
        )],
        Vec::new(),
    );
    let wrong_document = documents
        .iter()
        .find(|document| document.file.git_blob_sha1 == wrong_blob)
        .expect("conflicting identity is present")
        .clone();
    let error = check_owner_declaration_syntax(
        &downstream_requirements,
        &[wrong_document],
        syntax_limits(),
    )
    .expect_err("the downstream checker distinguishes the wrong claim");
    assert!(syntax_codes(&error).contains("source_blob_mismatch"));
}

#[test]
fn requirement_order_does_not_change_full_identity_order() {
    let release_fixture = FixtureDir::new("order-release");
    let development_fixture = FixtureDir::new("order-development");
    release_fixture.write(PATH_A, b"a");
    release_fixture.write(PATH_C, b"c");
    development_fixture.write(PATH_A, b"development a");
    development_fixture.write(PATH_B, b"development b");
    let release = release_fixture.open();
    let development = development_fixture.open();

    let parameters = vec![
        parameter(
            "development_a",
            SourceSnapshotRole::Development,
            DEVELOPMENT_REVISION,
            PATH_A,
            BLOB_A,
        ),
        parameter(
            "release_c",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_C,
            BLOB_A,
        ),
    ];
    let connectors = vec![
        connector(
            "development_b",
            SourceSnapshotRole::Development,
            DEVELOPMENT_REVISION,
            PATH_B,
            BLOB_B,
        ),
        connector(
            "release_a",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            BLOB_B,
        ),
    ];
    let forward = projection(parameters.clone(), connectors.clone());
    let reverse = projection(
        parameters.into_iter().rev().collect(),
        connectors.into_iter().rev().collect(),
    );

    let forward_documents =
        read_declaration_sources(&forward, roots(&release, &development), limits())
            .expect("forward requirements read");
    let reverse_documents =
        read_declaration_sources(&reverse, roots(&release, &development), limits())
            .expect("reverse requirements read");
    assert_eq!(forward_documents, reverse_documents);
    assert_eq!(forward_documents[0].snapshot, SourceSnapshotRole::Release);
    assert_eq!(forward_documents[0].file.path, PATH_A);
    assert_eq!(forward_documents[1].snapshot, SourceSnapshotRole::Release);
    assert_eq!(forward_documents[1].file.path, PATH_C);
    assert_eq!(
        forward_documents[2].snapshot,
        SourceSnapshotRole::Development
    );
    assert_eq!(forward_documents[2].file.path, PATH_A);
    assert_eq!(forward_documents[3].file.path, PATH_B);
}

#[test]
fn inclusive_document_per_file_and_total_boundaries_are_atomic() {
    let release_fixture = FixtureDir::new("boundaries-release");
    let development_fixture = FixtureDir::new("boundaries-development");
    release_fixture.write(PATH_A, b"1234");
    release_fixture.write(PATH_B, b"56");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let requirements = projection(
        vec![
            parameter(
                "a",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                BLOB_A,
            ),
            parameter(
                "b",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_B,
                BLOB_B,
            ),
        ],
        Vec::new(),
    );
    let exact = DeclarationSourceLimits {
        max_documents: 2,
        max_source_bytes: 4,
        max_total_source_bytes: 6,
    };
    let documents = read_declaration_sources(&requirements, roots(&release, &development), exact)
        .expect("every exact boundary is inclusive");
    assert_eq!(documents.len(), 2);
    assert_eq!(
        documents
            .iter()
            .map(|document| document.bytes.len())
            .sum::<usize>(),
        6
    );

    for over_limit in [
        DeclarationSourceLimits {
            max_documents: 1,
            ..exact
        },
        DeclarationSourceLimits {
            max_source_bytes: 3,
            ..exact
        },
        DeclarationSourceLimits {
            max_total_source_bytes: 5,
            ..exact
        },
    ] {
        let error =
            read_declaration_sources(&requirements, roots(&release, &development), over_limit)
                .expect_err("one beyond an inclusive boundary fails without output");
        assert!(codes(&error).contains("resource_limit"));
    }
}

#[test]
fn unsafe_paths_fail_before_any_root_access() {
    let release_fixture = FixtureDir::new("unsafe-release");
    let development_fixture = FixtureDir::new("unsafe-development");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let paths = [
        "",
        "/Buildings/Controls/OBC/ASHRAE/G36/Bad.mo",
        "Buildings\\Controls\\OBC\\ASHRAE\\G36\\Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/Bad\n.mo",
        "Buildings/Controls/OBC/ASHRAE/G36//Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/./Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/../Bad.mo",
        "Buildings/Controls/OBC/ASHRAE/Outside.mo",
        "Buildings/Controls/OBC/ASHRAE/G36/Bad.txt",
    ];

    for (index, path) in paths.into_iter().enumerate() {
        let requirements = projection(
            vec![parameter(
                &format!("unsafe_{index}"),
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                path,
                BLOB_A,
            )],
            Vec::new(),
        );
        let error =
            read_declaration_sources(&requirements, roots(&release, &development), limits())
                .expect_err("unsafe paths are rejected lexically");
        assert_eq!(codes(&error), HashSet::from(["invalid_source_path"]));
    }
}

#[test]
fn missing_and_non_regular_entries_return_sorted_stable_diagnostics() {
    let release_fixture = FixtureDir::new("entries-release");
    let development_fixture = FixtureDir::new("entries-development");
    release_fixture.create_dir(PATH_B);
    let release = release_fixture.open();
    let development = development_fixture.open();
    let requirements = projection(
        vec![
            parameter(
                "missing",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                BLOB_A,
            ),
            parameter(
                "directory",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_B,
                BLOB_B,
            ),
        ],
        Vec::new(),
    );

    let error = read_declaration_sources(&requirements, roots(&release, &development), limits())
        .expect_err("all preflight failures make the result atomic");
    assert_eq!(
        codes(&error),
        HashSet::from(["source_not_found", "source_not_regular"])
    );
    assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(error.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains(&release_fixture.path.display().to_string())
    }));
    assert!(error.diagnostics.iter().all(|diagnostic| {
        diagnostic.location.contains("$.sources.release")
            && diagnostic.location.contains(RELEASE_REVISION)
            && diagnostic.location.contains("blob=\"sha1:")
    }));
}

#[test]
fn io_error_kinds_map_without_platform_messages_or_paths() {
    let source = identity();
    let cases = [
        (
            IoOperation::Open,
            io::ErrorKind::NotFound,
            "source_not_found",
            "source entry was not found",
        ),
        (
            IoOperation::Open,
            io::ErrorKind::PermissionDenied,
            "source_access_denied",
            "source entry access was denied",
        ),
        (
            IoOperation::Open,
            io::ErrorKind::InvalidInput,
            "source_path_unavailable",
            "source entry could not be resolved within the supplied root",
        ),
        (
            IoOperation::Metadata,
            io::ErrorKind::Other,
            "source_metadata_failed",
            "opened source metadata could not be read",
        ),
        (
            IoOperation::Read,
            io::ErrorKind::Other,
            "source_read_failed",
            "opened source bytes could not be read",
        ),
    ];
    for (operation, kind, code, message) in cases {
        let diagnostic = io_diagnostic(source, operation, kind);
        assert_eq!(diagnostic.code, code);
        assert_eq!(diagnostic.message, message);
        assert!(!diagnostic.message.contains(PATH_A));
    }
}

#[cfg(unix)]
#[test]
fn external_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let release_fixture = FixtureDir::new("external-link-release");
    let development_fixture = FixtureDir::new("external-link-development");
    let outside_fixture = FixtureDir::new("external-link-outside");
    outside_fixture.write("Outside.mo", b"outside");
    let link = release_fixture.path.join(PATH_A);
    fs::create_dir_all(link.parent().expect("link has parent")).expect("link parent builds");
    symlink(outside_fixture.path.join("Outside.mo"), link).expect("external link builds");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let requirements = projection(
        vec![parameter(
            "external_link",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            BLOB_A,
        )],
        Vec::new(),
    );

    let error = read_declaration_sources(&requirements, roots(&release, &development), limits())
        .expect_err("a link outside the supplied root cannot be opened");
    assert_eq!(codes(&error), HashSet::from(["source_access_denied"]));
}

#[cfg(unix)]
#[test]
fn internal_symlink_follows_capability_resolution() {
    use std::os::unix::fs::symlink;

    let release_fixture = FixtureDir::new("internal-link-release");
    let development_fixture = FixtureDir::new("internal-link-development");
    let target_path = "Buildings/Controls/OBC/ASHRAE/G36/Generic/Target.mo";
    release_fixture.write(target_path, b"internal target");
    let link = release_fixture.path.join(PATH_A);
    symlink(Path::new("Target.mo"), link).expect("internal link builds");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let requirements = projection(
        vec![parameter(
            "internal_link",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            BLOB_A,
        )],
        Vec::new(),
    );

    let documents =
        read_declaration_sources(&requirements, roots(&release, &development), limits())
            .expect("an in-root relative link resolves inside the capability");
    assert_eq!(documents[0].bytes, b"internal target");
}

struct FailingReader {
    kind: io::ErrorKind,
}

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::from(self.kind))
    }
}

struct InterruptedThenData {
    interrupted: bool,
    data: Cursor<&'static [u8]>,
}

impl Read for InterruptedThenData {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        self.data.read(buffer)
    }
}

#[test]
fn bounded_reader_detects_size_changes_growth_limits_and_read_failures() {
    let roomy = DeclarationSourceLimits {
        max_documents: 1,
        max_source_bytes: 16,
        max_total_source_bytes: 16,
    };
    let grown = bounded_read(&mut Cursor::new(b"12345"), identity(), 4, 0, roomy)
        .expect_err("growth within policy is still a metadata inconsistency");
    assert_eq!(codes(&grown), HashSet::from(["source_changed"]));
    let shrunk = bounded_read(&mut Cursor::new(b"123"), identity(), 4, 0, roomy)
        .expect_err("shrinkage is a metadata inconsistency");
    assert_eq!(codes(&shrunk), HashSet::from(["source_changed"]));

    let per_file = bounded_read(
        &mut Cursor::new(b"12345"),
        identity(),
        4,
        0,
        DeclarationSourceLimits {
            max_source_bytes: 4,
            ..roomy
        },
    )
    .expect_err("the probe detects one byte over the per-file limit");
    assert_eq!(codes(&per_file), HashSet::from(["resource_limit"]));
    assert!(
        per_file.diagnostics[0]
            .message
            .contains("max_source_bytes 4")
    );

    let total = bounded_read(
        &mut Cursor::new(b"123"),
        identity(),
        2,
        2,
        DeclarationSourceLimits {
            max_total_source_bytes: 4,
            ..roomy
        },
    )
    .expect_err("the probe detects one byte over the remaining total");
    assert_eq!(codes(&total), HashSet::from(["resource_limit"]));
    assert!(
        total.diagnostics[0]
            .message
            .contains("max_total_source_bytes 4")
    );

    let read_error = bounded_read(
        &mut FailingReader {
            kind: io::ErrorKind::Other,
        },
        identity(),
        0,
        0,
        roomy,
    )
    .expect_err("read errors are mapped without their platform text");
    assert_eq!(codes(&read_error), HashSet::from(["source_read_failed"]));

    let mut interrupted = InterruptedThenData {
        interrupted: false,
        data: Cursor::new(b"ok"),
    };
    assert_eq!(
        bounded_read(&mut interrupted, identity(), 2, 0, roomy)
            .expect("an interrupted read is retried"),
        b"ok"
    );
}

#[test]
fn resource_helpers_fail_without_large_allocations() {
    assert_eq!(checked_diagnostic_capacity(2), Some(5));
    assert_eq!(checked_diagnostic_capacity(usize::MAX), None);

    let overflow = bounded_read(
        &mut Cursor::new(&[]),
        identity(),
        1,
        usize::MAX,
        DeclarationSourceLimits {
            max_documents: 1,
            max_source_bytes: 1,
            max_total_source_bytes: usize::MAX,
        },
    )
    .expect_err("metadata total overflow fails before allocation");
    assert_eq!(codes(&overflow), HashSet::from(["resource_limit"]));
    assert!(overflow.diagnostics[0].message.contains("overflows usize"));

    let mut values = Vec::<u8>::new();
    let error = reserve_exact(&mut values, usize::MAX, "$.test", "test vector")
        .expect_err("impossible capacity fails fallibly");
    assert!(values.is_empty());
    assert_eq!(error.diagnostics[0].code, "resource_limit");
    assert_eq!(error.diagnostics[0].location, "$.test");
}

#[test]
fn opaque_bytes_are_preserved_and_left_to_the_downstream_checker() {
    let release_fixture = FixtureDir::new("opaque-release");
    let development_fixture = FixtureDir::new("opaque-development");
    let cases: [(&str, &[u8], Option<String>, &str); 3] = [
        (
            PATH_A,
            b"wrong claimed content",
            None,
            "source_blob_mismatch",
        ),
        (PATH_B, b"\xff", Some(git_blob(b"\xff")), "source_not_utf8"),
        (
            PATH_C,
            b"this is not Modelica;",
            Some(git_blob(b"this is not Modelica;")),
            "modelica_parse_failed",
        ),
    ];
    for (path, bytes, _, _) in &cases {
        release_fixture.write(path, bytes);
    }
    let release = release_fixture.open();
    let development = development_fixture.open();

    for (index, (path, bytes, matching_blob, expected_code)) in cases.into_iter().enumerate() {
        let claimed_blob = matching_blob.unwrap_or_else(|| BLOB_A.to_owned());
        let requirements = projection(
            vec![parameter(
                &format!("opaque_{index}"),
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                path,
                &claimed_blob,
            )],
            Vec::new(),
        );
        let documents =
            read_declaration_sources(&requirements, roots(&release, &development), limits())
                .expect("acquisition accepts opaque bytes");
        assert_eq!(documents[0].bytes, bytes);
        let error = check_owner_declaration_syntax(&requirements, &documents, syntax_limits())
            .expect_err("the downstream checker owns content validation");
        assert!(syntax_codes(&error).contains(expected_code));
    }
}

#[test]
fn acquisition_is_repeatable_preserves_inputs_and_never_returns_partial_output() {
    let release_fixture = FixtureDir::new("atomic-release");
    let development_fixture = FixtureDir::new("atomic-development");
    release_fixture.write(PATH_A, b"present");
    let release = release_fixture.open();
    let development = development_fixture.open();
    let successful = projection(
        vec![parameter(
            "present",
            SourceSnapshotRole::Release,
            RELEASE_REVISION,
            PATH_A,
            BLOB_A,
        )],
        Vec::new(),
    );
    let successful_before = successful.clone();
    let first = read_declaration_sources(&successful, roots(&release, &development), limits())
        .expect("first acquisition succeeds");
    let second = read_declaration_sources(&successful, roots(&release, &development), limits())
        .expect("second acquisition succeeds");
    assert_eq!(first, second);
    assert_eq!(successful, successful_before);

    let failing = projection(
        vec![
            parameter(
                "present",
                SourceSnapshotRole::Release,
                RELEASE_REVISION,
                PATH_A,
                BLOB_A,
            ),
            parameter(
                "missing",
                SourceSnapshotRole::Release,
                OTHER_REVISION,
                PATH_B,
                BLOB_B,
            ),
        ],
        Vec::new(),
    );
    let failing_before = failing.clone();
    let first_error = read_declaration_sources(&failing, roots(&release, &development), limits())
        .expect_err("one missing identity makes the whole acquisition fail");
    let second_error = read_declaration_sources(&failing, roots(&release, &development), limits())
        .expect_err("the repeated failure is deterministic");
    assert_eq!(first_error, second_error);
    assert_eq!(failing, failing_before);
}

#[test]
fn product_module_uses_only_capability_relative_bounded_acquisition() {
    let source = include_str!("../declaration_source.rs");
    for forbidden in [
        "open_ambient_dir",
        "use std::fs",
        "canonicalize",
        "read_to_end",
        "read_to_string",
        "Dir::read",
        "Sha1::",
        "Sha256",
        "Digest",
        "use sha1",
        "use sha2",
        "from_utf8",
        "String::from_utf8",
        "rumoca",
        "serde",
        "json",
        "inventory",
        "SOURCE_PIN",
        "SOURCE_RELEASE_PIN",
        "SOURCE_DEVELOPMENT_PIN",
        "source-inventory",
        "git2",
        "std::process",
        "Command::",
        "std::net",
        "reqwest",
        "std::env",
        "Engine",
        "engine",
        "Studio",
        "studio",
        "cxf_json",
        "cxf-json",
    ] {
        assert!(
            !source.contains(forbidden),
            "product module contains excluded marker {forbidden}"
        );
    }
    assert!(source.contains(".open(identity.path)"));
    assert!(source.contains("reader.read(&mut buffer[..probe_bytes])"));
    assert!(source.contains("try_reserve_exact"));
}
