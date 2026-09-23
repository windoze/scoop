use super::*;
use scoop_identity::{
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedGeneratedCallableKey,
    GeneratedCallableKey, IdentityLayer, PendingIdentityValidation, PersistentGeneratedCallableId,
};
type Adapter = CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

fn add_adapter(
    output: &hir::DependencyHirOutput,
    fixture: &Fixture,
    record: &Adapter,
) -> (hir::OdrFreeHirFoundation, ValidatedIdentityGraph) {
    let decoded: DecodedCborIdentityRecord<
        PersistentGeneratedCallableId,
        DecodedGeneratedCallableKey,
    > = decode_canonical(&encode(record).unwrap(), DecodeLimits::default()).unwrap();
    let mut validation = PendingIdentityValidation::new();
    validation
        .register_external_graph_authorities(&fixture.identities)
        .unwrap();
    validation.register(IdentityLayer::Hir, &decoded).unwrap();
    validation.resolve(&decoded).unwrap();
    let graph = validation.finish().unwrap();
    let mut canonical = fixture.foundation.as_canonical().clone();
    let export = output.output().export.module();
    let mut records = export
        .class_constructors
        .iter()
        .filter_map(|(id, _)| {
            export.constructor_identities[id]
                .generated_record()
                .cloned()
        })
        .collect::<Vec<_>>();
    assert_eq!(canonical.counts().generated_callables, records.len());
    records.push(record.clone());
    canonical.set_generated_callables(records).unwrap();
    (
        hir::OdrFreeHirFoundation::try_new(canonical).unwrap(),
        graph,
    )
}

#[test]
fn adapter_signature_requires_artifact_identity_and_complete_defaultable_parameters() {
    with_core_sources(SOURCE, |output, fixture, sources, core| {
        for name in ["cell", "empty", "required", "variadic", "defaultVariadic"] {
            let reference = constructor(&template(output, name, u32::from(name == "variadic")));
            let hir::DefaultConstructorRefV1::Class {
                declaration: hir::DefaultClassConstructorIdV1::Source(constructor),
                owner_type,
            } = reference
            else {
                panic!("source constructor");
            };
            // The fixture owns the constructor. These persisted keys exercise
            // signature replay without claiming an executable adapter body.
            let record = Adapter::from_key(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor,
            })
            .unwrap();
            let reference = hir::DefaultConstructorRefV1::Class {
                declaration: hir::DefaultClassConstructorIdV1::Generated(record.id()),
                owner_type: owner_type.clone(),
            };
            let (artifact, graph) = add_adapter(output, fixture, &record);
            let missing = fixture
                .source
                .bind_to_foundation(&fixture.foundation, &graph, &mut meter())
                .unwrap();
            sources.with_bound(&missing, core, |members, constructors| {
                let bound = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                assert!(matches!(bound.default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root()), Err(Error::Target(error)) if matches!(*error, hir::DefaultSourceTargetSubjectError::MissingAdapter(id) if id == record.id())));
            });
            let foundation = fixture
                .source
                .bind_to_foundation(&artifact, &graph, &mut meter())
                .unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let bound = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                let result = bound.default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root());
                if name == "required" {
                    assert!(matches!(result, Err(Error::AdapterRequiredParameter { constructor: actual, position: 0 }) if actual == constructor));
                } else {
                    assert_eq!(result.unwrap(), Shape::Constructor { owner_type, parameters: vec![] });
                }
            });
        }
    });
}

#[test]
fn produced_adapter_is_a_constructor_and_never_a_member_callable() {
    with_core_source(SOURCE, |output, bound, _| {
        let export = output.output().export.module();
        let record = export
            .class_constructors
            .iter()
            .find_map(|(id, _)| export.constructor_identities[id].generated_record())
            .expect("produced zero-argument adapter");
        let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } = record.key()
        else {
            panic!("constructor adapter role");
        };
        let source = bound
            .constructors()
            .constructor_source(*constructor)
            .unwrap();
        let owner_type = source.payload().result().clone();
        let id = record.id();
        let reference = hir::DefaultConstructorRefV1::Class {
            declaration: hir::DefaultClassConstructorIdV1::Generated(id),
            owner_type: owner_type.clone(),
        };
        assert_eq!(
            bound
                .default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root())
                .unwrap(),
            Shape::Constructor {
                owner_type,
                parameters: vec![]
            }
        );
        let member = hir::DefaultCallableRefV1::try_new(
            hir::DefaultCallableDeclarationV1::Generated(id),
            OptionalSignatureType::from_option(Some(reference.owner_type().clone())),
            vec![],
        )
        .unwrap();
        assert!(
            matches!(bound.members().default_member_callable_shape(&member, &mut meter(), &WirePath::root()), Err(Error::Declaration(hir::DefaultCallableDeclarationV1::Generated(actual))) if actual == id)
        );
    });
}
