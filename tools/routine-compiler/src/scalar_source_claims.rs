//! Inventory-anchored scalar source claims for the routine compiler.
//!
//! Inventory membership proves only an exact path and Git blob in one pinned
//! snapshot. Modelica class paths and source members remain caller claims; this
//! module does not inspect declarations or define a persisted source map.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use num_bigint::BigInt;

use crate::scalar_abi::ScalarCoordinate;
use crate::scalar_names::{
    NamedScalarConnectorRow, NamedScalarParameterRow, NamedScalarProjection,
};

const INVENTORY_SCHEMA: &str = "cxf-library/g36-source-inventory/v1";
const UPSTREAM_REPOSITORY: &str = "https://github.com/lbl-srg/modelica-buildings.git";
const SOURCE_ROOT: &str = "Buildings/Controls/OBC/ASHRAE/G36";
const INVENTORY_SCOPE: &str = "source-root-regular-files";
const DEPENDENCY_CLOSURE: &str = "not-inventoried";
const LICENSE_UPSTREAM_PATH: &str = "Buildings/legal.html";
const LICENSE_RETAINED_PATH: &str = "routines/g36/LICENSE-BUILDINGS.html";
const SOURCE_ROOT_PREFIX: &str = "Buildings/Controls/OBC/ASHRAE/G36/";
const CLASS_PATH_PREFIX: &str = "Buildings.Controls.OBC.ASHRAE.G36.";
const MAX_IDENTIFIER_LENGTH: usize = 255;
const MAX_CLASS_PATH_LENGTH: usize = 1024;

/// One of the two independently pinned G36 source snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceSnapshotRole {
    Release,
    Development,
}

impl SourceSnapshotRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Development => "development",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Release => 0,
            Self::Development => 1,
        }
    }
}

/// Namespace of a named scalar owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceOwnerKind {
    Parameter,
    Connector,
}

impl SourceOwnerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Connector => "connector",
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::Parameter => Self::Connector,
            Self::Connector => Self::Parameter,
        }
    }
}

/// Caller-supplied snapshot role and exact Git revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePin {
    pub role: SourceSnapshotRole,
    pub revision: String,
}

/// License record carried by the typed source inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInventoryLicense {
    pub upstream_path: String,
    pub retained_path: String,
    pub git_blob_sha1: String,
    pub sha256: String,
}

/// One regular file in a source snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInventoryFile {
    pub path: String,
    pub mode: String,
    pub bytes: BigInt,
    pub git_blob_sha1: String,
    pub sha256: String,
}

/// One ordered release or development inventory snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInventorySnapshot {
    pub role: SourceSnapshotRole,
    pub revision: String,
    pub root_tree_sha1: String,
    pub file_count: BigInt,
    pub total_bytes: BigInt,
    pub modelica_file_count: BigInt,
    pub package_order_count: BigInt,
    pub files: Vec<SourceInventoryFile>,
}

/// Typed in-memory form of the governed G36 source inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInventory {
    pub schema: String,
    pub repository: String,
    pub source_root: String,
    pub inventory_scope: String,
    pub dependency_closure: String,
    pub license: SourceInventoryLicense,
    /// Governed order is release then development.
    pub snapshots: Vec<SourceInventorySnapshot>,
}

/// A G36 path and Git blob pair whose snapshot membership is validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFileLocator {
    pub path: String,
    pub git_blob_sha1: String,
}

/// Caller claim that associates a Modelica class path with one inventoried file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceClassClaim {
    pub canonical_class_path: String,
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
}

/// Caller claim that associates one named owner with a Modelica source member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMemberBinding {
    pub owner_kind: SourceOwnerKind,
    pub owner_id: String,
    pub canonical_class_path: String,
    pub source_member: String,
}

/// One parameter leaf joined to an inventory-anchored caller source claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarParameterSourceClaim {
    pub scalar_name: String,
    pub parameter_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub canonical_class_path: String,
    pub source_member: String,
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
}

/// One connector leaf joined to an inventory-anchored caller source claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarConnectorSourceClaim {
    pub scalar_name: String,
    pub connector_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub canonical_class_path: String,
    pub source_member: String,
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
}

/// Borrowed result of a scalar-name lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarSourceClaimRef<'a> {
    Parameter(&'a ScalarParameterSourceClaim),
    Connector(&'a ScalarConnectorSourceClaim),
}

enum SourceRows<'a> {
    Parameters(std::slice::Iter<'a, ScalarParameterSourceClaim>),
    Connectors(std::slice::Iter<'a, ScalarConnectorSourceClaim>),
}

/// Forward-row iterator returned by [`ScalarSourceClaimProjection::scalar_names_for_source`].
pub struct ScalarNamesForSource<'a> {
    rows: SourceRows<'a>,
    canonical_class_path: &'a str,
    source_member: &'a str,
}

impl<'a> Iterator for ScalarNamesForSource<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.rows {
            SourceRows::Parameters(rows) => rows.find_map(|row| {
                (row.canonical_class_path == self.canonical_class_path
                    && row.source_member == self.source_member)
                    .then_some(row.scalar_name.as_str())
            }),
            SourceRows::Connectors(rows) => rows.find_map(|row| {
                (row.canonical_class_path == self.canonical_class_path
                    && row.source_member == self.source_member)
                    .then_some(row.scalar_name.as_str())
            }),
        }
    }
}

/// Detached scalar source-claim rows in named projection order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarSourceClaimProjection {
    pub canonical_id: String,
    pub revision: BigInt,
    pub parameters: Vec<ScalarParameterSourceClaim>,
    pub connectors: Vec<ScalarConnectorSourceClaim>,
}

impl ScalarSourceClaimProjection {
    /// Finds the unique forward row with this scalar name.
    pub fn claim_for_scalar(&self, scalar_name: &str) -> Option<ScalarSourceClaimRef<'_>> {
        self.parameters
            .iter()
            .find(|row| row.scalar_name == scalar_name)
            .map(ScalarSourceClaimRef::Parameter)
            .or_else(|| {
                self.connectors
                    .iter()
                    .find(|row| row.scalar_name == scalar_name)
                    .map(ScalarSourceClaimRef::Connector)
            })
    }

    /// Derives ordered scalar names from forward rows; no reverse index is stored.
    pub fn scalar_names_for_source<'a>(
        &'a self,
        owner_kind: SourceOwnerKind,
        canonical_class_path: &'a str,
        source_member: &'a str,
    ) -> Option<ScalarNamesForSource<'a>> {
        let exists = match owner_kind {
            SourceOwnerKind::Parameter => self.parameters.iter().any(|row| {
                row.canonical_class_path == canonical_class_path
                    && row.source_member == source_member
            }),
            SourceOwnerKind::Connector => self.connectors.iter().any(|row| {
                row.canonical_class_path == canonical_class_path
                    && row.source_member == source_member
            }),
        };
        exists.then(|| ScalarNamesForSource {
            rows: match owner_kind {
                SourceOwnerKind::Parameter => SourceRows::Parameters(self.parameters.iter()),
                SourceOwnerKind::Connector => SourceRows::Connectors(self.connectors.iter()),
            },
            canonical_class_path,
            source_member,
        })
    }
}

