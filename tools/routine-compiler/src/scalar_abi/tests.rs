use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate::resolution::{
    ComparisonOperator, ConnectorDefinition, ConnectorPresence, DimensionDefinition, DimensionKind,
    EnumInputValue, EnumMemberDefinition, Guard, GuardOperand, NamedTypeDefinition,
    ParameterDefinition, ParameterValue, ResolutionLimits, ResolvedConnector, ScalarValue, Shape,
    TypeDefinition, TypeUse, ValidatedResolutionInput, resolve_validated,
};

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("test value is finite")
}

fn real(value: f64) -> ScalarValue {
    ScalarValue::Real(finite(value))
}

fn integer(value: BigInt) -> ScalarValue {
    ScalarValue::Integer(value)
}

fn enum_value(type_id: &str, member_id: &str) -> ScalarValue {
    ScalarValue::Enum(EnumInputValue {
        type_id: type_id.to_owned(),
        member_id: member_id.to_owned(),
    })
}

fn bool_guard(expected: bool) -> ConnectorPresence {
    ConnectorPresence::Guarded(Guard::Compare {
        operator: ComparisonOperator::Eq,
        left: GuardOperand::Parameter("enabled".to_owned()),
        right: GuardOperand::Literal {
            type_use: TypeUse::Primitive(PrimitiveType::Boolean),
            value: ScalarValue::Boolean(expected),
        },
    })
}

fn mode_members(order: &[(&str, &str)]) -> Vec<EnumMemberDefinition> {
    order
        .iter()
        .map(|(member_id, symbol)| EnumMemberDefinition {
            member_id: (*member_id).to_owned(),
            symbol: (*symbol).to_owned(),
        })
        .collect()
}

fn fixture_with_mode_members(order: &[(&str, &str)]) -> ValidatedResolutionInput {
    let huge = BigInt::from(10_u8).pow(100);
    ValidatedResolutionInput {
        canonical_id: "G36-RUST-SCALAR-ABI-TEST".to_owned(),
        revision: huge.clone(),
        types: vec![
            TypeDefinition {
                type_id: "temperature".to_owned(),
                definition: NamedTypeDefinition::Alias {
                    primitive: PrimitiveType::Real,
                    quantity: Some("thermodynamic_temperature".to_owned()),
                    unit: Some("K".to_owned()),
                    display_unit: Some("degC".to_owned()),
                },
            },
            TypeDefinition {
                type_id: "mode".to_owned(),
                definition: NamedTypeDefinition::Enum {
                    members: mode_members(order),
                },
            },
            TypeDefinition {
                type_id: "switch".to_owned(),
                definition: NamedTypeDefinition::Enum {
                    members: mode_members(&[("off", "OFF"), ("on", "ON")]),
                },
            },
        ],
        dimensions: vec![
            DimensionDefinition {
                dimension_id: "pair".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["first".to_owned(), "second".to_owned()],
                },
            },
            DimensionDefinition {
                dimension_id: "zones".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["north".to_owned(), "south".to_owned()],
                },
            },
        ],
        parameters: vec![
            ParameterDefinition {
                parameter_id: "scalar_real".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Scalar,
                source: ParameterSource::Default,
                value: ParameterValue::Scalar(real(1.5)),
            },
            ParameterDefinition {
                parameter_id: "vector_alias".to_owned(),
                type_use: TypeUse::Named("temperature".to_owned()),
                shape: Shape::Rank1 {
                    dimension_id: "pair".to_owned(),
                },
                source: ParameterSource::Assignment,
                value: ParameterValue::Rank1(vec![real(-0.0), integer(BigInt::from(7_u8))]),
            },
            ParameterDefinition {
                parameter_id: "huge_integer".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(integer(huge)),
            },
            ParameterDefinition {
                parameter_id: "enabled".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Boolean(true)),
            },
            ParameterDefinition {
                parameter_id: "matrix".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "pair".to_owned(),
                },
                source: ParameterSource::Default,
                value: ParameterValue::Rank2(vec![
                    vec![real(1.0), integer(BigInt::from(2_u8))],
                    vec![real(3.0), real(4.0)],
                ]),
            },
            ParameterDefinition {
                parameter_id: "selected_mode".to_owned(),
                type_use: TypeUse::Named("mode".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(enum_value("mode", "local-mid")),
            },
            ParameterDefinition {
                parameter_id: "selected_switch".to_owned(),
                type_use: TypeUse::Named("switch".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Default,
                value: ParameterValue::Scalar(enum_value("switch", "on")),
            },
        ],
        connectors: vec![
            ConnectorDefinition {
                connector_id: "matrix_output".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Named("temperature".to_owned()),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "pair".to_owned(),
                },
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "guarded_input".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                presence: bool_guard(true),
            },
            ConnectorDefinition {
                connector_id: "inactive_enum".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Named("mode".to_owned()),
                shape: Shape::Scalar,
                presence: bool_guard(false),
            },
            ConnectorDefinition {
                connector_id: "active_enum".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Named("mode".to_owned()),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Always,
            },
        ],
    }
}

