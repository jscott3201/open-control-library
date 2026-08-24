use std::path::{Component as PathComponent, Path, PathBuf};

use rumoca_core::{ClassType, Variability};
use rumoca_ir_ast::{ClassDef, Component, StoredDefinition};

enum DeclarationRequirement {
    Block {
        path: &'static str,
        expected_within: &'static str,
        class_name: &'static str,
        members: &'static [MemberRequirement],
    },
    Enumeration {
        path: &'static str,
        expected_within: &'static str,
        class_name: &'static str,
        literals: &'static [&'static str],
    },
}

enum MemberRequirement {
    PublicParameter {
        name: &'static str,
        declared_type: &'static str,
    },
    PublicComponent {
        name: &'static str,
        declared_type: &'static str,
    },
}

static DECLARATION_REQUIREMENTS: [DeclarationRequirement; 2] = [
    DeclarationRequirement::Block {
        path: "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo",
        expected_within: "Buildings.Controls.OBC.ASHRAE.G36.Generic",
        class_name: "TrimAndRespond",
        members: &[
            MemberRequirement::PublicParameter {
                name: "samplePeriod",
                declared_type: "Real",
            },
            MemberRequirement::PublicComponent {
                name: "numOfReq",
                declared_type: "Buildings.Controls.OBC.CDL.Interfaces.IntegerInput",
            },
        ],
    },
    DeclarationRequirement::Enumeration {
        path: "Buildings/Controls/OBC/ASHRAE/G36/Types/HeatingCoil.mo",
        expected_within: "Buildings.Controls.OBC.ASHRAE.G36.Types",
        class_name: "HeatingCoil",
        literals: &["None", "WaterBased", "Electric"],
    },
];

impl DeclarationRequirement {
    fn path(&self) -> &'static str {
        match self {
            Self::Block { path, .. } | Self::Enumeration { path, .. } => path,
        }
    }

    fn identity(&self) -> (&'static str, &'static str) {
        match self {
            Self::Block {
                expected_within,
                class_name,
                ..
            }
            | Self::Enumeration {
                expected_within,
                class_name,
                ..
            } => (expected_within, class_name),
        }
    }
}

/// Checks the fixed G36 `TrimAndRespond.mo` declaration before `HeatingCoil.mo`.
/// The check covers parsing, the `within` clause, direct class identity and kind,
/// public `parameter Real samplePeriod`, public
/// `Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq`, and the exact
/// `HeatingCoil` literals `None`, `WaterBased`, `Electric` in that order. It does
/// not resolve dependencies or inheritance.
pub fn verify_release_declarations(release_root: &Path) -> Result<(), String> {
    for requirement in &DECLARATION_REQUIREMENTS {
        let source = read_fixed_source(release_root, requirement.path())?;
        verify_declaration(&source, requirement)?;
    }
    Ok(())
}

fn read_fixed_source(release_root: &Path, relative: &str) -> Result<String, String> {
    let path = fixed_source_path(release_root, relative)?;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("{relative}: cannot read source ({:?})", error.kind()))?;
    String::from_utf8(bytes).map_err(|_| format!("{relative}: source is not UTF-8"))
}

fn fixed_source_path(release_root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, PathComponent::Normal(_)))
    {
        return Err(format!("unsafe fixed source path `{relative}`"));
    }
    Ok(release_root.join(relative_path))
}

fn parse_source(relative: &str, source: &str) -> Result<StoredDefinition, String> {
    rumoca_phase_parse::parse_to_ast(source, relative)
        .map_err(|_| format!("{relative}: Modelica parse failed"))
}

pub(crate) fn direct_class<'a>(
    parsed: &'a StoredDefinition,
    relative: &str,
    expected_within: &str,
    expected_class: &str,
) -> Result<(&'a ClassDef, String), String> {
    let within = parsed
        .within
        .as_ref()
        .ok_or_else(|| format!("{relative}: missing `within {expected_within};`"))?
        .to_string();
    if within != expected_within {
        return Err(format!(
            "{relative}: expected within `{expected_within}`, found `{within}`"
        ));
    }

    if parsed.classes.len() != 1 {
        return Err(format!(
            "{relative}: expected exactly one direct class `{expected_class}`, found {}",
            parsed.classes.len()
        ));
    }
    let class = parsed
        .classes
        .values()
        .next()
        .expect("class count checked above");
    let expected_canonical = format!("{expected_within}.{expected_class}");
    let actual_canonical = format!("{within}.{}", class.name.text);
    if actual_canonical != expected_canonical {
        return Err(format!(
            "{relative}: expected class `{expected_canonical}`, found `{actual_canonical}`"
        ));
    }
    Ok((class, expected_canonical))
}