/// One sortable failure from inventory, claim, binding, or projection validation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceClaimDiagnostic {
    pub code: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for SourceClaimDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}: {}",
            self.code, self.owner_kind, self.owner_id, self.location, self.message
        )
    }
}

/// Atomic projection failure with diagnostics sorted by every public field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceClaimError {
    pub diagnostics: Vec<SourceClaimDiagnostic>,
}

impl SourceClaimError {
    fn new(mut diagnostics: Vec<SourceClaimDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for SourceClaimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SourceClaimError {}

fn diagnostic(
    code: &str,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    message: impl Into<String>,
) -> SourceClaimDiagnostic {
    SourceClaimDiagnostic {
        code: code.to_owned(),
        owner_kind: owner_kind.to_owned(),
        owner_id: if owner_id.is_empty() {
            "$".to_owned()
        } else {
            owner_id.to_owned()
        },
        location: location.to_owned(),
        message: message.into(),
    }
}

fn resource_diagnostic(location: &str, message: &str) -> SourceClaimDiagnostic {
    diagnostic("resource_limit", "projection", "$", location, message)
}

fn checked_total_count(lengths: impl IntoIterator<Item = usize>) -> Option<usize> {
    lengths
        .into_iter()
        .try_fold(0_usize, |total, length| total.checked_add(length))
}

fn increment_count(
    count: &mut usize,
    location: &str,
    label: &str,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) {
    match count.checked_add(1) {
        Some(next) => *count = next,
        None => diagnostics.push(resource_diagnostic(
            location,
            &format!("{label} count overflows usize"),
        )),
    }
}

fn reserve_map<K: Eq + Hash, V>(
    map: &mut HashMap<K, V>,
    count: usize,
    location: &str,
    label: &str,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> bool {
    if map.try_reserve(count).is_ok() {
        true
    } else {
        diagnostics.push(resource_diagnostic(
            location,
            &format!("{label} allocation failed"),
        ));
        false
    }
}

fn reserve_set<T: Eq + Hash>(
    set: &mut HashSet<T>,
    count: usize,
    location: &str,
    label: &str,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> bool {
    if set.try_reserve(count).is_ok() {
        true
    } else {
        diagnostics.push(resource_diagnostic(
            location,
            &format!("{label} allocation failed"),
        ));
        false
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn is_revision(value: &str) -> bool {
    is_lower_hex(value, 40)
}

pub(crate) fn is_sha1(value: &str) -> bool {
    value
        .strip_prefix("sha1:")
        .is_some_and(|digest| is_lower_hex(digest, 40))
}

fn is_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| is_lower_hex(digest, 64))
}

pub(crate) fn safe_source_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("path must not be empty");
    }
    if path.starts_with('/') {
        return Err("absolute paths are forbidden");
    }
    if path.contains('\\') {
        return Err("backslashes are forbidden");
    }
    if path
        .chars()
        .any(|character| character < ' ' || character == '\u{7f}')
    {
        return Err("control characters are forbidden");
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err("empty path segments are forbidden");
        }
        if segment == "." {
            return Err("dot path segments are forbidden");
        }
        if segment == ".." {
            return Err("parent traversal is forbidden");
        }
    }
    if !path.starts_with(SOURCE_ROOT_PREFIX) {
        return Err("path must be below the governed G36 source root");
    }
    Ok(())
}

pub(crate) fn is_modelica_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_LENGTH || !value.is_ascii() {
        return false;
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(crate) fn is_class_path(value: &str) -> bool {
    if value.len() > MAX_CLASS_PATH_LENGTH {
        return false;
    }
    let Some(suffix) = value.strip_prefix(CLASS_PATH_PREFIX) else {
        return false;
    };
    !suffix.is_empty() && suffix.split('.').all(is_modelica_identifier)
}

struct PinValidation<'a> {
    revisions: [Option<&'a str>; 2],
    valid: bool,
}

impl<'a> PinValidation<'a> {
    fn revision(&self, role: SourceSnapshotRole) -> Option<&'a str> {
        self.revisions[role.index()]
    }
}

fn validate_pins<'a>(
    pins: &'a [SourcePin],
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> PinValidation<'a> {
    let start = diagnostics.len();
    let mut counts = [0_usize; 2];
    let mut revisions = [None, None];
    let mut revision_counts = HashMap::new();
    let retain_revision_counts = reserve_map(
        &mut revision_counts,
        pins.len(),
        "$.source_pins",
        "source revision index",
        diagnostics,
    );

    for pin in pins {
        let role = pin.role;
        let role_name = role.as_str();
        if !is_revision(&pin.revision) {
            diagnostics.push(diagnostic(
                "invalid_source_revision",
                "pins",
                role_name,
                "$.source_pins.revision",
                "revision must be 40 lowercase hexadecimal characters",
            ));
            continue;
        }
        let index = role.index();
        match counts[index].checked_add(1) {
            Some(count) => counts[index] = count,
            None => diagnostics.push(resource_diagnostic(
                "$.source_pins",
                "source pin count overflows usize",
            )),
        }
        revisions[index].get_or_insert(pin.revision.as_str());
        if retain_revision_counts {
            let count = revision_counts
                .entry(pin.revision.as_str())
                .or_insert(0_usize);
            match count.checked_add(1) {
                Some(next) => *count = next,
                None => diagnostics.push(resource_diagnostic(
                    "$.source_pins",
                    "source revision count overflows usize",
                )),
            }
        }
    }

