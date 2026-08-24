use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

fn finite(value: f64) -> FiniteReal {
    FiniteReal::new(value).expect("test real is finite")
}

fn real(value: f64) -> ScalarValue {
    ScalarValue::Real(finite(value))
}

fn integer(value: impl Into<BigInt>) -> ScalarValue {
    ScalarValue::Integer(value.into())
}

fn enum_value(type_id: &str, member_id: &str) -> ScalarValue {
    ScalarValue::Enum(EnumInputValue {
        type_id: type_id.to_owned(),
        member_id: member_id.to_owned(),
    })
}

fn parameter_operand(parameter_id: &str) -> GuardOperand {
    GuardOperand::Parameter(parameter_id.to_owned())
}

fn literal(type_use: TypeUse, value: ScalarValue) -> GuardOperand {
    GuardOperand::Literal { type_use, value }
}

fn comparison(operator: ComparisonOperator, left: GuardOperand, right: GuardOperand) -> Guard {
    Guard::Compare {
        operator,
        left,
        right,
    }
}

fn bool_comparison(parameter_id: &str, value: bool) -> Guard {
    comparison(
        ComparisonOperator::Eq,
        parameter_operand(parameter_id),
        literal(
            TypeUse::Primitive(PrimitiveType::Boolean),
            ScalarValue::Boolean(value),
        ),
    )
}

fn fixture() -> ValidatedResolutionInput {
    ValidatedResolutionInput {
        canonical_id: "G36-05-16-SYNTHETIC-SCHEMA-TEST".to_owned(),
        revision: BigInt::from(1_u8),
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
                type_id: "air_flow".to_owned(),
                definition: NamedTypeDefinition::Alias {
                    primitive: PrimitiveType::Real,
                    quantity: Some("volume_flow_rate".to_owned()),
                    unit: Some("m3/s".to_owned()),
                    display_unit: None,
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
                kind: DimensionKind::ParameterDriven {
                    parameter_id: "zone_count".to_owned(),
                    members: vec![
                        "north-zone".to_owned(),
                        "south-zone".to_owned(),
                        "core-zone".to_owned(),
                    ],
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
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank1 {
                    dimension_id: "fixed_pair".to_owned(),
                },
                source: ParameterSource::Default,
                value: ParameterValue::Rank1(vec![real(1.0), real(0.5)]),
            },
            ParameterDefinition {
                parameter_id: "zone_count".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Integer),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(integer(3_u8)),
            },
            ParameterDefinition {
                parameter_id: "enable_trim".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(ScalarValue::Boolean(false)),
            },
            ParameterDefinition {
                parameter_id: "initial_mode".to_owned(),
                type_use: TypeUse::Named("operating_mode".to_owned()),
                shape: Shape::Scalar,
                source: ParameterSource::Assignment,
                value: ParameterValue::Scalar(enum_value("operating_mode", "warm-up")),
            },
            ParameterDefinition {
                parameter_id: "optional_gain".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Scalar,
                source: ParameterSource::Default,
                value: ParameterValue::Scalar(real(1.0)),
            },
            ParameterDefinition {
                parameter_id: "zone_offsets".to_owned(),
                type_use: TypeUse::Named("temperature".to_owned()),
                shape: Shape::Rank1 {
                    dimension_id: "zones".to_owned(),
                },
                source: ParameterSource::Assignment,
                value: ParameterValue::Rank1(vec![real(0.0), real(0.5), real(-0.5)]),
            },
            ParameterDefinition {
                parameter_id: "matrix_weights".to_owned(),
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank2 {
                    first_dimension_id: "zones".to_owned(),
                    second_dimension_id: "fixed_pair".to_owned(),
                },
                source: ParameterSource::Assignment,
                value: ParameterValue::Rank2(vec![
                    vec![real(1.0), real(0.0)],
                    vec![real(0.5), real(0.5)],
                    vec![real(0.0), real(1.0)],
                ]),
            },
        ],
        connectors: vec![
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
                connector_id: "supply_air_flow".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Named("air_flow".to_owned()),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Always,
            },
            ConnectorDefinition {
                connector_id: "trim_request".to_owned(),
                direction: ConnectorDirection::Input,
                type_use: TypeUse::Primitive(PrimitiveType::Boolean),
                shape: Shape::Scalar,
                presence: ConnectorPresence::Guarded(Guard::And(vec![
                    bool_comparison("enable_trim", true),
                    Guard::Or(vec![
                        comparison(
                            ComparisonOperator::Gt,
                            parameter_operand("zone_count"),
                            literal(TypeUse::Primitive(PrimitiveType::Integer), integer(1_u8)),
                        ),
                        Guard::Not(Box::new(comparison(
                            ComparisonOperator::Eq,
                            parameter_operand("initial_mode"),
                            literal(
                                TypeUse::Named("operating_mode".to_owned()),
                                enum_value("operating_mode", "unoccupied"),
                            ),
                        ))),
                    ]),
                ])),
            },
            ConnectorDefinition {
                connector_id: "zone_commands".to_owned(),
                direction: ConnectorDirection::Output,
                type_use: TypeUse::Primitive(PrimitiveType::Real),
                shape: Shape::Rank1 {
                    dimension_id: "zones".to_owned(),
                },
                presence: ConnectorPresence::Always,
            },
        ],
    }
}