fn expect_class_kind(
    class: &ClassDef,
    relative: &str,
    canonical: &str,
    expected: ClassType,
) -> Result<(), String> {
    if class.class_type != expected {
        return Err(format!(
            "{relative}: `{canonical}` must be a {}, found {}",
            expected.as_str(),
            class.class_type.as_str()
        ));
    }
    Ok(())
}

pub(crate) fn direct_component<'a>(
    class: &'a ClassDef,
    relative: &str,
    canonical: &str,
    name: &str,
) -> Result<&'a Component, String> {
    class.components.get(name).ok_or_else(|| {
        format!("{relative}: `{canonical}.{name}` must be a direct component declaration")
    })
}

pub(crate) fn expect_public_component(
    component: &Component,
    relative: &str,
    canonical: &str,
) -> Result<(), String> {
    if component.is_protected {
        return Err(format!(
            "{relative}: `{canonical}.{}` must be public",
            component.name
        ));
    }
    Ok(())
}

fn expect_declared_type(
    component: &Component,
    relative: &str,
    canonical: &str,
    expected_type: &str,
) -> Result<(), String> {
    let actual_type = component.type_name.to_string();
    if actual_type != expected_type {
        return Err(format!(
            "{relative}: `{canonical}.{}` must have declared type `{expected_type}`, found `{actual_type}`",
            component.name
        ));
    }
    Ok(())
}

fn verify_declaration(source: &str, requirement: &DeclarationRequirement) -> Result<(), String> {
    let relative = requirement.path();
    let parsed = parse_source(relative, source)?;
    let (expected_within, class_name) = requirement.identity();
    let (class, canonical) = direct_class(&parsed, relative, expected_within, class_name)?;

    match requirement {
        DeclarationRequirement::Block { members, .. } => {
            expect_class_kind(class, relative, &canonical, ClassType::Block)?;
            for member in *members {
                verify_member(class, relative, &canonical, member)?;
            }
        }
        DeclarationRequirement::Enumeration { literals, .. } => {
            expect_class_kind(class, relative, &canonical, ClassType::Type)?;
            let actual_literals: Vec<&str> = class
                .enum_literals
                .iter()
                .map(|literal| literal.ident.text.as_ref())
                .collect();
            if actual_literals.as_slice() != *literals {
                return Err(format!(
                    "{relative}: `{canonical}` enum literals must be [{}], found [{}]",
                    literals.join(", "),
                    actual_literals.join(", ")
                ));
            }
        }
    }
    Ok(())
}