    if pins.len() != 2 {
        diagnostics.push(diagnostic(
            "invalid_source_pins",
            "pins",
            "$",
            "$.source_pins",
            "source_pins must contain exactly release and development",
        ));
    }
    for role in [SourceSnapshotRole::Release, SourceSnapshotRole::Development] {
        match counts[role.index()] {
            0 => diagnostics.push(diagnostic(
                "missing_source_pin",
                "pins",
                role.as_str(),
                "$.source_pins",
                format!("source_pins is missing `{}`", role.as_str()),
            )),
            1 => {}
            count => diagnostics.push(diagnostic(
                "duplicate_source_pin",
                "pins",
                role.as_str(),
                "$.source_pins",
                format!("source_pins contains {count} `{}` pins", role.as_str()),
            )),
        }
    }
    if retain_revision_counts {
        for (revision, count) in revision_counts {
            if count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_source_revision",
                    "pins",
                    "$",
                    "$.source_pins",
                    format!("source revision `{revision}` is used more than once"),
                ));
            }
        }
    }

    PinValidation {
        revisions,
        valid: diagnostics.len() == start,
    }
}

struct InventoryIndex<'a> {
    files: [HashMap<&'a str, &'a str>; 2],
    valid: bool,
}

impl<'a> InventoryIndex<'a> {
    fn blob(&self, role: SourceSnapshotRole, path: &str) -> Option<&'a str> {
        self.files[role.index()].get(path).copied()
    }
}

struct DerivedCounts {
    file_count: BigInt,
    total_bytes: BigInt,
    modelica_file_count: BigInt,
    package_order_count: BigInt,
}

fn validate_inventory_files<'a>(
    files: &'a [SourceInventoryFile],
    role: SourceSnapshotRole,
    location: &str,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> (HashMap<&'a str, &'a str>, DerivedCounts, bool) {
    let start = diagnostics.len();
    let role_name = role.as_str();
    let mut path_counts = HashMap::new();
    let retain_path_counts = reserve_map(
        &mut path_counts,
        files.len(),
        location,
        "inventory path index",
        diagnostics,
    );
    let mut total_bytes = BigInt::from(0_u8);
    let mut modelica_count = 0_usize;
    let mut package_order_count = 0_usize;
    let mut previous_path: Option<&str> = None;
    let mut order_invalid = false;

    for (index, file) in files.iter().enumerate() {
        let row_location = format!("{location}[{index}]");
        let safe_path = match safe_source_path(&file.path) {
            Ok(()) => true,
            Err(problem) => {
                diagnostics.push(diagnostic(
                    "unsafe_inventory_path",
                    "inventory",
                    role_name,
                    &format!("{row_location}.path"),
                    problem,
                ));
                false
            }
        };
        if file.mode != "100644" {
            diagnostics.push(diagnostic(
                "invalid_inventory_file",
                "inventory",
                role_name,
                &format!("{row_location}.mode"),
                "mode must be `100644`",
            ));
        }
        let valid_size = file.bytes >= BigInt::from(0_u8);
        if !valid_size {
            diagnostics.push(diagnostic(
                "invalid_inventory_file",
                "inventory",
                role_name,
                &format!("{row_location}.bytes"),
                "bytes must be nonnegative",
            ));
        } else {
            total_bytes += &file.bytes;
        }
        if !is_sha1(&file.git_blob_sha1) {
            diagnostics.push(diagnostic(
                "invalid_inventory_blob",
                "inventory",
                role_name,
                &format!("{row_location}.git_blob_sha1"),
                "git_blob_sha1 must match sha1:<40 lowercase hex>",
            ));
        }
        if !is_sha256(&file.sha256) {
            diagnostics.push(diagnostic(
                "invalid_inventory_file",
                "inventory",
                role_name,
                &format!("{row_location}.sha256"),
                "sha256 must match sha256:<64 lowercase hex>",
            ));
        }
        if safe_path {
            if let Some(previous) = previous_path
                && file.path.as_str() < previous
            {
                order_invalid = true;
            }
            previous_path = Some(&file.path);
            if retain_path_counts {
                let count = path_counts.entry(file.path.as_str()).or_insert(0_usize);
                match count.checked_add(1) {
                    Some(next) => *count = next,
                    None => diagnostics.push(resource_diagnostic(
                        location,
                        "inventory path count overflows usize",
                    )),
                }
            }
            if file.path.ends_with(".mo") {
                modelica_count = match modelica_count.checked_add(1) {
                    Some(count) => count,
                    None => {
                        diagnostics.push(resource_diagnostic(
                            location,
                            "Modelica file count overflows usize",
                        ));
                        modelica_count
                    }
                };
            }
            if file.path.ends_with("/package.order") {
                package_order_count = match package_order_count.checked_add(1) {
                    Some(count) => count,
                    None => {
                        diagnostics.push(resource_diagnostic(
                            location,
                            "package.order count overflows usize",
                        ));
                        package_order_count
                    }
                };
            }
        }
    }

    if retain_path_counts {
        for (path, count) in &path_counts {
            if *count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_inventory_path",
                    "inventory",
                    role_name,
                    location,
                    format!("path `{path}` occurs {count} times"),
                ));
            }
        }
    }
    if order_invalid {
        diagnostics.push(diagnostic(
            "inventory_file_order",
            "inventory",
            role_name,
            location,
            "file paths must be lexicographically ordered",
        ));
    }

    let rows_valid = diagnostics.len() == start;
    let counts = DerivedCounts {
        file_count: BigInt::from(files.len()),
        total_bytes,
        modelica_file_count: BigInt::from(modelica_count),
        package_order_count: BigInt::from(package_order_count),
    };
    let mut index = HashMap::new();
    if rows_valid
        && reserve_map(
            &mut index,
            files.len(),
            location,
            "inventory file index",
            diagnostics,
        )
    {
        for file in files {
            index.insert(file.path.as_str(), file.git_blob_sha1.as_str());
        }
    }
    let valid = rows_valid && diagnostics.len() == start;
    (index, counts, valid)
}