fn fixture() -> ValidatedResolutionInput {
    fixture_with_mode_members(&[
        ("local-high", "LOCAL_HIGH"),
        ("local-low", "LOCAL_LOW"),
        ("local-mid", "LOCAL_MID"),
    ])
}

fn resolve(input: &ValidatedResolutionInput) -> ResolvedSpecialization {
    resolve_validated(input, ResolutionLimits::default()).expect("fixture resolves")
}

fn mode_mapping() -> EnumAbiMapping {
    EnumAbiMapping {
        type_id: "mode".to_owned(),
        canonical_class_path: "Buildings.Controls.Types.Mode".to_owned(),
        source_members: vec![
            "Unused".to_owned(),
            "SourceMid".to_owned(),
            "SourceLow".to_owned(),
            "SourceHigh".to_owned(),
            "AnotherUnused".to_owned(),
        ],
        member_mappings: vec![
            EnumAbiMemberMapping {
                member_id: "local-low".to_owned(),
                source_literal: "SourceLow".to_owned(),
            },
            EnumAbiMemberMapping {
                member_id: "local-high".to_owned(),
                source_literal: "SourceHigh".to_owned(),
            },
            EnumAbiMemberMapping {
                member_id: "local-mid".to_owned(),
                source_literal: "SourceMid".to_owned(),
            },
        ],
    }
}

fn switch_mapping() -> EnumAbiMapping {
    EnumAbiMapping {
        type_id: "switch".to_owned(),
        canonical_class_path: "Buildings.Controls.Types.Switch".to_owned(),
        source_members: vec!["Off".to_owned(), "On".to_owned()],
        member_mappings: vec![
            EnumAbiMemberMapping {
                member_id: "off".to_owned(),
                source_literal: "Off".to_owned(),
            },
            EnumAbiMemberMapping {
                member_id: "on".to_owned(),
                source_literal: "On".to_owned(),
            },
        ],
    }
}

fn mappings() -> Vec<EnumAbiMapping> {
    vec![mode_mapping(), switch_mapping()]
}

fn parameter_rows<'a>(
    projection: &'a ScalarAbiProjection,
    parameter_id: &str,
) -> Vec<&'a ScalarParameterAbiRow> {
    projection
        .parameters
        .iter()
        .filter(|row| row.parameter_id == parameter_id)
        .collect()
}

