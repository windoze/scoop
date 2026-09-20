use super::*;
use hir::{InheritanceSourceParameterV1 as Parameter, ProtectedParameterCallingKindV1 as Kind};

#[test]
fn source_parameter_arity_names_and_types_must_match_bound_signatures() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let core = inputs.protocols().fundamental_types();
        for record in sources
            .protocols
            .records()
            .iter()
            .filter(|r| !r.parameters().is_empty())
        {
            let owner = record.owner();
            let first = &record.parameters()[0];
            for field in [0, 1, 2] {
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
                    _ => unreachable!(),
                }
                let mut forged = sources.clone();
                forged.replace(Record::try_new(owner, parameters, &mut meter()).unwrap());
                let error = forged.bind(&foundation, core, &mut meter()).unwrap_err();
                assert!(matches!((field, error), (0, Error::Arity(actual))
                    | (1 | 2, Error::Shape { owner: actual, position: 0 }) if actual == owner));
            }
        }
    });
}

#[test]
fn scalar_and_binder_parameters_cannot_claim_vararg_array_protocols() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
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
                let mut forged = sources.clone();
                forged.replace(Record::try_new(record.owner(), parameters, &mut meter()).unwrap());
                assert!(
                    matches!(forged.bind(&foundation, inputs.protocols().fundamental_types(), &mut meter()),
                    Err(Error::Vararg { owner, position: 0 }) if owner == record.owner())
                );
            }
        }
    });
}

#[test]
fn parameter_origins_must_resolve_actual_foundation_source_points() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let record = sources
            .protocols
            .records()
            .iter()
            .find(|r| !r.parameters().is_empty())
            .unwrap();
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
        sources.replace(Record::try_new(record.owner(), parameters, &mut meter()).unwrap());
        assert!(
            matches!(sources.bind(&foundation, inputs.protocols().fundamental_types(), &mut meter()),
            Err(Error::Foundation(hir::TypeFoundationBindingError::MissingSourcePoint(offset))) if offset == invalid_offset)
        );
    });
}