fn validate_inventory_snapshot<'a>(
    snapshot: &'a SourceInventorySnapshot,
    index: usize,
    expected_role: SourceSnapshotRole,
    pins: &PinValidation<'_>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> (HashMap<&'a str, &'a str>, bool) {
    let start = diagnostics.len();
    let role_name = expected_role.as_str();
    let location = format!("$.source_inventory.snapshots[{index}]");
    if snapshot.role != expected_role {
        diagnostics.push(diagnostic(
            "inventory_snapshot_role",
            "inventory",
            role_name,
            &format!("{location}.role"),
            format!("snapshot role must be `{role_name}`"),
        ));
    }
    if !is_revision(&snapshot.revision) {
        diagnostics.push(diagnostic(
            "invalid_inventory_revision",
            "inventory",
            role_name,
            &format!("{location}.revision"),
            "revision must be 40 lowercase hexadecimal characters",
        ));
    } else if let Some(pin) = pins.revision(expected_role)
        && snapshot.revision != pin
    {
        diagnostics.push(diagnostic(
            "inventory_snapshot_revision",
            "inventory",
            role_name,
            &format!("{location}.revision"),
            "snapshot revision must equal its supplied source pin",
        ));
    }
    if !is_sha1(&snapshot.root_tree_sha1) {
        diagnostics.push(diagnostic(
            "invalid_inventory_snapshot",
            "inventory",
            role_name,
            &format!("{location}.root_tree_sha1"),
            "root_tree_sha1 must match sha1:<40 lowercase hex>",
        ));
    }

    let declared_counts = [
        ("file_count", &snapshot.file_count),
        ("total_bytes", &snapshot.total_bytes),
        ("modelica_file_count", &snapshot.modelica_file_count),
        ("package_order_count", &snapshot.package_order_count),
    ];
    let mut counts_valid = true;
    for (name, value) in declared_counts {
        if value < &BigInt::from(0_u8) {
            counts_valid = false;
            diagnostics.push(diagnostic(
                "invalid_inventory_count",
                "inventory",
                role_name,
                &format!("{location}.{name}"),
                format!("{name} must be nonnegative"),
            ));
        }
    }

    let files_location = format!("{location}.files");
    let (file_index, expected_counts, files_valid) =
        validate_inventory_files(&snapshot.files, expected_role, &files_location, diagnostics);
    if counts_valid && files_valid {
        for (name, declared, derived) in [
            (
                "file_count",
                &snapshot.file_count,
                &expected_counts.file_count,
            ),
            (
                "total_bytes",
                &snapshot.total_bytes,
                &expected_counts.total_bytes,
            ),
            (
                "modelica_file_count",
                &snapshot.modelica_file_count,
                &expected_counts.modelica_file_count,
            ),
            (
                "package_order_count",
                &snapshot.package_order_count,
                &expected_counts.package_order_count,
            ),
        ] {
            if declared != derived {
                diagnostics.push(diagnostic(
                    "inventory_count_mismatch",
                    "inventory",
                    role_name,
                    &format!("{location}.{name}"),
                    format!("{name} must equal the value derived from files"),
                ));
            }
        }
    }

    let valid = diagnostics.len() == start;
    (if valid { file_index } else { HashMap::new() }, valid)
}

fn validate_inventory<'a>(
    inventory: &'a SourceInventory,
    pins: &PinValidation<'_>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> InventoryIndex<'a> {
    let start = diagnostics.len();
    for (name, actual, expected) in [
        ("schema", inventory.schema.as_str(), INVENTORY_SCHEMA),
        (
            "repository",
            inventory.repository.as_str(),
            UPSTREAM_REPOSITORY,
        ),
        ("source_root", inventory.source_root.as_str(), SOURCE_ROOT),
        (
            "inventory_scope",
            inventory.inventory_scope.as_str(),
            INVENTORY_SCOPE,
        ),
        (
            "dependency_closure",
            inventory.dependency_closure.as_str(),
            DEPENDENCY_CLOSURE,
        ),
    ] {
        if actual != expected {
            diagnostics.push(diagnostic(
                "inventory_constant",
                "inventory",
                "$",
                &format!("$.source_inventory.{name}"),
                format!("{name} must equal the governed constant"),
            ));
        }
    }

    if inventory.license.upstream_path != LICENSE_UPSTREAM_PATH {
        diagnostics.push(diagnostic(
            "invalid_inventory_license",
            "inventory",
            "$",
            "$.source_inventory.license.upstream_path",
            "upstream_path must match the governed inventory contract",
        ));
    }
    if inventory.license.retained_path != LICENSE_RETAINED_PATH {
        diagnostics.push(diagnostic(
            "invalid_inventory_license",
            "inventory",
            "$",
            "$.source_inventory.license.retained_path",
            "retained_path must match the governed inventory contract",
        ));
    }
    if !is_sha1(&inventory.license.git_blob_sha1) {
        diagnostics.push(diagnostic(
            "invalid_inventory_license",
            "inventory",
            "$",
            "$.source_inventory.license.git_blob_sha1",
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        ));
    }
    if !is_sha256(&inventory.license.sha256) {
        diagnostics.push(diagnostic(
            "invalid_inventory_license",
            "inventory",
            "$",
            "$.source_inventory.license.sha256",
            "sha256 must match sha256:<64 lowercase hex>",
        ));
    }

    let mut files = [HashMap::new(), HashMap::new()];
    if inventory.snapshots.len() != 2 {
        diagnostics.push(diagnostic(
            "invalid_inventory_snapshots",
            "inventory",
            "$",
            "$.source_inventory.snapshots",
            "snapshots must contain release then development",
        ));
    } else {
        for (index, role) in [SourceSnapshotRole::Release, SourceSnapshotRole::Development]
            .into_iter()
            .enumerate()
        {
            let (snapshot_files, valid) = validate_inventory_snapshot(
                &inventory.snapshots[index],
                index,
                role,
                pins,
                diagnostics,
            );
            if valid {
                files[role.index()] = snapshot_files;
            }
        }
    }

    InventoryIndex {
        files,
        valid: diagnostics.len() == start,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct OwnerKey<'a> {
    kind: SourceOwnerKind,
    id: &'a str,
}

fn validate_coordinates(
    coordinates: &[ScalarCoordinate],
    owner_kind: SourceOwnerKind,
    owner_id: &str,
    location: &str,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> bool {
    let start = diagnostics.len();
    for (index, coordinate) in coordinates.iter().enumerate() {
        let coordinate_location = format!("{location}.coordinates[{index}]");
        if coordinate.dimension_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_dimension_id",
                owner_kind.as_str(),
                owner_id,
                &format!("{coordinate_location}.dimension_id"),
                "dimension_id must not be empty",
            ));
        }
        if coordinate.member_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_member_id",
                owner_kind.as_str(),
                owner_id,
                &format!("{coordinate_location}.member_id"),
                "member_id must not be empty",
            ));
        }
    }
    diagnostics.len() == start
}

struct NamedValidation<'a> {
    expected_owners: HashSet<OwnerKey<'a>>,
    valid: bool,
}