fn parameter<'a>(result: &'a ResolvedSpecialization, parameter_id: &str) -> &'a ResolvedParameter {
    result
        .parameters
        .iter()
        .find(|parameter| parameter.parameter_id == parameter_id)
        .expect("resolved parameter exists")
}

fn connector<'a>(result: &'a ResolvedSpecialization, connector_id: &str) -> &'a ResolvedConnector {
    result
        .connectors
        .iter()
        .find(|connector| connector.connector_id == connector_id)
        .expect("resolved connector exists")
}

fn parameter_mut<'a>(
    input: &'a mut ValidatedResolutionInput,
    parameter_id: &str,
) -> &'a mut ParameterDefinition {
    input
        .parameters
        .iter_mut()
        .find(|parameter| parameter.parameter_id == parameter_id)
        .expect("input parameter exists")
}

fn connector_mut<'a>(
    input: &'a mut ValidatedResolutionInput,
    connector_id: &str,
) -> &'a mut ConnectorDefinition {
    input
        .connectors
        .iter_mut()
        .find(|connector| connector.connector_id == connector_id)
        .expect("input connector exists")
}

fn resolve(input: &ValidatedResolutionInput) -> ResolvedSpecialization {
    resolve_validated(input, ResolutionLimits::default()).expect("fixture resolves")
}

fn assert_invalid(input: ValidatedResolutionInput) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        resolve_validated(&input, ResolutionLimits::default())
    }));
    let result = outcome.expect("malformed typed input must not panic");
    assert!(
        matches!(result, Err(ResolutionError::InvalidInput { .. })),
        "expected InvalidInput, got {result:?}"
    );
}

fn guarded_result(guard: Guard) -> bool {
    let mut input = fixture();
    connector_mut(&mut input, "trim_request").presence = ConnectorPresence::Guarded(guard);
    connector(&resolve(&input), "trim_request").active
}

