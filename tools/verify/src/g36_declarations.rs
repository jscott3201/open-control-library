use std::path::{Component as PathComponent, Path, PathBuf};

use rumoca_core::{ClassType, Variability};
use rumoca_ir_ast::{ClassDef, Component, StoredDefinition};

const TRIM_AND_RESPOND_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo";
const TRIM_AND_RESPOND_WITHIN: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic";
const TRIM_AND_RESPOND_CLASS: &str = "TrimAndRespond";
const TRIM_AND_RESPOND_CANONICAL: &str = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond";

const HEATING_COIL_PATH: &str = "Buildings/Controls/OBC/ASHRAE/G36/Types/HeatingCoil.mo";
const HEATING_COIL_WITHIN: &str = "Buildings.Controls.OBC.ASHRAE.G36.Types";
const HEATING_COIL_CLASS: &str = "HeatingCoil";
const HEATING_COIL_CANONICAL: &str = "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil";
const HEATING_COIL_LITERALS: [&str; 3] = ["None", "WaterBased", "Electric"];

pub(crate) fn verify_release_declarations(release_root: &Path) -> Result<(), String> {
    let trim_source = read_fixed_source(release_root, TRIM_AND_RESPOND_PATH)?;
    verify_trim_and_respond(&trim_source)?;

    let heating_coil_source = read_fixed_source(release_root, HEATING_COIL_PATH)?;
    verify_heating_coil(&heating_coil_source)
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

fn direct_class<'a>(
    parsed: &'a StoredDefinition,
    relative: &str,
    expected_within: &str,
    expected_class: &str,
    expected_canonical: &str,
) -> Result<&'a ClassDef, String> {
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
    let actual_canonical = format!("{within}.{}", class.name.text);
    if actual_canonical != expected_canonical {
        return Err(format!(
            "{relative}: expected class `{expected_canonical}`, found `{actual_canonical}`"
        ));
    }
    Ok(class)
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

fn direct_component<'a>(
    class: &'a ClassDef,
    relative: &str,
    canonical: &str,
    name: &str,
) -> Result<&'a Component, String> {
    class.components.get(name).ok_or_else(|| {
        format!("{relative}: `{canonical}.{name}` must be a direct component declaration")
    })
}