fn validate_named_projection<'a>(
    projection: &'a NamedScalarProjection,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> NamedValidation<'a> {
    let start = diagnostics.len();
    if projection.canonical_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_named_metadata",
            "projection",
            "$",
            "$.named_projection.canonical_id",
            "canonical_id must not be empty",
        ));
    }
    if projection.revision <= BigInt::from(0_u8) {
        diagnostics.push(diagnostic(
            "invalid_named_metadata",
            "projection",
            "$",
            "$.named_projection.revision",
            "revision must be positive",
        ));
    }

    let row_count = checked_total_count([projection.parameters.len(), projection.connectors.len()]);
    if row_count.is_none() {
        diagnostics.push(resource_diagnostic(
            "$.named_projection",
            "named scalar row count overflows usize",
        ));
    }
    let capacity = row_count.unwrap_or(0);
    let mut scalar_counts = HashMap::new();
    let retain_scalars = reserve_map(
        &mut scalar_counts,
        capacity,
        "$.named_projection",
        "scalar name index",
        diagnostics,
    );
    let mut expected_owners = HashSet::new();
    let retain_owners = reserve_set(
        &mut expected_owners,
        capacity,
        "$.named_projection",
        "named owner index",
        diagnostics,
    );

    for (index, row) in projection.parameters.iter().enumerate() {
        let location = format!("$.named_projection.parameters[{index}]");
        validate_named_parameter(
            row,
            &location,
            retain_scalars,
            retain_owners,
            &mut scalar_counts,
            &mut expected_owners,
            diagnostics,
        );
    }
    for (index, row) in projection.connectors.iter().enumerate() {
        let location = format!("$.named_projection.connectors[{index}]");
        validate_named_connector(
            row,
            &location,
            retain_scalars,
            retain_owners,
            &mut scalar_counts,
            &mut expected_owners,
            diagnostics,
        );
    }
    if retain_scalars {
        for (scalar_name, count) in scalar_counts {
            if count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_scalar_name",
                    "projection",
                    "$",
                    "$.named_projection",
                    format!("scalar name `{scalar_name}` occurs {count} times"),
                ));
            }
        }
    }

    NamedValidation {
        expected_owners,
        valid: diagnostics.len() == start,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_named_parameter<'a>(
    row: &'a NamedScalarParameterRow,
    location: &str,
    retain_scalars: bool,
    retain_owners: bool,
    scalar_counts: &mut HashMap<&'a str, usize>,
    expected_owners: &mut HashSet<OwnerKey<'a>>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) {
    validate_named_row(
        &row.scalar_name,
        &row.parameter_id,
        &row.coordinates,
        SourceOwnerKind::Parameter,
        "p_",
        "parameter_id",
        location,
        retain_scalars,
        retain_owners,
        scalar_counts,
        expected_owners,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_named_connector<'a>(
    row: &'a NamedScalarConnectorRow,
    location: &str,
    retain_scalars: bool,
    retain_owners: bool,
    scalar_counts: &mut HashMap<&'a str, usize>,
    expected_owners: &mut HashSet<OwnerKey<'a>>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) {
    validate_named_row(
        &row.scalar_name,
        &row.connector_id,
        &row.coordinates,
        SourceOwnerKind::Connector,
        "c_",
        "connector_id",
        location,
        retain_scalars,
        retain_owners,
        scalar_counts,
        expected_owners,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_named_row<'a>(
    scalar_name: &'a str,
    owner_id: &'a str,
    coordinates: &[ScalarCoordinate],
    owner_kind: SourceOwnerKind,
    prefix: &str,
    owner_field: &str,
    location: &str,
    retain_scalars: bool,
    retain_owners: bool,
    scalar_counts: &mut HashMap<&'a str, usize>,
    expected_owners: &mut HashSet<OwnerKey<'a>>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) {
    let start = diagnostics.len();
    if owner_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_owner_id",
            owner_kind.as_str(),
            owner_id,
            &format!("{location}.{owner_field}"),
            format!("{owner_field} must not be empty"),
        ));
    }
    if scalar_name.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_scalar_name",
            owner_kind.as_str(),
            owner_id,
            &format!("{location}.scalar_name"),
            "scalar_name must not be empty",
        ));
    } else if !scalar_name.starts_with(prefix) {
        diagnostics.push(diagnostic(
            "scalar_name_namespace",
            owner_kind.as_str(),
            owner_id,
            &format!("{location}.scalar_name"),
            format!(
                "{} scalar names must start with `{prefix}`",
                owner_kind.as_str()
            ),
        ));
    }
    validate_coordinates(coordinates, owner_kind, owner_id, location, diagnostics);
    if diagnostics.len() == start {
        if retain_scalars {
            let count = scalar_counts.entry(scalar_name).or_insert(0_usize);
            match count.checked_add(1) {
                Some(next) => *count = next,
                None => diagnostics.push(resource_diagnostic(
                    "$.named_projection",
                    "scalar name count overflows usize",
                )),
            }
        }
        if retain_owners {
            expected_owners.insert(OwnerKey {
                kind: owner_kind,
                id: owner_id,
            });
        }
    }
}

#[derive(Clone, Copy)]
struct ClaimGroup {
    count: usize,
    first_index: usize,
}

struct LocatorGroup<'a> {
    count: usize,
    min_owner: &'a str,
}

struct ClassValidation<'a> {
    groups: HashMap<&'a str, ClaimGroup>,
    valid_claims: HashMap<&'a str, &'a SourceClassClaim>,
}

fn structurally_valid_claim(claim: &SourceClassClaim) -> bool {
    is_class_path(&claim.canonical_class_path)
        && is_revision(&claim.revision)
        && safe_source_path(&claim.file.path).is_ok()
        && claim.file.path.ends_with(".mo")
        && is_sha1(&claim.file.git_blob_sha1)
}

