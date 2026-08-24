use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use num_bigint::BigInt;

use super::*;
use crate::resolution::{
    ComparisonOperator, ConnectorDefinition, ConnectorPresence, DimensionDefinition, DimensionKind,
    EnumInputValue, EnumMemberDefinition, FiniteReal, Guard, GuardOperand, NamedTypeDefinition,
    ParameterDefinition, ParameterSource, ParameterValue, ResolutionLimits, ScalarValue, Shape,
    TypeDefinition, TypeUse, ValidatedResolutionInput, resolve_validated,
};
use crate::scalar_abi::{
    EnumAbiMapping, EnumAbiMemberMapping, ScalarAbiProjection, ScalarAbiType, ScalarAbiValue,
    ScalarConnectorAbiRow, ScalarCoordinate, ScalarParameterAbiRow, project_scalar_abi,
};

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("test value is finite")
}

fn real(value: f64) -> ScalarValue {
    ScalarValue::Real(finite(value))
}

fn integer(value: impl Into<BigInt>) -> ScalarValue {
    ScalarValue::Integer(value.into())
}

fn typed_fixture() -> ValidatedResolutionInput {
    let huge = BigInt::from(10_u8).pow(80);
    ValidatedResolutionInput {
        canonical_id: "G36-RUST-SCALAR-NAME-TEST".to_owned(),
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
                type_id: "operating_mode".to_owned(),
                definition: NamedTypeDefinition::Enum {
                    members: vec![
                        EnumMemberDefinition {
                            member_id: "occupied".to_owned(),
                            symbol: "OCCUPIED".to_owned(),
                        },
                        EnumMemberDefinition {
                            member_id: "warm-up".to_owned(),
                            symbol: "WARM_UP".to_owned(),
                        },
                        EnumMemberDefinition {
                            member_id: "unoccupied".to_owned(),
                            symbol: "UNOCCUPIED".to_owned(),
                        },
                    ],
                },
            },
        ],
        dimensions: vec![
            DimensionDefinition {
                dimension_id: "fixed_pair".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["primary".to_owned(), "secondary".to_owned()],
                },
            },
            DimensionDefinition {
                dimension_id: "zones".to_owned(),
                kind: DimensionKind::Fixed {
                    members: vec!["north-zone".to_owned(), "south-zone".to_owned()],
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
                parameter_id: "fixed_gains".to_owned(),
                type_use: TypeUse::Named("temperature".to_owned()),
                shape: Shape::Rank1 {
                    dimension_id: "fixed_pair".to_owned(),
                },
                source: ParameterSource::Assignment,
                value: ParameterValue::Rank1(vec![real(-0.0), integer(7_u8)]),
            },
            ParameterDefinition {
                parameter_id: "huge_integer".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(integer(huge)),
            },
            ParameterDefinition {
                parameter_id: "matrix_weights".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "fixed_pair".to_owned(),
                },
                source: ParameterSource::Default,
                value: ParameterValue::Rank2(vec![
                    vec![real(1.0), integer(2_u8)],
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
                    member_id: "warm-up".to_owned(),
                })),
            },
            ParameterDefinition {
                parameter_id: "enabled".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Boolean(false)),
            },
        ],
        connectors: vec![
            ConnectorDefinition {
                connector_id: "supply_air_flow".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "zone_temperatures".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Named("temperature".to_owned()),
                shape: Shape::Rank1 {
                    dimension_id: "zones".to_owned(),
                },
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "matrix_feedback".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "fixed_pair".to_owned(),
                },
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "trim_request".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Guarded(Guard::Compare {
                    operator: ComparisonOperator::Eq,
                    left: GuardOperand::Parameter("enabled".to_owned()),
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
        canonical_class_path: "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil".to_owned(),
        source_members: vec![
            "None".to_owned(),
            "WaterBased".to_owned(),
            "Electric".to_owned(),
        ],
        member_mappings: vec![
            EnumAbiMemberMapping {
                member_id: "occupied".to_owned(),
                source_literal: "None".to_owned(),
            },
            EnumAbiMemberMapping {
                member_id: "warm-up".to_owned(),
                source_literal: "WaterBased".to_owned(),
            },
            EnumAbiMemberMapping {
                member_id: "unoccupied".to_owned(),
                source_literal: "Electric".to_owned(),
            },
        ],
    }
}

fn typed_projection() -> ScalarAbiProjection {
    let resolved = resolve_validated(&typed_fixture(), ResolutionLimits::default())
        .expect("typed fixture resolves");
    project_scalar_abi(&resolved, &[enum_mapping()]).expect("typed fixture projects")
}

fn coordinate(dimension_id: &str, member_id: &str, ordinal: usize) -> ScalarCoordinate {
    ScalarCoordinate {
        dimension_id: dimension_id.to_owned(),
        member_id: member_id.to_owned(),
        ordinal,
    }
}

fn parameter_row(parameter_id: &str, coordinates: Vec<ScalarCoordinate>) -> ScalarParameterAbiRow {
    ScalarParameterAbiRow {
        parameter_id: parameter_id.to_owned(),
        coordinates,
        abi_type: ScalarAbiType::Primitive(PrimitiveType::Real),
        source: ParameterSource::Default,
        value: ScalarAbiValue::Real(finite(1.0)),
    }
}

fn connector_row(connector_id: &str, coordinates: Vec<ScalarCoordinate>) -> ScalarConnectorAbiRow {
    ScalarConnectorAbiRow {
        connector_id: connector_id.to_owned(),
        coordinates,
        abi_type: ScalarAbiType::Primitive(PrimitiveType::Real),
        direction: ConnectorDirection::Input,
    }
}

fn direct_projection(
    parameters: Vec<ScalarParameterAbiRow>,
    connectors: Vec<ScalarConnectorAbiRow>,
) -> ScalarAbiProjection {
    ScalarAbiProjection {
        canonical_id: "G36-RUST-DIRECT-NAME-TEST".to_owned(),
        revision: BigInt::from(1_u8),
        parameters,
        connectors,
    }
}

fn named_parameters<'a>(
    projection: &'a NamedScalarProjection,
    parameter_id: &str,
) -> Vec<&'a NamedScalarParameterRow> {
    projection
        .parameters
        .iter()
        .filter(|row| row.parameter_id == parameter_id)
        .collect()
}