fn verify_member(
    class: &ClassDef,
    relative: &str,
    canonical: &str,
    requirement: &MemberRequirement,
) -> Result<(), String> {
    match requirement {
        MemberRequirement::PublicParameter {
            name,
            declared_type,
        } => {
            let component = direct_component(class, relative, canonical, name)?;
            expect_public_component(component, relative, canonical)?;
            if !matches!(component.variability, Variability::Parameter(_)) {
                return Err(format!(
                    "{relative}: `{canonical}.{name}` must have parameter variability"
                ));
            }
            expect_declared_type(component, relative, canonical, declared_type)
        }
        MemberRequirement::PublicComponent {
            name,
            declared_type,
        } => {
            let component = direct_component(class, relative, canonical, name)?;
            expect_public_component(component, relative, canonical)?;
            expect_declared_type(component, relative, canonical, declared_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    const VALID_TRIM: &str = r#"
within Buildings.Controls.OBC.ASHRAE.G36.Generic;
block TrimAndRespond
  parameter Real samplePeriod;
  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;
end TrimAndRespond;
"#;

    const VALID_HEATING_COIL: &str = r#"
within Buildings.Controls.OBC.ASHRAE.G36.Types;
type HeatingCoil = enumeration(None, WaterBased, Electric);
"#;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cxf-verify-g36-declarations-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&path).expect("create temporary release root");
            Self { path }
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.path.join(relative);
            std::fs::create_dir_all(path.parent().expect("source path has parent"))
                .expect("create source parent");
            std::fs::write(path, bytes).expect("write source fixture");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).expect("remove temporary release root");
        }
    }

    const TRIM_REQUIREMENT: usize = 0;
    const HEATING_COIL_REQUIREMENT: usize = 1;

    fn catalog_requirement(index: usize) -> &'static DeclarationRequirement {
        &DECLARATION_REQUIREMENTS[index]
    }

    fn catalog_path(index: usize) -> &'static str {
        catalog_requirement(index).path()
    }

    fn catalog_identity(index: usize) -> (&'static str, &'static str) {
        catalog_requirement(index).identity()
    }

    fn catalog_canonical(index: usize) -> String {
        let (within, class_name) = catalog_identity(index);
        format!("{within}.{class_name}")
    }

    fn verify_catalog_source(index: usize, source: &str) -> Result<(), String> {
        verify_declaration(source, catalog_requirement(index))
    }

    fn trim_with(declaration: &str) -> String {
        let (within, class_name) = catalog_identity(TRIM_REQUIREMENT);
        format!("within {within};\nblock {class_name}\n  {declaration}\nend {class_name};\n")
    }

    #[test]
    fn catalog_keeps_fixed_source_order() {
        let paths: Vec<&str> = DECLARATION_REQUIREMENTS
            .iter()
            .map(DeclarationRequirement::path)
            .collect();
        assert_eq!(
            paths,
            [
                "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo",
                "Buildings/Controls/OBC/ASHRAE/G36/Types/HeatingCoil.mo",
            ]
        );
    }

    #[test]
    fn accepts_expected_declarations() {
        verify_catalog_source(TRIM_REQUIREMENT, VALID_TRIM)
            .expect("TrimAndRespond fixture conforms");
        verify_catalog_source(HEATING_COIL_REQUIREMENT, VALID_HEATING_COIL)
            .expect("HeatingCoil fixture conforms");

        let root = TempRoot::new();
        root.write(catalog_path(TRIM_REQUIREMENT), VALID_TRIM.as_bytes());
        root.write(
            catalog_path(HEATING_COIL_REQUIREMENT),
            VALID_HEATING_COIL.as_bytes(),
        );
        verify_release_declarations(&root.path).expect("release fixture conforms");
    }

    #[test]
    fn rejects_parse_failure() {
        let error = verify_catalog_source(
            TRIM_REQUIREMENT,
            "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond",
        )
        .expect_err("incomplete class must fail");
        assert_eq!(
            error,
            format!("{}: Modelica parse failed", catalog_path(TRIM_REQUIREMENT))
        );
    }

    #[test]
    fn rejects_missing_or_wrong_class_identity() {
        let path = catalog_path(TRIM_REQUIREMENT);
        let (within, class_name) = catalog_identity(TRIM_REQUIREMENT);
        let canonical = catalog_canonical(TRIM_REQUIREMENT);
        let cases = [
            (
                "block TrimAndRespond end TrimAndRespond;",
                format!("{path}: missing `within {within};`"),
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Types; block TrimAndRespond end TrimAndRespond;",
                format!(
                    "{path}: expected within `{within}`, found `Buildings.Controls.OBC.ASHRAE.G36.Types`"
                ),
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block Wrong end Wrong;",
                format!("{path}: expected class `{canonical}`, found `{within}.Wrong`"),
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond end TrimAndRespond; block Extra end Extra;",
                format!("{path}: expected exactly one direct class `{class_name}`, found 2"),
            ),
        ];
        for (source, expected) in cases {
            let error = verify_catalog_source(TRIM_REQUIREMENT, source)
                .expect_err("identity mismatch must fail");
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn rejects_wrong_class_kinds() {
        let trim_path = catalog_path(TRIM_REQUIREMENT);
        let trim_canonical = catalog_canonical(TRIM_REQUIREMENT);
        let trim = VALID_TRIM.replacen("block TrimAndRespond", "model TrimAndRespond", 1);
        let error =
            verify_catalog_source(TRIM_REQUIREMENT, &trim).expect_err("model is not a block");
        assert_eq!(
            error,
            format!("{trim_path}: `{trim_canonical}` must be a block, found model")
        );

        let heating_path = catalog_path(HEATING_COIL_REQUIREMENT);
        let heating_canonical = catalog_canonical(HEATING_COIL_REQUIREMENT);
        let heating = VALID_HEATING_COIL.replacen(
            "type HeatingCoil = enumeration(None, WaterBased, Electric);",
            "block HeatingCoil end HeatingCoil;",
            1,
        );
        let error = verify_catalog_source(HEATING_COIL_REQUIREMENT, &heating)
            .expect_err("block is not a type");
        assert_eq!(
            error,
            format!("{heating_path}: `{heating_canonical}` must be a type, found block")
        );
    }

    #[test]
    fn rejects_invalid_sample_period_declarations() {
        let path = catalog_path(TRIM_REQUIREMENT);
        let canonical = catalog_canonical(TRIM_REQUIREMENT);
        let (within, class_name) = catalog_identity(TRIM_REQUIREMENT);
        let cases = [
            (
                trim_with("Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;"),
                format!(
                    "{path}: `{canonical}.samplePeriod` must be a direct component declaration"
                ),
            ),
            (
                trim_with(
                    "type samplePeriod = Real; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                format!(
                    "{path}: `{canonical}.samplePeriod` must be a direct component declaration"
                ),
            ),
            (
                trim_with(
                    "Real samplePeriod; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                format!("{path}: `{canonical}.samplePeriod` must have parameter variability"),
            ),
            (
                trim_with(
                    "parameter Integer samplePeriod; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                format!(
                    "{path}: `{canonical}.samplePeriod` must have declared type `Real`, found `Integer`"
                ),
            ),
            (
                format!(
                    "within {within};\nblock {class_name}\nprotected\n  parameter Real samplePeriod;\npublic\n  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;\nend {class_name};\n"
                ),
                format!("{path}: `{canonical}.samplePeriod` must be public"),
            ),
        ];
        for (source, expected) in cases {
            let error = verify_catalog_source(TRIM_REQUIREMENT, &source)
                .expect_err("invalid samplePeriod declaration must fail");
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn rejects_invalid_num_of_req_declarations() {
        let path = catalog_path(TRIM_REQUIREMENT);
        let canonical = catalog_canonical(TRIM_REQUIREMENT);
        let (within, class_name) = catalog_identity(TRIM_REQUIREMENT);
        let cases = [
            (
                trim_with("parameter Real samplePeriod;"),
                format!("{path}: `{canonical}.numOfReq` must be a direct component declaration"),
            ),
            (
                trim_with("parameter Real samplePeriod; Integer numOfReq;"),
                format!(
                    "{path}: `{canonical}.numOfReq` must have declared type `Buildings.Controls.OBC.CDL.Interfaces.IntegerInput`, found `Integer`"
                ),
            ),
            (
                format!(
                    "within {within};\nblock {class_name}\n  parameter Real samplePeriod;\nprotected\n  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;\nend {class_name};\n"
                ),
                format!("{path}: `{canonical}.numOfReq` must be public"),
            ),
        ];
        for (source, expected) in cases {
            let error = verify_catalog_source(TRIM_REQUIREMENT, &source)
                .expect_err("invalid numOfReq declaration must fail");
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn rejects_wrong_enum_literals_or_order() {
        let path = catalog_path(HEATING_COIL_REQUIREMENT);
        let canonical = catalog_canonical(HEATING_COIL_REQUIREMENT);
        let cases = [
            (
                VALID_HEATING_COIL.replace(
                    "enumeration(None, WaterBased, Electric)",
                    "enumeration(None, Electric, WaterBased)",
                ),
                "None, Electric, WaterBased",
            ),
            (
                VALID_HEATING_COIL.replace(
                    "enumeration(None, WaterBased, Electric)",
                    "enumeration(None, WaterBased)",
                ),
                "None, WaterBased",
            ),
        ];
        for (source, found) in cases {
            let error = verify_catalog_source(HEATING_COIL_REQUIREMENT, &source)
                .expect_err("wrong enum list must fail");
            assert_eq!(
                error,
                format!(
                    "{path}: `{canonical}` enum literals must be [None, WaterBased, Electric], found [{found}]"
                )
            );
        }
    }

    #[test]
    fn first_source_failure_short_circuits_before_second_read() {
        let root = TempRoot::new();
        root.write(
            catalog_path(TRIM_REQUIREMENT),
            b"within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond",
        );

        let error = verify_release_declarations(&root.path)
            .expect_err("first source parse failure must short-circuit");
        assert_eq!(
            error,
            format!("{}: Modelica parse failed", catalog_path(TRIM_REQUIREMENT))
        );
    }

    #[test]
    fn rejects_missing_unreadable_or_non_utf8_sources() {
        let trim_path = catalog_path(TRIM_REQUIREMENT);
        let missing = TempRoot::new();
        let error =
            verify_release_declarations(&missing.path).expect_err("missing source must fail");
        assert_eq!(error, format!("{trim_path}: cannot read source (NotFound)"));

        let unreadable = TempRoot::new();
        std::fs::create_dir_all(unreadable.path.join(trim_path))
            .expect("create directory at source path");
        let error =
            verify_release_declarations(&unreadable.path).expect_err("unreadable source must fail");
        assert!(error.contains("cannot read source"), "{error}");

        let non_utf8 = TempRoot::new();
        non_utf8.write(trim_path, &[0xff, 0xfe]);
        let error =
            verify_release_declarations(&non_utf8.path).expect_err("non-UTF-8 source must fail");
        assert_eq!(error, format!("{trim_path}: source is not UTF-8"));
    }
}