#[test]
fn baseline_identity_dimensions_parameters_and_types_are_exact() {
    let result = resolve(&fixture());
    assert_eq!(result.canonical_id, "G36-05-16-SYNTHETIC-SCHEMA-TEST");
    assert_eq!(result.revision, BigInt::from(1_u8));
    assert_eq!(
        result.dimensions,
        vec![
            ResolvedDimension {
                dimension_id: "fixed_pair".to_owned(),
                kind: ResolvedDimensionKind::Fixed,
                extent: 2,
                members: vec!["primary".to_owned(), "secondary".to_owned()],
            },
            ResolvedDimension {
                dimension_id: "zones".to_owned(),
                kind: ResolvedDimensionKind::Parameter,
                extent: 3,
                members: vec![
                    "north-zone".to_owned(),
                    "south-zone".to_owned(),
                    "core-zone".to_owned(),
                ],
            },
        ]
    );
    assert_eq!(
        result
            .parameters
            .iter()
            .map(|parameter| (parameter.parameter_id.as_str(), parameter.source))
            .collect::<Vec<_>>(),
        vec![
            ("sample_period_s", ParameterSource::Default),
            ("fixed_gains", ParameterSource::Default),
            ("zone_count", ParameterSource::Assignment),
            ("enable_trim", ParameterSource::Assignment),
            ("initial_mode", ParameterSource::Assignment),
            ("optional_gain", ParameterSource::Default),
            ("zone_offsets", ParameterSource::Assignment),
            ("matrix_weights", ParameterSource::Assignment),
        ]
    );

    let sample_period = parameter(&result, "sample_period_s");
    assert_eq!(
        sample_period.resolved_type,
        ResolvedType::Primitive(PrimitiveType::Real)
    );
    assert!(sample_period.dimension_ids.is_empty());
    assert_eq!(
        sample_period.leaves,
        vec![ScalarParameterLeaf {
            coordinates: Vec::new(),
            value: ResolvedScalarValue::Real(finite(60.0)),
        }]
    );

    assert_eq!(
        parameter(&result, "zone_offsets").resolved_type,
        ResolvedType::Alias {
            type_id: "temperature".to_owned(),
            primitive: PrimitiveType::Real,
            quantity: Some("thermodynamic_temperature".to_owned()),
            unit: Some("K".to_owned()),
            display_unit: Some("degC".to_owned()),
        }
    );
    assert_eq!(
        parameter(&result, "initial_mode").resolved_type,
        ResolvedType::Enum {
            type_id: "operating_mode".to_owned(),
            members: vec![
                ResolvedEnumMember {
                    member_id: "occupied".to_owned(),
                    symbol: "OCCUPIED".to_owned(),
                },
                ResolvedEnumMember {
                    member_id: "warm-up".to_owned(),
                    symbol: "WARM_UP".to_owned(),
                },
                ResolvedEnumMember {
                    member_id: "unoccupied".to_owned(),
                    symbol: "UNOCCUPIED".to_owned(),
                },
            ],
        }
    );
    assert_eq!(
        parameter(&result, "initial_mode").leaves[0].value,
        ResolvedScalarValue::Enum(ResolvedEnumValue {
            type_id: "operating_mode".to_owned(),
            member_id: "warm-up".to_owned(),
            symbol: "WARM_UP".to_owned(),
        })
    );
}