fn error_codes(error: &ScalarAbiError) -> HashSet<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn assert_mapping_error(
    resolved: &ResolvedSpecialization,
    mappings: &[EnumAbiMapping],
    expected_codes: &[&str],
) {
    let first = project_scalar_abi(resolved, mappings).expect_err("mapping must fail");
    let second = project_scalar_abi(resolved, mappings).expect_err("mapping must fail again");
    assert_eq!(first, second);
    assert!(first.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    let codes = error_codes(&first);
    for code in expected_codes {
        assert!(codes.contains(code), "missing {code} in {first:?}");
    }
}

#[test]
fn typed_chain_preserves_order_types_values_and_resolved_connector_state() {
    let mut input = fixture();
    let input_before = input.clone();
    let resolved = resolve(&input);
    let huge = BigInt::from(10_u8).pow(100);

    let ParameterValue::Scalar(ScalarValue::Boolean(enabled)) = &mut input.parameters[3].value
    else {
        panic!("fixture enabled parameter is a scalar Boolean");
    };
    *enabled = false;
    let projection = project_scalar_abi(&resolved, &mappings()).expect("projection succeeds");

    assert_eq!(
        input_before.parameters[3].value,
        ParameterValue::Scalar(ScalarValue::Boolean(true))
    );
    assert_eq!(projection.canonical_id, "G36-RUST-SCALAR-ABI-TEST");
    assert_eq!(projection.revision, huge);
    assert_eq!(
        projection
            .parameters
            .iter()
            .map(|row| row.parameter_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "scalar_real",
            "vector_alias",
            "vector_alias",
            "huge_integer",
            "enabled",
            "matrix",
            "matrix",
            "matrix",
            "matrix",
            "selected_mode",
            "selected_switch",
        ]
    );

    let scalar = parameter_rows(&projection, "scalar_real")[0];
    assert!(scalar.coordinates.is_empty());
    assert_eq!(
        scalar.abi_type,
        ScalarAbiType::Primitive(PrimitiveType::Real)
    );
    assert_eq!(scalar.source, ParameterSource::Default);
    assert_eq!(scalar.value, ScalarAbiValue::Real(finite(1.5)));

    let vector = parameter_rows(&projection, "vector_alias");
    assert_eq!(
        vector
            .iter()
            .map(|row| (
                row.coordinates[0].member_id.as_str(),
                row.coordinates[0].ordinal,
                row.value.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("first", 0, ScalarAbiValue::Real(finite(-0.0))),
            ("second", 1, ScalarAbiValue::Integer(BigInt::from(7_u8)),),
        ]
    );
    let ScalarAbiValue::Real(negative_zero) = vector[0].value else {
        panic!("negative zero remains a real");
    };
    assert_eq!(negative_zero.get().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(
        vector[0].abi_type,
        ScalarAbiType::Alias {
            type_id: "temperature".to_owned(),
            primitive: PrimitiveType::Real,
            quantity: Some("thermodynamic_temperature".to_owned()),
            unit: Some("K".to_owned()),
            display_unit: Some("degC".to_owned()),
        }
    );
    assert_eq!(vector[0].source, ParameterSource::Assignment);

    assert_eq!(
        parameter_rows(&projection, "huge_integer")[0].value,
        ScalarAbiValue::Integer(BigInt::from(10_u8).pow(100))
    );
    assert_eq!(
        parameter_rows(&projection, "enabled")[0].value,
        ScalarAbiValue::Boolean(true)
    );
    let matrix = parameter_rows(&projection, "matrix");
    assert_eq!(
        matrix
            .iter()
            .map(|row| (
                row.coordinates
                    .iter()
                    .map(|coordinate| (
                        coordinate.dimension_id.as_str(),
                        coordinate.member_id.as_str(),
                        coordinate.ordinal,
                    ))
                    .collect::<Vec<_>>(),
                row.value.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                vec![("zones", "north", 0), ("pair", "first", 0)],
                ScalarAbiValue::Real(finite(1.0)),
            ),
            (
                vec![("zones", "north", 0), ("pair", "second", 1)],
                ScalarAbiValue::Integer(BigInt::from(2_u8)),
            ),
            (
                vec![("zones", "south", 1), ("pair", "first", 0)],
                ScalarAbiValue::Real(finite(3.0)),
            ),
            (
                vec![("zones", "south", 1), ("pair", "second", 1)],
                ScalarAbiValue::Real(finite(4.0)),
            ),
        ]
    );

    let selected_mode = parameter_rows(&projection, "selected_mode")[0];
    assert_eq!(
        selected_mode.abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: "Buildings.Controls.Types.Mode".to_owned(),
        }
    );
    assert_eq!(selected_mode.value, ScalarAbiValue::Enum { ordinal: 2 });
    assert_eq!(
        parameter_rows(&projection, "selected_switch")[0].value,
        ScalarAbiValue::Enum { ordinal: 2 }
    );

    assert_eq!(
        projection
            .connectors
            .iter()
            .map(|row| row.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "matrix_output",
            "matrix_output",
            "matrix_output",
            "matrix_output",
            "guarded_input",
            "active_enum",
        ]
    );
    assert_eq!(
        projection.connectors[0].direction,
        ConnectorDirection::Output
    );
    assert_eq!(
        projection.connectors[4].direction,
        ConnectorDirection::Input
    );
    assert!(
        projection
            .connectors
            .iter()
            .all(|row| row.connector_id != "inactive_enum")
    );
}

#[test]
fn enum_ordinals_ignore_local_and_mapping_order_and_allow_extra_source_members() {
    let first = resolve(&fixture_with_mode_members(&[
        ("local-high", "SYMBOL_9"),
        ("local-low", "SYMBOL_2"),
        ("local-mid", "SYMBOL_5"),
    ]));
    let second = resolve(&fixture_with_mode_members(&[
        ("local-mid", "ARBITRARY_A"),
        ("local-high", "ARBITRARY_B"),
        ("local-low", "ARBITRARY_C"),
    ]));
    let first_mappings = mappings();
    let mut reordered_mode = mode_mapping();
    reordered_mode.member_mappings.reverse();
    let second_mappings = vec![switch_mapping(), reordered_mode];

    let first_projection =
        project_scalar_abi(&first, &first_mappings).expect("first projection succeeds");
    let second_projection =
        project_scalar_abi(&second, &second_mappings).expect("second projection succeeds");
    assert_eq!(first_projection, second_projection);
    assert_eq!(
        parameter_rows(&first_projection, "selected_mode")[0].value,
        ScalarAbiValue::Enum { ordinal: 2 }
    );
    assert_eq!(mode_mapping().source_members.len(), 5);
}

#[test]
fn inactive_enum_connector_alone_needs_no_mapping_and_emits_no_row() {
    let resolved = ResolvedSpecialization {
        canonical_id: "inactive-only".to_owned(),
        revision: BigInt::from(1_u8),
        dimensions: Vec::new(),
        parameters: Vec::new(),
        connectors: vec![ResolvedConnector {
            connector_id: "inactive_enum".to_owned(),
            direction: ConnectorDirection::Input,
            resolved_type: ResolvedType::Enum {
                type_id: "mode".to_owned(),
                members: vec![ResolvedEnumMember {
                    member_id: "off".to_owned(),
                    symbol: "OFF".to_owned(),
                }],
            },
            dimension_ids: Vec::new(),
            active: false,
            guard_result: Some(false),
            leaves: Vec::new(),
        }],
    };

    let projection = project_scalar_abi(&resolved, &[]).expect("inactive enum is ignored");
    assert!(projection.connectors.is_empty());
}

#[test]
fn enum_mapping_diagnostics_cover_invalid_claims_and_are_sorted_and_atomic() {
    let resolved = resolve(&fixture());
    assert_mapping_error(&resolved, &[], &["unsupported_enum"]);
    assert_mapping_error(
        &resolved,
        &[mode_mapping(), mode_mapping(), switch_mapping()],
        &["duplicate_enum_mapping"],
    );

    let mut unknown = mode_mapping();
    unknown.type_id = "missing".to_owned();
    assert_mapping_error(
        &resolved,
        &[mode_mapping(), switch_mapping(), unknown],
        &["unknown_enum_mapping_type"],
    );

    let non_enum = EnumAbiMapping {
        type_id: "temperature".to_owned(),
        canonical_class_path: "Example.Temperature".to_owned(),
        source_members: vec!["Value".to_owned()],
        member_mappings: Vec::new(),
    };
    assert_mapping_error(
        &resolved,
        &[mode_mapping(), switch_mapping(), non_enum],
        &["non_enum_mapping_type"],
    );

    let mut missing = mode_mapping();
    missing.member_mappings.pop();
    assert_mapping_error(
        &resolved,
        &[missing, switch_mapping()],
        &["missing_enum_local_member"],
    );

    let mut extra = mode_mapping();
    extra.source_members.push("Spare".to_owned());
    extra.member_mappings.push(EnumAbiMemberMapping {
        member_id: "spare".to_owned(),
        source_literal: "Spare".to_owned(),
    });
    assert_mapping_error(
        &resolved,
        &[extra, switch_mapping()],
        &["extra_enum_local_member"],
    );

    let mut duplicate_local = mode_mapping();
    duplicate_local.source_members.push("Spare".to_owned());
    duplicate_local.member_mappings.push(EnumAbiMemberMapping {
        member_id: "local-low".to_owned(),
        source_literal: "Spare".to_owned(),
    });
    assert_mapping_error(
        &resolved,
        &[duplicate_local, switch_mapping()],
        &["duplicate_enum_local_member"],
    );

    let mut duplicate_literal = mode_mapping();
    duplicate_literal.member_mappings[1].source_literal = "SourceLow".to_owned();
    assert_mapping_error(
        &resolved,
        &[duplicate_literal, switch_mapping()],
        &["duplicate_enum_source_literal"],
    );

    let mut duplicate_source = mode_mapping();
    duplicate_source.source_members.push("SourceLow".to_owned());
    assert_mapping_error(
        &resolved,
        &[duplicate_source, switch_mapping()],
        &["duplicate_enum_source_member"],
    );

    let mut unknown_literal = mode_mapping();
    unknown_literal.member_mappings[0].source_literal = "Missing".to_owned();
    assert_mapping_error(
        &resolved,
        &[unknown_literal, switch_mapping()],
        &["unknown_enum_source_literal"],
    );

    let malformed = EnumAbiMapping {
        type_id: String::new(),
        canonical_class_path: String::new(),
        source_members: vec![String::new()],
        member_mappings: vec![EnumAbiMemberMapping {
            member_id: String::new(),
            source_literal: String::new(),
        }],
    };
    assert_mapping_error(
        &resolved,
        &[mode_mapping(), switch_mapping(), malformed],
        &["invalid_enum_mapping"],
    );
}

fn assert_malformed_resolved(resolved: ResolvedSpecialization, expected_message: &str) {
    let mappings = mappings();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        project_scalar_abi(&resolved, &mappings)
    }));
    let result = outcome.expect("malformed resolved input must not panic");
    let error = result.expect_err("malformed resolved input must fail");
    assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(
        error
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected_message)),
        "missing message {expected_message:?} in {error:?}"
    );
}