fn validate_class_claims<'a>(
    claims: &'a [SourceClassClaim],
    pins: &PinValidation<'_>,
    inventory: &InventoryIndex<'_>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> ClassValidation<'a> {
    let mut groups: HashMap<&'a str, ClaimGroup> = HashMap::new();
    let retain_groups = reserve_map(
        &mut groups,
        claims.len(),
        "$.class_claims",
        "class claim index",
        diagnostics,
    );
    let mut locator_groups: HashMap<(&'a str, &'a str), LocatorGroup<'a>> = HashMap::new();
    let retain_locators = reserve_map(
        &mut locator_groups,
        claims.len(),
        "$.class_claims",
        "file locator index",
        diagnostics,
    );

    for (index, claim) in claims.iter().enumerate() {
        let owner = claim.canonical_class_path.as_str();
        let valid_class = is_class_path(owner);
        if !valid_class {
            diagnostics.push(diagnostic(
                "invalid_class_path",
                "class",
                owner,
                "$.class_claims.canonical_class_path",
                "canonical_class_path must be a bounded class below the G36 package",
            ));
        } else if retain_groups {
            match groups.entry(owner) {
                Entry::Occupied(mut entry) => increment_count(
                    &mut entry.get_mut().count,
                    "$.class_claims",
                    "class claim",
                    diagnostics,
                ),
                Entry::Vacant(entry) => {
                    entry.insert(ClaimGroup {
                        count: 1,
                        first_index: index,
                    });
                }
            }
        }
        let valid_revision = is_revision(&claim.revision);
        if !valid_revision {
            diagnostics.push(diagnostic(
                "invalid_class_revision",
                "class",
                owner,
                "$.class_claims.revision",
                "revision must be 40 lowercase hexadecimal characters",
            ));
        } else if pins.valid
            && pins
                .revision(claim.snapshot)
                .is_some_and(|revision| revision != claim.revision)
        {
            diagnostics.push(diagnostic(
                "class_revision_mismatch",
                "class",
                owner,
                "$.class_claims.revision",
                "class claim revision must equal its supplied source pin",
            ));
        }

        let safe_path = match safe_source_path(&claim.file.path) {
            Ok(()) => true,
            Err(problem) => {
                diagnostics.push(diagnostic(
                    "unsafe_class_path",
                    "class",
                    owner,
                    "$.class_claims.file.path",
                    problem,
                ));
                false
            }
        };
        let modelica_path = safe_path && claim.file.path.ends_with(".mo");
        if safe_path && !modelica_path {
            diagnostics.push(diagnostic(
                "non_modelica_locator",
                "class",
                owner,
                "$.class_claims.file.path",
                "the primary class locator must end in `.mo`",
            ));
        }
        let valid_blob = is_sha1(&claim.file.git_blob_sha1);
        if !valid_blob {
            diagnostics.push(diagnostic(
                "invalid_file_blob",
                "class",
                owner,
                "$.class_claims.file.git_blob_sha1",
                "git_blob_sha1 must match sha1:<40 lowercase hex>",
            ));
        }

        if safe_path && valid_blob && retain_locators {
            let diagnostic_owner = if owner.is_empty() { "$" } else { owner };
            match locator_groups
                .entry((claim.file.path.as_str(), claim.file.git_blob_sha1.as_str()))
            {
                Entry::Occupied(mut entry) => {
                    let group: &mut LocatorGroup<'a> = entry.get_mut();
                    increment_count(
                        &mut group.count,
                        "$.class_claims",
                        "file locator",
                        diagnostics,
                    );
                    if diagnostic_owner < group.min_owner {
                        group.min_owner = diagnostic_owner;
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(LocatorGroup {
                        count: 1,
                        min_owner: diagnostic_owner,
                    });
                }
            }
        }

        if valid_class && valid_revision && modelica_path && valid_blob && inventory.valid {
            match inventory.blob(claim.snapshot, &claim.file.path) {
                None => diagnostics.push(diagnostic(
                    "absent_file_locator",
                    "class",
                    owner,
                    "$.class_claims.file.path",
                    "file path is absent from the claimed snapshot",
                )),
                Some(blob) if blob != claim.file.git_blob_sha1 => diagnostics.push(diagnostic(
                    "file_blob_mismatch",
                    "class",
                    owner,
                    "$.class_claims.file.git_blob_sha1",
                    "file blob does not match the claimed snapshot path",
                )),
                Some(_) => {}
            }
        }
    }

    if retain_groups {
        for (class_path, group) in &groups {
            if group.count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_class_claim",
                    "class",
                    class_path,
                    "$.class_claims",
                    format!("canonical class has {} claims", group.count),
                ));
            }
        }
    }
    if retain_locators {
        for ((path, blob), group) in locator_groups {
            if group.count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_file_locator",
                    "class",
                    group.min_owner,
                    "$.class_claims",
                    format!(
                        "file locator (`{path}`, `{blob}`) occurs {} times",
                        group.count
                    ),
                ));
            }
        }
    }

    let mut valid_claims = HashMap::new();
    if reserve_map(
        &mut valid_claims,
        groups.len(),
        "$.class_claims",
        "validated class claim index",
        diagnostics,
    ) {
        for (class_path, group) in &groups {
            if group.count == 1
                && let Some(claim) = claims.get(group.first_index)
                && structurally_valid_claim(claim)
            {
                valid_claims.insert(*class_path, claim);
            }
        }
    }

    ClassValidation {
        groups,
        valid_claims,
    }
}

#[derive(Clone, Copy)]
struct BindingGroup {
    count: usize,
    first_index: usize,
}

struct SourceGroup<'a> {
    first_owner: &'a str,
    min_owner: &'a str,
    multiple_owners: bool,
}

struct BindingValidation<'a> {
    valid_bindings: HashMap<OwnerKey<'a>, &'a SourceMemberBinding>,
}

fn structurally_valid_binding(binding: &SourceMemberBinding) -> bool {
    !binding.owner_id.is_empty()
        && is_class_path(&binding.canonical_class_path)
        && is_modelica_identifier(&binding.source_member)
}