#[test]
fn rank_one_and_rank_two_leaves_are_row_major_with_stable_coordinates() {
    let result = resolve(&fixture());
    let fixed_gains = parameter(&result, "fixed_gains");
    assert_eq!(fixed_gains.dimension_ids, ["fixed_pair"]);
    assert_eq!(
        fixed_gains
            .leaves
            .iter()
            .map(|leaf| {
                let coordinate = &leaf.coordinates[0];
                (
                    coordinate.dimension_id.as_str(),
                    coordinate.member_id.as_str(),
                    coordinate.ordinal,
                    leaf.value.clone(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "fixed_pair",
                "primary",
                0,
                ResolvedScalarValue::Real(finite(1.0)),
            ),
            (
                "fixed_pair",
                "secondary",
                1,
                ResolvedScalarValue::Real(finite(0.5)),
            ),
        ]
    );

    let matrix = parameter(&result, "matrix_weights");
    assert_eq!(matrix.dimension_ids, ["zones", "fixed_pair"]);
    assert_eq!(
        matrix
            .leaves
            .iter()
            .map(|leaf| (
                leaf.coordinates
                    .iter()
                    .map(|coordinate| coordinate.member_id.as_str())
                    .collect::<Vec<_>>(),
                leaf.value.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                vec!["north-zone", "primary"],
                ResolvedScalarValue::Real(finite(1.0)),
            ),
            (
                vec!["north-zone", "secondary"],
                ResolvedScalarValue::Real(finite(0.0)),
            ),
            (
                vec!["south-zone", "primary"],
                ResolvedScalarValue::Real(finite(0.5)),
            ),
            (
                vec!["south-zone", "secondary"],
                ResolvedScalarValue::Real(finite(0.5)),
            ),
            (
                vec!["core-zone", "primary"],
                ResolvedScalarValue::Real(finite(0.0)),
            ),
            (
                vec!["core-zone", "secondary"],
                ResolvedScalarValue::Real(finite(1.0)),
            ),
        ]
    );
}

#[test]
fn connectors_preserve_order_presence_dimensions_and_leaves() {
    let result = resolve(&fixture());
    assert_eq!(
        result
            .connectors
            .iter()
            .map(|connector| connector.connector_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "zone_temperatures",
            "supply_air_flow",
            "trim_request",
            "zone_commands",
        ]
    );
    let trim = connector(&result, "trim_request");
    assert!(!trim.active);
    assert_eq!(trim.guard_result, Some(false));
    assert!(trim.leaves.is_empty());

    let temperatures = connector(&result, "zone_temperatures");
    assert!(temperatures.active);
    assert_eq!(temperatures.guard_result, None);
    assert_eq!(temperatures.dimension_ids, ["zones"]);
    assert_eq!(
        temperatures.resolved_type,
        ResolvedType::Alias {
            type_id: "temperature".to_owned(),
            primitive: PrimitiveType::Real,
            quantity: Some("thermodynamic_temperature".to_owned()),
            unit: Some("K".to_owned()),
            display_unit: Some("degC".to_owned()),
        }
    );
    assert_eq!(
        temperatures
            .leaves
            .iter()
            .map(|leaf| {
                let coordinate = &leaf.coordinates[0];
                (coordinate.member_id.as_str(), coordinate.ordinal)
            })
            .collect::<Vec<_>>(),
        vec![("north-zone", 0), ("south-zone", 1), ("core-zone", 2)]
    );
    assert_eq!(connector(&result, "supply_air_flow").leaves.len(), 1);

    let mut input = fixture();
    let inactive_guard = match &connector_mut(&mut input, "trim_request").presence {
        ConnectorPresence::Guarded(guard) => guard.clone(),
        ConnectorPresence::Always => panic!("fixture trim connector is guarded"),
    };
    connector_mut(&mut input, "zone_commands").presence =
        ConnectorPresence::Guarded(inactive_guard);
    let result = resolve(&input);
    let commands = connector(&result, "zone_commands");
    assert!(!commands.active);
    assert_eq!(commands.guard_result, Some(false));
    assert_eq!(commands.dimension_ids, ["zones"]);
    assert!(commands.leaves.is_empty());
}

#[test]
fn every_guard_operator_and_enum_equality_is_evaluated() {
    let numeric_cases = [
        (ComparisonOperator::Eq, 3.0),
        (ComparisonOperator::Ne, 4.0),
        (ComparisonOperator::Lt, 4.0),
        (ComparisonOperator::Lte, 3.0),
        (ComparisonOperator::Gt, 2.0),
        (ComparisonOperator::Gte, 3.0),
    ];
    for (operator, right) in numeric_cases {
        assert!(guarded_result(comparison(
            operator,
            parameter_operand("zone_count"),
            literal(TypeUse::Primitive(PrimitiveType::Real), real(right)),
        )));
    }

    assert!(guarded_result(comparison(
        ComparisonOperator::Eq,
        parameter_operand("initial_mode"),
        literal(
            TypeUse::Named("operating_mode".to_owned()),
            enum_value("operating_mode", "warm-up"),
        ),
    )));
    assert!(guarded_result(Guard::And(vec![
        bool_comparison("enable_trim", false),
        comparison(
            ComparisonOperator::Gt,
            parameter_operand("zone_count"),
            literal(TypeUse::Primitive(PrimitiveType::Integer), integer(1_u8)),
        ),
    ])));
    assert!(guarded_result(Guard::Or(vec![
        bool_comparison("enable_trim", true),
        bool_comparison("enable_trim", false),
    ])));
    assert!(guarded_result(Guard::Not(Box::new(bool_comparison(
        "enable_trim",
        true,
    )))));
}

#[test]
fn guards_short_circuit_and_connector_state_is_reused() {
    let mut input = fixture();
    connector_mut(&mut input, "trim_request").presence =
        ConnectorPresence::Guarded(Guard::And(vec![
            bool_comparison("enable_trim", true),
            comparison(
                ComparisonOperator::Gt,
                parameter_operand("zone_count"),
                literal(TypeUse::Primitive(PrimitiveType::Integer), integer(1_u8)),
            ),
        ]));
    let validated = validate_input(&input).expect("fixture validates");
    let evaluation = connector_states(&validated).expect("guards evaluate");
    assert_eq!(evaluation.counts.guard_roots, 1);
    assert_eq!(evaluation.counts.comparisons, 1);
    assert!(!evaluation.states[2].active);
    assert_eq!(
        preflight_scalar_leaves(&validated, &evaluation.states, 100_000),
        Ok(23)
    );
    assert_eq!(evaluation.counts.guard_roots, 1);
    assert_eq!(evaluation.counts.comparisons, 1);
}

#[test]
fn authored_member_order_remaps_values_without_sorting() {
    let mut input = fixture();
    let DimensionKind::Fixed { members } = &mut input.dimensions[0].kind else {
        panic!("fixture first dimension is fixed");
    };
    members.reverse();
    let result = resolve(&input);
    assert_eq!(
        parameter(&result, "fixed_gains")
            .leaves
            .iter()
            .map(|leaf| (leaf.coordinates[0].member_id.as_str(), leaf.value.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("secondary", ResolvedScalarValue::Real(finite(1.0)),),
            ("primary", ResolvedScalarValue::Real(finite(0.5))),
        ]
    );

    let mut input = fixture();
    let DimensionKind::ParameterDriven { members, .. } = &mut input.dimensions[1].kind else {
        panic!("fixture second dimension is parameter-driven");
    };
    members.swap(0, 1);
    let result = resolve(&input);
    assert_eq!(
        parameter(&result, "zone_offsets")
            .leaves
            .iter()
            .map(|leaf| (leaf.coordinates[0].member_id.as_str(), leaf.value.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("south-zone", ResolvedScalarValue::Real(finite(0.0)),),
            ("north-zone", ResolvedScalarValue::Real(finite(0.5)),),
            ("core-zone", ResolvedScalarValue::Real(finite(-0.5))),
        ]
    );
}

#[test]
fn resolution_is_repeatable_detached_and_does_not_mutate_input() {
    let mut input = fixture();
    let before = input.clone();
    let first = resolve(&input);
    let second = resolve(&input);
    assert_eq!(first, second);
    assert_eq!(input, before);

    let NamedTypeDefinition::Enum { members } = &mut input.types[2].definition else {
        panic!("fixture named type is enum");
    };
    members[0].symbol = "CHANGED".to_owned();
    let DimensionKind::Fixed { members } = &mut input.dimensions[0].kind else {
        panic!("fixture dimension is fixed");
    };
    members[0] = "changed".to_owned();
    assert_eq!(
        match &parameter(&first, "initial_mode").resolved_type {
            ResolvedType::Enum { members, .. } => members[0].symbol.as_str(),
            _ => panic!("initial mode resolves to enum"),
        },
        "OCCUPIED"
    );
    assert_eq!(first.dimensions[0].members[0], "primary");
}

#[test]
fn guard_depth_and_aggregate_node_limits_preflight() {
    let mut input = fixture();
    let mut guard = bool_comparison("enable_trim", false);
    for _ in 0..100 {
        guard = Guard::Not(Box::new(guard));
    }
    connector_mut(&mut input, "trim_request").presence = ConnectorPresence::Guarded(guard);
    let error = resolve_validated(
        &input,
        ResolutionLimits {
            max_guard_depth: 4,
            ..ResolutionLimits::default()
        },
    )
    .expect_err("deep guard must fail");
    assert!(matches!(error, ResolutionError::ResourceLimit { .. }));
    assert!(error.to_string().contains("guard depth 5 exceeds limit 4"));

    let mut input = fixture();
    connector_mut(&mut input, "trim_request").presence = ConnectorPresence::Guarded(Guard::And(
        (0..10)
            .map(|_| bool_comparison("enable_trim", false))
            .collect(),
    ));
    let error = resolve_validated(
        &input,
        ResolutionLimits {
            max_guard_nodes: 5,
            ..ResolutionLimits::default()
        },
    )
    .expect_err("aggregate node limit must fail");
    assert!(matches!(error, ResolutionError::ResourceLimit { .. }));
    assert!(
        error
            .to_string()
            .contains("guard node count exceeds limit 5")
    );
}

#[test]
fn scalar_limit_and_checked_count_overflow_are_resource_failures() {
    let error = resolve_validated(
        &fixture(),
        ResolutionLimits {
            max_scalar_leaves: 22,
            ..ResolutionLimits::default()
        },
    )
    .expect_err("baseline expands to 23 leaves");
    assert!(matches!(error, ResolutionError::ResourceLimit { .. }));
    assert!(
        error
            .to_string()
            .contains("scalar leaf expansion 23 exceeds limit 22")
    );

    assert!(matches!(
        checked_leaf_product(usize::MAX, 2),
        Err(ResolutionError::ResourceLimit { .. })
    ));
}

#[test]
fn mixed_numeric_guards_are_exact_and_preserve_numeric_variants() {
    let two_pow_53 = BigInt::parse_bytes(b"9007199254740992", 10).expect("integer parses");
    let two_pow_53_plus_one = BigInt::parse_bytes(b"9007199254740993", 10).expect("integer parses");
    assert!(guarded_result(comparison(
        ComparisonOperator::Eq,
        literal(
            TypeUse::Primitive(PrimitiveType::Integer),
            ScalarValue::Integer(two_pow_53.clone()),
        ),
        literal(
            TypeUse::Primitive(PrimitiveType::Real),
            real(9_007_199_254_740_992.0),
        ),
    )));
    assert!(!guarded_result(comparison(
        ComparisonOperator::Eq,
        literal(
            TypeUse::Primitive(PrimitiveType::Integer),
            ScalarValue::Integer(two_pow_53_plus_one.clone()),
        ),
        literal(
            TypeUse::Primitive(PrimitiveType::Real),
            real(9_007_199_254_740_992.0),
        ),
    )));
    assert!(guarded_result(comparison(
        ComparisonOperator::Gt,
        literal(
            TypeUse::Primitive(PrimitiveType::Integer),
            ScalarValue::Integer(two_pow_53_plus_one),
        ),
        literal(
            TypeUse::Primitive(PrimitiveType::Real),
            real(9_007_199_254_740_992.0),
        ),
    )));

    let ten_pow_400 = BigInt::from(10_u8).pow(400);
    assert!(guarded_result(comparison(
        ComparisonOperator::Gt,
        literal(
            TypeUse::Primitive(PrimitiveType::Integer),
            ScalarValue::Integer(ten_pow_400.clone()),
        ),
        literal(TypeUse::Primitive(PrimitiveType::Real), real(f64::MAX)),
    )));
    assert!(guarded_result(comparison(
        ComparisonOperator::Lt,
        literal(TypeUse::Primitive(PrimitiveType::Integer), integer(-3_i8)),
        literal(TypeUse::Primitive(PrimitiveType::Real), real(-2.5)),
    )));
    assert!(guarded_result(comparison(
        ComparisonOperator::Eq,
        literal(TypeUse::Primitive(PrimitiveType::Integer), integer(0_u8)),
        literal(TypeUse::Primitive(PrimitiveType::Real), real(-0.0)),
    )));

    let mut input = fixture();
    parameter_mut(&mut input, "sample_period_s").value =
        ParameterValue::Scalar(ScalarValue::Integer(ten_pow_400.clone()));
    parameter_mut(&mut input, "optional_gain").value = ParameterValue::Scalar(real(-0.0));
    input.revision = ten_pow_400.clone();
    let result = resolve(&input);
    assert_eq!(result.revision, ten_pow_400.clone());
    assert_eq!(
        parameter(&result, "sample_period_s").leaves[0].value,
        ResolvedScalarValue::Integer(ten_pow_400)
    );
    let ResolvedScalarValue::Real(negative_zero) =
        parameter(&result, "optional_gain").leaves[0].value
    else {
        panic!("negative zero remains a real")
    };
    assert_eq!(negative_zero.get().to_bits(), (-0.0_f64).to_bits());
    assert!(negative_zero.get().is_sign_negative());
}

#[test]
fn malformed_typed_inputs_return_invalid_input_without_panicking() {
    let mut input = fixture();
    input.types.push(input.types[0].clone());
    assert_invalid(input);

    let mut input = fixture();
    input.dimensions.push(input.dimensions[0].clone());
    assert_invalid(input);

    let mut input = fixture();
    input.parameters.push(input.parameters[0].clone());
    assert_invalid(input);

    let mut input = fixture();
    input.connectors.push(input.connectors[0].clone());
    assert_invalid(input);

    let mut input = fixture();
    let NamedTypeDefinition::Enum { members } = &mut input.types[2].definition else {
        panic!("fixture named type is enum");
    };
    members.push(members[0].clone());
    assert_invalid(input);

    let mut input = fixture();
    let DimensionKind::Fixed { members } = &mut input.dimensions[0].kind else {
        panic!("fixture dimension is fixed");
    };
    members[1] = members[0].clone();
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "sample_period_s").type_use =
        TypeUse::Named("missing_type".to_owned());
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "fixed_gains").shape = Shape::Rank1 {
        dimension_id: "missing_dimension".to_owned(),
    };
    assert_invalid(input);

    let mut input = fixture();
    let DimensionKind::ParameterDriven { parameter_id, .. } = &mut input.dimensions[1].kind else {
        panic!("fixture dimension is parameter-driven");
    };
    *parameter_id = "missing_parameter".to_owned();
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "fixed_gains").value = ParameterValue::Scalar(real(1.0));
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "fixed_gains").value = ParameterValue::Rank1(vec![real(1.0)]);
    assert_invalid(input);

    let mut input = fixture();
    let ParameterValue::Rank2(rows) = &mut parameter_mut(&mut input, "matrix_weights").value else {
        panic!("fixture matrix value is rank two");
    };
    rows[1].pop();
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "initial_mode").value =
        ParameterValue::Scalar(enum_value("operating_mode", "missing-member"));
    assert_invalid(input);

    let mut input = fixture();
    parameter_mut(&mut input, "initial_mode").value =
        ParameterValue::Scalar(enum_value("other_enum", "warm-up"));
    assert_invalid(input);

    let mut input = fixture();
    connector_mut(&mut input, "trim_request").presence = ConnectorPresence::Guarded(comparison(
        ComparisonOperator::Eq,
        parameter_operand("missing_parameter"),
        literal(
            TypeUse::Primitive(PrimitiveType::Boolean),
            ScalarValue::Boolean(true),
        ),
    ));
    assert_invalid(input);

    let mut input = fixture();
    connector_mut(&mut input, "trim_request").presence = ConnectorPresence::Guarded(comparison(
        ComparisonOperator::Lt,
        parameter_operand("enable_trim"),
        literal(
            TypeUse::Primitive(PrimitiveType::Boolean),
            ScalarValue::Boolean(true),
        ),
    ));
    assert_invalid(input);
}