#[test]
fn malformed_manually_constructed_resolved_values_return_errors_without_panics() {
    let baseline = resolve(&fixture());

    let mut resolved = baseline.clone();
    resolved.parameters[2].leaves[0].value = ResolvedScalarValue::Boolean(false);
    assert_malformed_resolved(resolved, "does not match resolved primitive");

    let mut resolved = baseline.clone();
    let ResolvedScalarValue::Enum(value) = &mut resolved.parameters[5].leaves[0].value else {
        panic!("fixture mode is enum-valued");
    };
    value.type_id = "switch".to_owned();
    assert_malformed_resolved(resolved, "enum type `switch` does not match `mode`");

    let mut resolved = baseline.clone();
    let ResolvedScalarValue::Enum(value) = &mut resolved.parameters[5].leaves[0].value else {
        panic!("fixture mode is enum-valued");
    };
    value.member_id = "missing".to_owned();
    assert_malformed_resolved(resolved, "names unknown member `missing`");

    let mut resolved = baseline.clone();
    let ResolvedScalarValue::Enum(value) = &mut resolved.parameters[5].leaves[0].value else {
        panic!("fixture mode is enum-valued");
    };
    value.symbol = "DRIFTED".to_owned();
    assert_malformed_resolved(resolved, "enum symbol `DRIFTED`");

    let mut resolved = baseline.clone();
    resolved.parameters[4].leaves[1].coordinates[1].ordinal = 0;
    assert_malformed_resolved(resolved, "expected 1 for row-major order");

    let mut resolved = baseline.clone();
    resolved.dimensions[0].extent = 3;
    assert_malformed_resolved(resolved, "extent 3 does not match 2 members");

    let mut resolved = baseline.clone();
    resolved.connectors[2]
        .leaves
        .push(crate::resolution::ScalarConnectorLeaf {
            coordinates: Vec::new(),
        });
    assert_malformed_resolved(resolved, "inactive connector has 1 leaves");

    let mut resolved = baseline.clone();
    resolved.connectors[1].guard_result = Some(false);
    assert_malformed_resolved(resolved, "guard result contradicts active state");

    let mut resolved = baseline;
    resolved.connectors[0].leaves.clear();
    assert_malformed_resolved(resolved, "has 0 leaves, expected 4");
}