fn validate_member_bindings<'a>(
    bindings: &'a [SourceMemberBinding],
    named: &NamedValidation<'_>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> BindingValidation<'a> {
    let mut groups: HashMap<OwnerKey<'a>, BindingGroup> = HashMap::new();
    let retain_groups = reserve_map(
        &mut groups,
        bindings.len(),
        "$.member_bindings",
        "member binding index",
        diagnostics,
    );
    let mut source_groups: HashMap<(SourceOwnerKind, &'a str, &'a str), SourceGroup<'a>> =
        HashMap::new();
    let retain_sources = reserve_map(
        &mut source_groups,
        bindings.len(),
        "$.member_bindings",
        "source key index",
        diagnostics,
    );

    for (index, binding) in bindings.iter().enumerate() {
        let owner_kind = binding.owner_kind;
        let owner_id = binding.owner_id.as_str();
        if owner_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_owner_id",
                owner_kind.as_str(),
                owner_id,
                "$.member_bindings.owner_id",
                "owner_id must not be empty",
            ));
        }
        if !is_class_path(&binding.canonical_class_path) {
            diagnostics.push(diagnostic(
                "invalid_binding_class_path",
                "binding",
                owner_id,
                "$.member_bindings.canonical_class_path",
                "canonical_class_path must be a bounded class below the G36 package",
            ));
        }
        if !is_modelica_identifier(&binding.source_member) {
            diagnostics.push(diagnostic(
                "invalid_source_member",
                "binding",
                owner_id,
                "$.member_bindings.source_member",
                "source_member must be a bounded ASCII Modelica identifier",
            ));
        }

        if structurally_valid_binding(binding) {
            let owner = OwnerKey {
                kind: owner_kind,
                id: owner_id,
            };
            if retain_groups {
                match groups.entry(owner) {
                    Entry::Occupied(mut entry) => increment_count(
                        &mut entry.get_mut().count,
                        "$.member_bindings",
                        "member binding",
                        diagnostics,
                    ),
                    Entry::Vacant(entry) => {
                        entry.insert(BindingGroup {
                            count: 1,
                            first_index: index,
                        });
                    }
                }
            }
            if retain_sources {
                source_groups
                    .entry((
                        owner_kind,
                        binding.canonical_class_path.as_str(),
                        binding.source_member.as_str(),
                    ))
                    .and_modify(|group: &mut SourceGroup<'a>| {
                        if owner_id != group.first_owner {
                            group.multiple_owners = true;
                        }
                        if owner_id < group.min_owner {
                            group.min_owner = owner_id;
                        }
                    })
                    .or_insert(SourceGroup {
                        first_owner: owner_id,
                        min_owner: owner_id,
                        multiple_owners: false,
                    });
            }
        }
    }

    if retain_groups {
        for (owner, group) in &groups {
            if group.count > 1 {
                diagnostics.push(diagnostic(
                    "duplicate_member_binding",
                    owner.kind.as_str(),
                    owner.id,
                    "$.member_bindings",
                    format!("owner has {} member bindings", group.count),
                ));
            }
        }
    }
    if retain_sources {
        for ((owner_kind, _, _), group) in source_groups {
            if group.multiple_owners {
                diagnostics.push(diagnostic(
                    "duplicate_source_key",
                    owner_kind.as_str(),
                    group.min_owner,
                    "$.member_bindings",
                    "distinct owners claim the same class and source member",
                ));
            }
        }
    }

    if named.valid && retain_groups {
        for owner in &named.expected_owners {
            if !groups.contains_key(owner) {
                diagnostics.push(diagnostic(
                    "missing_member_binding",
                    owner.kind.as_str(),
                    owner.id,
                    "$.member_bindings",
                    "named scalar owner has no member binding",
                ));
            }
        }
        for owner in groups.keys() {
            if !named.expected_owners.contains(owner) {
                if named.expected_owners.contains(&OwnerKey {
                    kind: owner.kind.opposite(),
                    id: owner.id,
                }) {
                    diagnostics.push(diagnostic(
                        "cross_namespace_binding",
                        owner.kind.as_str(),
                        owner.id,
                        "$.member_bindings",
                        format!(
                            "owner exists only in the {} namespace",
                            owner.kind.opposite().as_str()
                        ),
                    ));
                }
                diagnostics.push(diagnostic(
                    "extra_member_binding",
                    owner.kind.as_str(),
                    owner.id,
                    "$.member_bindings",
                    "member binding has no named scalar owner",
                ));
            }
        }
    }

    let mut valid_bindings = HashMap::new();
    if reserve_map(
        &mut valid_bindings,
        groups.len(),
        "$.member_bindings",
        "validated member binding index",
        diagnostics,
    ) {
        for (owner, group) in groups {
            if group.count == 1
                && let Some(binding) = bindings.get(group.first_index)
            {
                valid_bindings.insert(owner, binding);
            }
        }
    }
    BindingValidation { valid_bindings }
}

fn validate_claim_usage(
    claims: &[SourceClassClaim],
    class_validation: &ClassValidation<'_>,
    binding_validation: &BindingValidation<'_>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) {
    let mut referenced = HashSet::new();
    if !reserve_set(
        &mut referenced,
        binding_validation.valid_bindings.len(),
        "$.class_claims",
        "referenced class index",
        diagnostics,
    ) {
        return;
    }
    for binding in binding_validation.valid_bindings.values() {
        referenced.insert(binding.canonical_class_path.as_str());
    }
    for class_path in &referenced {
        match class_validation
            .groups
            .get(class_path)
            .map(|group| group.count)
        {
            None | Some(0) => diagnostics.push(diagnostic(
                "missing_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "member binding references no supplied class claim",
            )),
            Some(1) => {}
            Some(_) => diagnostics.push(diagnostic(
                "ambiguous_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "member binding references more than one supplied class claim",
            )),
        }
    }
    for class_path in class_validation.groups.keys() {
        if !referenced.contains(class_path) {
            diagnostics.push(diagnostic(
                "unused_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "class claim is not referenced by a member binding",
            ));
            for claim in claims
                .iter()
                .filter(|claim| claim.canonical_class_path == *class_path)
            {
                if safe_source_path(&claim.file.path).is_ok() && is_sha1(&claim.file.git_blob_sha1)
                {
                    diagnostics.push(diagnostic(
                        "extra_file_locator",
                        "class",
                        class_path,
                        "$.class_claims",
                        "file locator belongs to an unused class claim",
                    ));
                }
            }
        }
    }
}

struct PreparedParameter<'a> {
    row: &'a NamedScalarParameterRow,
    binding: &'a SourceMemberBinding,
    claim: &'a SourceClassClaim,
}

struct PreparedConnector<'a> {
    row: &'a NamedScalarConnectorRow,
    binding: &'a SourceMemberBinding,
    claim: &'a SourceClassClaim,
}

fn prepare_rows<'a>(
    projection: &'a NamedScalarProjection,
    classes: &'a ClassValidation<'a>,
    bindings: &'a BindingValidation<'a>,
    diagnostics: &mut Vec<SourceClaimDiagnostic>,
) -> (Vec<PreparedParameter<'a>>, Vec<PreparedConnector<'a>>) {
    let mut parameters = Vec::new();
    if parameters
        .try_reserve_exact(projection.parameters.len())
        .is_err()
    {
        diagnostics.push(resource_diagnostic(
            "$.named_projection.parameters",
            "prepared parameter row allocation failed",
        ));
    }
    let mut connectors = Vec::new();
    if connectors
        .try_reserve_exact(projection.connectors.len())
        .is_err()
    {
        diagnostics.push(resource_diagnostic(
            "$.named_projection.connectors",
            "prepared connector row allocation failed",
        ));
    }
    if !diagnostics.is_empty() {
        return (parameters, connectors);
    }

    for row in &projection.parameters {
        let owner = OwnerKey {
            kind: SourceOwnerKind::Parameter,
            id: &row.parameter_id,
        };
        let Some(binding) = bindings.valid_bindings.get(&owner).copied() else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "parameter",
                &row.parameter_id,
                "$.member_bindings",
                "validated parameter binding is unavailable",
            ));
            continue;
        };
        let Some(claim) = classes
            .valid_claims
            .get(binding.canonical_class_path.as_str())
            .copied()
        else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "class",
                &binding.canonical_class_path,
                "$.class_claims",
                "validated class claim is unavailable",
            ));
            continue;
        };
        parameters.push(PreparedParameter {
            row,
            binding,
            claim,
        });
    }
    for row in &projection.connectors {
        let owner = OwnerKey {
            kind: SourceOwnerKind::Connector,
            id: &row.connector_id,
        };
        let Some(binding) = bindings.valid_bindings.get(&owner).copied() else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "connector",
                &row.connector_id,
                "$.member_bindings",
                "validated connector binding is unavailable",
            ));
            continue;
        };
        let Some(claim) = classes
            .valid_claims
            .get(binding.canonical_class_path.as_str())
            .copied()
        else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "class",
                &binding.canonical_class_path,
                "$.class_claims",
                "validated class claim is unavailable",
            ));
            continue;
        };
        connectors.push(PreparedConnector {
            row,
            binding,
            claim,
        });
    }
    (parameters, connectors)
}

