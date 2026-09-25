use super::*;
use hir::{InheritanceSourceParameterV1 as Parameter, ProtectedParameterCallingKindV1 as Kind};

#[test]
fn complete_parameter_shapes_require_exact_arity_names_types_and_order() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            for record in sources
                .protocols
                .records()
                .iter()
                .filter(|r| !r.parameters().is_empty())
            {
                let owner = record.owner();
                let first = &record.parameters()[0];
                for field in 0..4 {
                    let mut parameters = record.parameters().to_vec();
                    match field {
                        0 => {
                            parameters.pop().unwrap();
                        }
                        1 | 2 => {
                            let shape = first.shape();
                            let name = if field == 1 {
                                scoop_identity::CanonicalIdentifier::new("renamed").unwrap()
                            } else {
                                shape.name().clone()
                            };
                            let value_type = if field == 2 {
                                SignatureTypeKey::Nominal(core.unit().persistent())
                            } else {
                                shape.value_type().clone()
                            };
                            parameters[0] = Parameter::new(
                                hir::SourceParameterShapeV1::new(name, value_type),
                                first.calling_kind(),
                                first.definition_origin().clone(),
                            );
                        }
                        3 if parameters.len() > 1 => parameters.swap(0, 1),
                        3 => continue,
                        _ => unreachable!(),
                    }
                    let forged = sources.replacing(Record::try_new(owner, parameters).unwrap());
                    let Error::Contract(error) = members
                        .bind_parameter_protocols(constructors, &forged)
                        .unwrap_err()
                    else {
                        panic!("parameter contract rejection");
                    };
                    match (field, *error) {
                        (0, ContractError::Arity(actual))
                        | (
                            1..=3,
                            ContractError::Shape {
                                owner: actual,
                                position: 0,
                            },
                        ) => assert_eq!(actual, owner),
                        (_, error) => panic!("unexpected parameter shape error: {error}"),
                    }
                }
            }
        });
    });
}

#[test]
fn complete_scalar_and_binder_parameters_cannot_claim_varargs() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            for record in sources
                .protocols
                .records()
                .iter()
                .filter(|r| !r.parameters().is_empty())
            {
                let first = &record.parameters()[0];
                for kind in [Kind::VarargEmpty, Kind::VarargDefault] {
                    let mut parameters = record.parameters().to_vec();
                    parameters[0] = Parameter::new(
                        first.shape().clone(),
                        kind,
                        first.definition_origin().clone(),
                    );
                    let forged =
                        sources.replacing(Record::try_new(record.owner(), parameters).unwrap());
                    let Error::Contract(error) = members
                        .bind_parameter_protocols(constructors, &forged)
                        .unwrap_err()
                    else {
                        panic!("vararg contract rejection");
                    };
                    let ContractError::Vararg { owner, position: 0 } = *error else {
                        panic!("canonical Array rejection");
                    };
                    assert_eq!(owner, record.owner());
                }
            }
        });
    });
}

#[test]
fn complete_parameter_origins_require_real_foundation_endpoints_for_every_role() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            for record in sources
                .protocols
                .records()
                .iter()
                .filter(|r| !r.parameters().is_empty())
            {
                let first = &record.parameters()[0];
                let origin = first.definition_origin().origin();
                let context = fixture
                    .foundation
                    .source_context_key(origin.context())
                    .unwrap();
                let invalid_offset = u64::MAX - 1;
                let invalid = scoop_identity::DefinitionOrigin::new(
                    origin.source().clone(),
                    scoop_identity::SourceSpan::new(invalid_offset, u64::MAX).unwrap(),
                    context,
                )
                .unwrap();
                let mut parameters = record.parameters().to_vec();
                parameters[0] = Parameter::new(
                    first.shape().clone(),
                    first.calling_kind(),
                    hir::ExportDefinitionSourceV1::new(invalid),
                );
                let forged =
                    sources.replacing(Record::try_new(record.owner(), parameters).unwrap());
                let Error::Contract(error) = members
                    .bind_parameter_protocols(constructors, &forged)
                    .unwrap_err()
                else {
                    panic!("foundation endpoint rejection");
                };
                let ContractError::Foundation(hir::TypeFoundationBindingError::MissingSourcePoint(
                    offset,
                )) = *error
                else {
                    panic!("missing source point");
                };
                assert_eq!(offset, invalid_offset);
            }
        });
    });
}