#[test]
fn typed_chain_assigns_exact_scalar_vector_and_matrix_names_in_abi_order() {
    let projection = typed_projection();
    let named = allocate_scalar_names(&projection).expect("name allocation succeeds");

    assert_eq!(named.canonical_id, projection.canonical_id);
    assert_eq!(named.revision, BigInt::from(10_u8).pow(80));
    assert_eq!(
        named
            .parameters
            .iter()
            .map(|row| row.parameter_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "sample_period_s",
            "fixed_gains",
            "fixed_gains",
            "huge_integer",
            "matrix_weights",
            "matrix_weights",
            "matrix_weights",
            "matrix_weights",
            "initial_mode",
            "enabled",
        ]
    );
    assert_eq!(
        named_parameters(&named, "sample_period_s")[0].scalar_name,
        "p_73616d706c655f706572696f645f73"
    );
    assert_eq!(
        named_parameters(&named, "fixed_gains")
            .iter()
            .map(|row| row.scalar_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "p_66697865645f6761696e73_66697865645f70616972_7072696d617279",
            "p_66697865645f6761696e73_66697865645f70616972_7365636f6e64617279",
        ]
    );
    assert_eq!(
        named_parameters(&named, "matrix_weights")
            .iter()
            .map(|row| row.scalar_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "p_6d61747269785f77656967687473_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7072696d617279",
            "p_6d61747269785f77656967687473_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7365636f6e64617279",
            "p_6d61747269785f77656967687473_7a6f6e6573_736f7574682d7a6f6e65_66697865645f70616972_7072696d617279",
            "p_6d61747269785f77656967687473_7a6f6e6573_736f7574682d7a6f6e65_66697865645f70616972_7365636f6e64617279",
        ]
    );
    assert_eq!(
        named
            .connectors
            .iter()
            .map(|row| row.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "supply_air_flow",
            "zone_temperatures",
            "zone_temperatures",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
            "matrix_feedback",
        ]
    );
    assert_eq!(
        named.connectors[0].scalar_name,
        "c_737570706c795f6169725f666c6f77"
    );
    assert_eq!(
        named.connectors[1].scalar_name,
        "c_7a6f6e655f74656d706572617475726573_7a6f6e6573_6e6f7274682d7a6f6e65"
    );
    assert_eq!(
        named.connectors[3].scalar_name,
        "c_6d61747269785f666565646261636b_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7072696d617279"
    );
    assert!(
        named
            .connectors
            .iter()
            .all(|row| row.connector_id != "trim_request")
    );

    for (named_row, abi_row) in named.parameters.iter().zip(&projection.parameters) {
        assert_eq!(named_row.parameter_id, abi_row.parameter_id);
        assert_eq!(named_row.coordinates, abi_row.coordinates);
        assert_eq!(named_row.abi_type, abi_row.abi_type);
        assert_eq!(named_row.source, abi_row.source);
        assert_eq!(named_row.value, abi_row.value);
    }
    for (named_row, abi_row) in named.connectors.iter().zip(&projection.connectors) {
        assert_eq!(named_row.connector_id, abi_row.connector_id);
        assert_eq!(named_row.coordinates, abi_row.coordinates);
        assert_eq!(named_row.abi_type, abi_row.abi_type);
        assert_eq!(named_row.direction, abi_row.direction);
    }

    let fixed = named_parameters(&named, "fixed_gains");
    let ScalarAbiValue::Real(negative_zero) = fixed[0].value else {
        panic!("first fixed gain must remain a real")
    };
    assert_eq!(negative_zero.get().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(fixed[1].value, ScalarAbiValue::Integer(BigInt::from(7_u8)));
    assert_eq!(
        fixed[0].abi_type,
        ScalarAbiType::Alias {
            type_id: "temperature".to_owned(),
            primitive: PrimitiveType::Real,
            quantity: Some("thermodynamic_temperature".to_owned()),
            unit: Some("K".to_owned()),
            display_unit: Some("degC".to_owned()),
        }
    );
    assert_eq!(fixed[0].source, ParameterSource::Assignment);
    assert_eq!(
        named_parameters(&named, "huge_integer")[0].value,
        ScalarAbiValue::Integer(BigInt::from(10_u8).pow(80))
    );
    let initial_mode = named_parameters(&named, "initial_mode")[0];
    assert_eq!(
        initial_mode.abi_type,
        ScalarAbiType::Enum {
            canonical_class_path: "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil".to_owned(),
        }
    );
    assert_eq!(initial_mode.value, ScalarAbiValue::Enum { ordinal: 2 });
    assert_eq!(initial_mode.source, ParameterSource::Assignment);
    assert_eq!(named.connectors[0].direction, ConnectorDirection::Output);
    assert_eq!(named.connectors[1].direction, ConnectorDirection::Input);
}

#[test]
fn names_use_stable_utf8_ids_not_ordinals_positions_or_normalization() {
    let north = parameter_row("gain", vec![coordinate("zones", "north-zone", 0)]);
    let south = parameter_row("gain", vec![coordinate("zones", "south-zone", 1)]);
    let first = allocate_scalar_names(&direct_projection(
        vec![north.clone(), south.clone()],
        Vec::new(),
    ))
    .expect("first allocation succeeds");
    let mut moved_south = south;
    moved_south.coordinates[0].ordinal = 71;
    let mut moved_north = north;
    moved_north.coordinates[0].ordinal = 99;
    let reordered = allocate_scalar_names(&direct_projection(
        vec![moved_south, moved_north],
        Vec::new(),
    ))
    .expect("reordered allocation succeeds");
    let by_member = |value: &NamedScalarProjection| {
        value
            .parameters
            .iter()
            .map(|row| {
                (
                    row.coordinates[0].member_id.clone(),
                    row.scalar_name.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(by_member(&first), by_member(&reordered));
    assert_eq!(
        reordered.parameters[0].coordinates[0].member_id,
        "south-zone"
    );
    assert_eq!(reordered.parameters[1].coordinates[0].ordinal, 99);

    let encoded = allocate_scalar_names(&direct_projection(
        vec![
            parameter_row("a_b", vec![coordinate("d_e", "μ_雪", 0)]),
            parameter_row("a", vec![coordinate("b_d", "e_μ_雪", 1)]),
            parameter_row("é", Vec::new()),
            parameter_row("e\u{301}", Vec::new()),
        ],
        Vec::new(),
    ))
    .expect("Unicode allocation succeeds");
    assert_eq!(
        encoded
            .parameters
            .iter()
            .map(|row| row.scalar_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "p_615f62_645f65_cebc5fe99baa",
            "p_61_625f64_655fcebc5fe99baa",
            "p_c3a9",
            "p_65cc81",
        ]
    );
    assert_ne!(
        encoded.parameters[2].scalar_name,
        encoded.parameters[3].scalar_name
    );
    for row in &encoded.parameters {
        assert!(row.scalar_name[2..].split('_').all(|component| {
            !component.is_empty()
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }));
    }
}

#[test]
fn namespaces_are_disjoint_and_duplicates_are_reported_within_each_kind() {
    let coordinates = vec![coordinate("zones", "north-zone", 7)];
    let parameter = parameter_row("shared", coordinates.clone());
    let connector = connector_row("shared", coordinates);
    let named = allocate_scalar_names(&direct_projection(
        vec![parameter.clone()],
        vec![connector.clone()],
    ))
    .expect("cross-namespace names do not collide");
    assert_eq!(
        named.parameters[0].scalar_name,
        "p_736861726564_7a6f6e6573_6e6f7274682d7a6f6e65"
    );
    assert_eq!(
        named.connectors[0].scalar_name,
        "c_736861726564_7a6f6e6573_6e6f7274682d7a6f6e65"
    );

    let error = allocate_scalar_names(&direct_projection(
        vec![parameter.clone(), parameter],
        vec![connector.clone(), connector],
    ))
    .expect_err("same-namespace duplicates must fail");
    assert_eq!(error.diagnostics.len(), 2);
    assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(error.diagnostics.iter().all(|item| {
        item.code == "duplicate_scalar_name"
            && (item.owner_kind == "parameter" || item.owner_kind == "connector")
    }));
    assert_eq!(
        error
            .diagnostics
            .iter()
            .map(|item| item.location.as_str())
            .collect::<Vec<_>>(),
        vec!["$.connectors", "$.parameters"]
    );
}

#[test]
fn malformed_manual_projection_returns_sorted_atomic_errors_without_panicking() {
    let malformed = ScalarAbiProjection {
        canonical_id: String::new(),
        revision: BigInt::from(0_u8),
        parameters: vec![
            ScalarParameterAbiRow {
                parameter_id: String::new(),
                coordinates: vec![coordinate("", "", usize::MAX)],
                abi_type: ScalarAbiType::Alias {
                    type_id: String::new(),
                    primitive: PrimitiveType::Boolean,
                    quantity: Some(String::new()),
                    unit: None,
                    display_unit: None,
                },
                source: ParameterSource::Default,
                value: ScalarAbiValue::Integer(BigInt::from(1_u8)),
            },
            ScalarParameterAbiRow {
                parameter_id: "integer_mismatch".to_owned(),
                coordinates: Vec::new(),
                abi_type: ScalarAbiType::Primitive(PrimitiveType::Integer),
                source: ParameterSource::Assignment,
                value: ScalarAbiValue::Boolean(true),
            },
            ScalarParameterAbiRow {
                parameter_id: "real_mismatch".to_owned(),
                coordinates: Vec::new(),
                abi_type: ScalarAbiType::Primitive(PrimitiveType::Real),
                source: ParameterSource::Default,
                value: ScalarAbiValue::Boolean(false),
            },
            ScalarParameterAbiRow {
                parameter_id: "enum_bad".to_owned(),
                coordinates: Vec::new(),
                abi_type: ScalarAbiType::Enum {
                    canonical_class_path: String::new(),
                },
                source: ParameterSource::Assignment,
                value: ScalarAbiValue::Enum { ordinal: 0 },
            },
            ScalarParameterAbiRow {
                parameter_id: "boolean_mismatch".to_owned(),
                coordinates: Vec::new(),
                abi_type: ScalarAbiType::Primitive(PrimitiveType::Boolean),
                source: ParameterSource::Default,
                value: ScalarAbiValue::Real(finite(1.0)),
            },
            ScalarParameterAbiRow {
                parameter_id: "enum_mismatch".to_owned(),
                coordinates: Vec::new(),
                abi_type: ScalarAbiType::Enum {
                    canonical_class_path: "Example.Mode".to_owned(),
                },
                source: ParameterSource::Default,
                value: ScalarAbiValue::Integer(BigInt::from(1_u8)),
            },
        ],
        connectors: vec![ScalarConnectorAbiRow {
            connector_id: String::new(),
            coordinates: Vec::new(),
            abi_type: ScalarAbiType::Enum {
                canonical_class_path: String::new(),
            },
            direction: ConnectorDirection::Input,
        }],
    };

    let mut attempts = Vec::new();
    for _ in 0..2 {
        let outcome = catch_unwind(AssertUnwindSafe(|| allocate_scalar_names(&malformed)));
        let result = outcome.expect("malformed input must not panic");
        let error = result.expect_err("malformed input must fail atomically");
        assert!(error.diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        let codes = error
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>();
        for expected in [
            "invalid_metadata",
            "invalid_owner_id",
            "invalid_dimension_id",
            "invalid_member_id",
            "invalid_abi_payload",
        ] {
            assert!(codes.contains(&expected), "missing {expected} in {error:?}");
        }
        assert!(error.diagnostics.iter().any(|item| {
            item.location == "$.parameters[3].value.ordinal"
                && item.message == "enum ordinal must be one-based"
        }));
        assert!(error.diagnostics.iter().any(|item| {
            item.location == "$.parameters[0].abi_type.type_id"
                && item.message == "alias type ID must not be empty"
        }));
        assert!(error.diagnostics.iter().any(|item| {
            item.location == "$.connectors[0].abi_type.canonical_class_path"
                && item.message == "enum canonical class path must not be empty"
        }));
        attempts.push(error);
    }
    assert_eq!(attempts[0], attempts[1]);
}

#[test]
fn allocation_is_repeatable_does_not_mutate_input_and_returns_detached_rows() {
    let huge = BigInt::from(10_u8).pow(90);
    let mut projection = direct_projection(
        vec![ScalarParameterAbiRow {
            parameter_id: "gain".to_owned(),
            coordinates: vec![coordinate("zones", "north", 0)],
            abi_type: ScalarAbiType::Alias {
                type_id: "temperature".to_owned(),
                primitive: PrimitiveType::Real,
                quantity: Some("thermodynamic_temperature".to_owned()),
                unit: Some("K".to_owned()),
                display_unit: Some("degC".to_owned()),
            },
            source: ParameterSource::Assignment,
            value: ScalarAbiValue::Integer(huge.clone()),
        }],
        vec![connector_row("signal", Vec::new())],
    );
    projection.revision = huge.clone();
    let before = projection.clone();
    let first = allocate_scalar_names(&projection).expect("first allocation succeeds");
    let second = allocate_scalar_names(&projection).expect("second allocation succeeds");
    assert_eq!(first, second);
    assert_eq!(projection, before);

    projection.canonical_id = "changed".to_owned();
    projection.revision = BigInt::from(1_u8);
    projection.parameters[0].parameter_id = "changed".to_owned();
    projection.parameters[0].coordinates[0].member_id = "changed".to_owned();
    let ScalarAbiType::Alias {
        quantity,
        display_unit,
        ..
    } = &mut projection.parameters[0].abi_type
    else {
        panic!("fixture parameter must be an alias")
    };
    *quantity = Some("changed".to_owned());
    *display_unit = None;
    projection.parameters[0].value = ScalarAbiValue::Integer(BigInt::from(0_u8));
    projection.connectors[0].connector_id = "changed".to_owned();

    assert_eq!(first.canonical_id, "G36-RUST-DIRECT-NAME-TEST");
    assert_eq!(first.revision, huge);
    assert_eq!(first.parameters[0].parameter_id, "gain");
    assert_eq!(first.parameters[0].coordinates[0].member_id, "north");
    assert_eq!(
        first.parameters[0].value,
        ScalarAbiValue::Integer(BigInt::from(10_u8).pow(90))
    );
    assert_eq!(first.connectors[0].connector_id, "signal");
    let ScalarAbiType::Alias {
        quantity,
        display_unit,
        ..
    } = &first.parameters[0].abi_type
    else {
        panic!("named parameter must remain an alias")
    };
    assert_eq!(quantity.as_deref(), Some("thermodynamic_temperature"));
    assert_eq!(display_unit.as_deref(), Some("degC"));
}

#[test]
fn checked_name_resources_fail_without_large_allocations() {
    assert_eq!(checked_encoded_name_length(2, [3_usize, 2, 4]), Some(22));
    assert_eq!(checked_encoded_name_length(2, [usize::MAX]), None);
    assert_eq!(checked_encoded_name_length(usize::MAX, [1]), None);

    let mut buffer = String::new();
    assert_eq!(
        reserve_name_buffer(&mut buffer, usize::MAX),
        Err(NameResourceFailure::AllocationFailed)
    );
    assert!(buffer.is_empty());
    let diagnostic = resource_diagnostic(
        "parameter",
        "gain",
        "$.parameters[0]",
        NameResourceFailure::LengthOverflow.message(),
    );
    assert_eq!(diagnostic.code, "resource_limit");
    assert_eq!(diagnostic.owner_kind, "parameter");
    assert_eq!(diagnostic.owner_id, "gain");
    assert_eq!(diagnostic.location, "$.parameters[0]");
    assert_eq!(diagnostic.message, "scalar name length overflows usize");
}

#[test]
fn named_output_surface_contains_only_names_and_scalar_abi_payload() {
    let result = allocate_scalar_names(&direct_projection(
        vec![parameter_row("gain", Vec::new())],
        vec![connector_row("signal", Vec::new())],
    ))
    .expect("allocation succeeds");
    let NamedScalarProjection {
        canonical_id,
        revision,
        mut parameters,
        mut connectors,
    } = result;
    let NamedScalarParameterRow {
        scalar_name: parameter_name,
        parameter_id,
        coordinates: parameter_coordinates,
        abi_type: parameter_type,
        source,
        value,
    } = parameters.remove(0);
    let NamedScalarConnectorRow {
        scalar_name: connector_name,
        connector_id,
        coordinates: connector_coordinates,
        abi_type: connector_type,
        direction,
    } = connectors.remove(0);

    assert_eq!(canonical_id, "G36-RUST-DIRECT-NAME-TEST");
    assert_eq!(revision, BigInt::from(1_u8));
    assert_eq!(parameter_name, "p_6761696e");
    assert_eq!(parameter_id, "gain");
    assert!(parameter_coordinates.is_empty());
    assert_eq!(
        parameter_type,
        ScalarAbiType::Primitive(PrimitiveType::Real)
    );
    assert_eq!(source, ParameterSource::Default);
    assert_eq!(value, ScalarAbiValue::Real(finite(1.0)));
    assert_eq!(connector_name, "c_7369676e616c");
    assert_eq!(connector_id, "signal");
    assert!(connector_coordinates.is_empty());
    assert_eq!(
        connector_type,
        ScalarAbiType::Primitive(PrimitiveType::Real)
    );
    assert_eq!(direction, ConnectorDirection::Input);
}