fn clone_text(value: &str) -> Result<String, ()> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| ())?;
    output.push_str(value);
    Ok(output)
}

fn clone_coordinates(coordinates: &[ScalarCoordinate]) -> Result<Vec<ScalarCoordinate>, ()> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(coordinates.len())
        .map_err(|_| ())?;
    for coordinate in coordinates {
        output.push(ScalarCoordinate {
            dimension_id: clone_text(&coordinate.dimension_id)?,
            member_id: clone_text(&coordinate.member_id)?,
            ordinal: coordinate.ordinal,
        });
    }
    Ok(output)
}

fn clone_locator(locator: &SourceFileLocator) -> Result<SourceFileLocator, ()> {
    Ok(SourceFileLocator {
        path: clone_text(&locator.path)?,
        git_blob_sha1: clone_text(&locator.git_blob_sha1)?,
    })
}

fn output_resource_error(location: &str, label: &str) -> SourceClaimError {
    SourceClaimError::new(vec![resource_diagnostic(
        location,
        &format!("{label} allocation failed"),
    )])
}

/// Validates the complete typed join and projects detached scalar claim rows.
///
/// No filesystem, Git, source parser, or declaration evidence participates.
/// Class and member identities are copied claims after exact snapshot path/blob
/// membership is established.
pub fn project_scalar_source_claims(
    named_projection: &NamedScalarProjection,
    source_inventory: &SourceInventory,
    source_pins: &[SourcePin],
    class_claims: &[SourceClassClaim],
    member_bindings: &[SourceMemberBinding],
) -> Result<ScalarSourceClaimProjection, SourceClaimError> {
    let mut diagnostics = Vec::new();
    let input_count = checked_total_count([
        source_pins.len(),
        source_inventory.snapshots.len(),
        class_claims.len(),
        member_bindings.len(),
        named_projection.parameters.len(),
        named_projection.connectors.len(),
    ]);
    match input_count {
        Some(count) => {
            if diagnostics.try_reserve(count).is_err() {
                return Err(SourceClaimError::new(vec![resource_diagnostic(
                    "$",
                    "diagnostic allocation failed",
                )]));
            }
        }
        None => {
            return Err(SourceClaimError::new(vec![resource_diagnostic(
                "$",
                "input count overflows usize",
            )]));
        }
    }

    let pins = validate_pins(source_pins, &mut diagnostics);
    let inventory = validate_inventory(source_inventory, &pins, &mut diagnostics);
    let named = validate_named_projection(named_projection, &mut diagnostics);
    let classes = validate_class_claims(class_claims, &pins, &inventory, &mut diagnostics);
    let bindings = validate_member_bindings(member_bindings, &named, &mut diagnostics);
    validate_claim_usage(class_claims, &classes, &bindings, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(SourceClaimError::new(diagnostics));
    }

    let (prepared_parameters, prepared_connectors) =
        prepare_rows(named_projection, &classes, &bindings, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(SourceClaimError::new(diagnostics));
    }

    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(prepared_parameters.len())
        .map_err(|_| output_resource_error("$.parameters", "parameter output row vector"))?;
    let mut connectors = Vec::new();
    connectors
        .try_reserve_exact(prepared_connectors.len())
        .map_err(|_| output_resource_error("$.connectors", "connector output row vector"))?;

    for prepared in prepared_parameters {
        let row = prepared.row;
        let binding = prepared.binding;
        let claim = prepared.claim;
        parameters.push(ScalarParameterSourceClaim {
            scalar_name: clone_text(&row.scalar_name)
                .map_err(|_| output_resource_error("$.parameters", "scalar name"))?,
            parameter_id: clone_text(&row.parameter_id)
                .map_err(|_| output_resource_error("$.parameters", "parameter ID"))?,
            coordinates: clone_coordinates(&row.coordinates)
                .map_err(|_| output_resource_error("$.parameters", "coordinate"))?,
            canonical_class_path: clone_text(&binding.canonical_class_path)
                .map_err(|_| output_resource_error("$.parameters", "class path"))?,
            source_member: clone_text(&binding.source_member)
                .map_err(|_| output_resource_error("$.parameters", "source member"))?,
            snapshot: claim.snapshot,
            revision: clone_text(&claim.revision)
                .map_err(|_| output_resource_error("$.parameters", "source revision"))?,
            file: clone_locator(&claim.file)
                .map_err(|_| output_resource_error("$.parameters", "file locator"))?,
        });
    }
    for prepared in prepared_connectors {
        let row = prepared.row;
        let binding = prepared.binding;
        let claim = prepared.claim;
        connectors.push(ScalarConnectorSourceClaim {
            scalar_name: clone_text(&row.scalar_name)
                .map_err(|_| output_resource_error("$.connectors", "scalar name"))?,
            connector_id: clone_text(&row.connector_id)
                .map_err(|_| output_resource_error("$.connectors", "connector ID"))?,
            coordinates: clone_coordinates(&row.coordinates)
                .map_err(|_| output_resource_error("$.connectors", "coordinate"))?,
            canonical_class_path: clone_text(&binding.canonical_class_path)
                .map_err(|_| output_resource_error("$.connectors", "class path"))?,
            source_member: clone_text(&binding.source_member)
                .map_err(|_| output_resource_error("$.connectors", "source member"))?,
            snapshot: claim.snapshot,
            revision: clone_text(&claim.revision)
                .map_err(|_| output_resource_error("$.connectors", "source revision"))?,
            file: clone_locator(&claim.file)
                .map_err(|_| output_resource_error("$.connectors", "file locator"))?,
        });
    }

    Ok(ScalarSourceClaimProjection {
        canonical_id: clone_text(&named_projection.canonical_id)
            .map_err(|_| output_resource_error("$", "canonical ID"))?,
        revision: named_projection.revision.clone(),
        parameters,
        connectors,
    })
}

#[cfg(test)]
mod tests;