fn expect_public_component(
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

fn verify_trim_and_respond(source: &str) -> Result<(), String> {
    let parsed = parse_source(TRIM_AND_RESPOND_PATH, source)?;
    let class = direct_class(
        &parsed,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_WITHIN,
        TRIM_AND_RESPOND_CLASS,
        TRIM_AND_RESPOND_CANONICAL,
    )?;
    expect_class_kind(
        class,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
        ClassType::Block,
    )?;

    let sample_period = direct_component(
        class,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
        "samplePeriod",
    )?;
    expect_public_component(
        sample_period,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
    )?;
    if !matches!(sample_period.variability, Variability::Parameter(_)) {
        return Err(format!(
            "{TRIM_AND_RESPOND_PATH}: `{TRIM_AND_RESPOND_CANONICAL}.samplePeriod` must have parameter variability"
        ));
    }
    expect_declared_type(
        sample_period,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
        "Real",
    )?;

    let num_of_req = direct_component(
        class,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
        "numOfReq",
    )?;
    expect_public_component(
        num_of_req,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
    )?;
    expect_declared_type(
        num_of_req,
        TRIM_AND_RESPOND_PATH,
        TRIM_AND_RESPOND_CANONICAL,
        "Buildings.Controls.OBC.CDL.Interfaces.IntegerInput",
    )
}

fn verify_heating_coil(source: &str) -> Result<(), String> {
    let parsed = parse_source(HEATING_COIL_PATH, source)?;
    let class = direct_class(
        &parsed,
        HEATING_COIL_PATH,
        HEATING_COIL_WITHIN,
        HEATING_COIL_CLASS,
        HEATING_COIL_CANONICAL,
    )?;
    expect_class_kind(
        class,
        HEATING_COIL_PATH,
        HEATING_COIL_CANONICAL,
        ClassType::Type,
    )?;

    let literals: Vec<&str> = class
        .enum_literals
        .iter()
        .map(|literal| literal.ident.text.as_ref())
        .collect();
    if literals != HEATING_COIL_LITERALS {
        return Err(format!(
            "{HEATING_COIL_PATH}: `{HEATING_COIL_CANONICAL}` enum literals must be [None, WaterBased, Electric], found [{}]",
            literals.join(", ")
        ));
    }
    Ok(())
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

    fn trim_with(declaration: &str) -> String {
        format!(
            "within {TRIM_AND_RESPOND_WITHIN};\nblock {TRIM_AND_RESPOND_CLASS}\n  {declaration}\nend {TRIM_AND_RESPOND_CLASS};\n"
        )
    }

    #[test]
    fn accepts_expected_declarations() {
        verify_trim_and_respond(VALID_TRIM).expect("TrimAndRespond fixture conforms");
        verify_heating_coil(VALID_HEATING_COIL).expect("HeatingCoil fixture conforms");

        let root = TempRoot::new();
        root.write(TRIM_AND_RESPOND_PATH, VALID_TRIM.as_bytes());
        root.write(HEATING_COIL_PATH, VALID_HEATING_COIL.as_bytes());
        verify_release_declarations(&root.path).expect("release fixture conforms");
    }

    #[test]
    fn rejects_parse_failure() {
        let error = verify_trim_and_respond(
            "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond",
        )
        .expect_err("incomplete class must fail");
        assert_eq!(
            error,
            format!("{TRIM_AND_RESPOND_PATH}: Modelica parse failed")
        );
    }

    #[test]
    fn rejects_missing_or_wrong_class_identity() {
        let cases = [
            (
                "block TrimAndRespond end TrimAndRespond;",
                "missing `within Buildings.Controls.OBC.ASHRAE.G36.Generic;`",
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Types; block TrimAndRespond end TrimAndRespond;",
                "expected within `Buildings.Controls.OBC.ASHRAE.G36.Generic`",
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block Wrong end Wrong;",
                "expected class `Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond`",
            ),
            (
                "within Buildings.Controls.OBC.ASHRAE.G36.Generic; block TrimAndRespond end TrimAndRespond; block Extra end Extra;",
                "expected exactly one direct class `TrimAndRespond`, found 2",
            ),
        ];
        for (source, expected) in cases {
            let error = verify_trim_and_respond(source).expect_err("identity mismatch must fail");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_wrong_class_kinds() {
        let trim = VALID_TRIM.replacen("block TrimAndRespond", "model TrimAndRespond", 1);
        let error = verify_trim_and_respond(&trim).expect_err("model is not a block");
        assert!(error.contains("must be a block, found model"), "{error}");

        let heating = VALID_HEATING_COIL.replacen(
            "type HeatingCoil = enumeration(None, WaterBased, Electric);",
            "block HeatingCoil end HeatingCoil;",
            1,
        );
        let error = verify_heating_coil(&heating).expect_err("block is not a type");
        assert!(error.contains("must be a type, found block"), "{error}");
    }

    #[test]
    fn rejects_invalid_sample_period_declarations() {
        let cases = [
            (
                trim_with("Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;"),
                "samplePeriod` must be a direct component declaration",
            ),
            (
                trim_with(
                    "type samplePeriod = Real; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                "samplePeriod` must be a direct component declaration",
            ),
            (
                trim_with(
                    "Real samplePeriod; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                "samplePeriod` must have parameter variability",
            ),
            (
                trim_with(
                    "parameter Integer samplePeriod; Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;",
                ),
                "samplePeriod` must have declared type `Real`, found `Integer`",
            ),
            (
                format!(
                    "within {TRIM_AND_RESPOND_WITHIN};\nblock {TRIM_AND_RESPOND_CLASS}\nprotected\n  parameter Real samplePeriod;\npublic\n  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;\nend {TRIM_AND_RESPOND_CLASS};\n"
                ),
                "samplePeriod` must be public",
            ),
        ];
        for (source, expected) in cases {
            let error = verify_trim_and_respond(&source)
                .expect_err("invalid samplePeriod declaration must fail");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_invalid_num_of_req_declarations() {
        let cases = [
            (
                trim_with("parameter Real samplePeriod;"),
                "numOfReq` must be a direct component declaration",
            ),
            (
                trim_with("parameter Real samplePeriod; Integer numOfReq;"),
                "numOfReq` must have declared type `Buildings.Controls.OBC.CDL.Interfaces.IntegerInput`, found `Integer`",
            ),
            (
                format!(
                    "within {TRIM_AND_RESPOND_WITHIN};\nblock {TRIM_AND_RESPOND_CLASS}\n  parameter Real samplePeriod;\nprotected\n  Buildings.Controls.OBC.CDL.Interfaces.IntegerInput numOfReq;\nend {TRIM_AND_RESPOND_CLASS};\n"
                ),
                "numOfReq` must be public",
            ),
        ];
        for (source, expected) in cases {
            let error = verify_trim_and_respond(&source)
                .expect_err("invalid numOfReq declaration must fail");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_wrong_enum_literals_or_order() {
        for source in [
            VALID_HEATING_COIL.replace(
                "enumeration(None, WaterBased, Electric)",
                "enumeration(None, Electric, WaterBased)",
            ),
            VALID_HEATING_COIL.replace(
                "enumeration(None, WaterBased, Electric)",
                "enumeration(None, WaterBased)",
            ),
        ] {
            let error = verify_heating_coil(&source).expect_err("wrong enum list must fail");
            assert!(
                error.contains("enum literals must be [None, WaterBased, Electric]"),
                "{error}"
            );
        }
    }

    #[test]
    fn rejects_missing_unreadable_or_non_utf8_sources() {
        let missing = TempRoot::new();
        let error =
            verify_release_declarations(&missing.path).expect_err("missing source must fail");
        assert!(error.contains("cannot read source (NotFound)"), "{error}");

        let unreadable = TempRoot::new();
        std::fs::create_dir_all(unreadable.path.join(TRIM_AND_RESPOND_PATH))
            .expect("create directory at source path");
        let error =
            verify_release_declarations(&unreadable.path).expect_err("unreadable source must fail");
        assert!(error.contains("cannot read source"), "{error}");

        let non_utf8 = TempRoot::new();
        non_utf8.write(TRIM_AND_RESPOND_PATH, &[0xff, 0xfe]);
        let error =
            verify_release_declarations(&non_utf8.path).expect_err("non-UTF-8 source must fail");
        assert!(error.contains("source is not UTF-8"), "{error}");
    }
}