#[test]
fn finite_real_construction_rejects_nonfinite_values_without_panicking() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let outcome = catch_unwind(|| FiniteReal::new(value));
        let result = outcome.expect("nonfinite construction must not panic");
        assert!(matches!(result, Err(ResolutionError::InvalidInput { .. })));
    }
}

#[test]
fn resolved_type_surface_contains_only_resolution_concepts() {
    let result = resolve(&fixture());
    let ResolvedSpecialization {
        canonical_id: _,
        revision: _,
        dimensions,
        parameters,
        connectors,
    } = result;
    for dimension in dimensions {
        let ResolvedDimension {
            dimension_id: _,
            kind: _,
            extent: _,
            members: _,
        } = dimension;
    }
    for parameter in parameters {
        let ResolvedParameter {
            parameter_id: _,
            resolved_type,
            dimension_ids: _,
            source: _,
            leaves,
        } = parameter;
        match resolved_type {
            ResolvedType::Primitive(_) => {}
            ResolvedType::Alias {
                type_id: _,
                primitive: _,
                quantity: _,
                unit: _,
                display_unit: _,
            } => {}
            ResolvedType::Enum {
                type_id: _,
                members: _,
            } => {}
        }
        for leaf in leaves {
            let ScalarParameterLeaf {
                coordinates: _,
                value: _,
            } = leaf;
        }
    }
    for connector in connectors {
        let ResolvedConnector {
            connector_id: _,
            direction: _,
            resolved_type: _,
            dimension_ids: _,
            active: _,
            guard_result: _,
            leaves,
        } = connector;
        for leaf in leaves {
            let ScalarConnectorLeaf { coordinates: _ } = leaf;
        }
    }
}