#[test]
fn projection_is_repeatable_detached_and_does_not_mutate_inputs() {
    let mut resolved = resolve(&fixture());
    let mut mappings = mappings();
    let resolved_before = resolved.clone();
    let mappings_before = mappings.clone();
    let first = project_scalar_abi(&resolved, &mappings).expect("projection succeeds");
    let second = project_scalar_abi(&resolved, &mappings).expect("projection repeats");
    assert_eq!(first, second);
    assert_eq!(resolved, resolved_before);
    assert_eq!(mappings, mappings_before);

    resolved.canonical_id = "changed".to_owned();
    resolved.parameters[1].leaves[0].coordinates[0].member_id = "changed".to_owned();
    resolved.parameters[2].leaves[0].value = ResolvedScalarValue::Integer(BigInt::from(0_u8));
    mappings[0].canonical_class_path = "Changed.Path".to_owned();
    mappings[0].source_members[1] = "ChangedLiteral".to_owned();

    assert_eq!(first.canonical_id, "G36-RUST-SCALAR-ABI-TEST");
    assert_eq!(
        parameter_rows(&first, "vector_alias")[0].coordinates[0].member_id,
        "first"
    );
    assert_eq!(
        parameter_rows(&first, "huge_integer")[0].value,
        ScalarAbiValue::Integer(BigInt::from(10_u8).pow(100))
    );
    assert_eq!(
        parameter_rows(&first, "selected_mode")[0].abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: "Buildings.Controls.Types.Mode".to_owned(),
        }
    );
}
