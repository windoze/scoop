use super::*;
use scoop_identity::{
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedGeneratedCallableKey,
    GeneratedCallableKey, IdentityLayer, PendingIdentityValidation, PersistentGeneratedCallableId,
};
type Adapter = CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

fn graph(fixture: &Fixture, record: &Adapter) -> ValidatedIdentityGraph {
    let decoded: DecodedCborIdentityRecord<
        PersistentGeneratedCallableId,
        DecodedGeneratedCallableKey,
    > = decode_canonical(&encode(record).unwrap()).unwrap();
    let mut validation = PendingIdentityValidation::new();
    validation
        .register_external_graph_authorities(&fixture.identities)
        .unwrap();
    validation.register(IdentityLayer::Hir, &decoded).unwrap();
    validation.resolve(&decoded).unwrap();
    validation.finish().unwrap()
}

#[test]
fn constructor_adapter_identity_routes_require_both_artifact_records_and_one_graph() {
    with_hir_source(SOURCE, |output, core| {
        let fixture = Fixture::from_output(output);
        let record = reference(output, "cell", 0);
        let Constructor::Class {
            declaration: ClassId::Source(source),
            owner_type,
        } = record.target()
        else {
            panic!("source class constructor");
        };
        // Exercise the persisted identity route independently of executable
        // adapter materialization; the declared constructor supplies defaults.
        let adapter = Adapter::from_key(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
            constructor: *source,
        })
        .unwrap();
        let target = Constructor::Class {
            declaration: ClassId::Generated(adapter.id()),
            owner_type: owner_type.clone(),
        };
        let graph = graph(&fixture, &adapter);
        let foundation = fixture
            .source
            .bind_to_foundation(&fixture.foundation, &graph)
            .unwrap();
        assert!(
            matches!(foundation.default_constructor_access_subject(&target), Err(Error::MissingAdapter(id)) if id == adapter.id())
        );
        let required = BTreeSet::from([Subject::Constructor(*source)]);
        let table = Table::from_export_hir(&output.output().export, &required).unwrap();
        let access = foundation
            .bind_default_access_declarations(&table, &required)
            .unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let domains =
            hir::DefaultSourceDomainsV1::new(&access, &[], inputs.protocols().fundamental_types())
                .unwrap();
        assert!(
            matches!(domains.value_source_domain(hir::DefaultSourceValueTargetV1::Constructor(&target)), Err(hir::DefaultSourceDomainError::Target(error)) if matches!(*error, Error::MissingAdapter(id) if id == adapter.id()))
        );
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical
            .set_generated_callables(vec![adapter.clone()])
            .unwrap();
        let artifact = hir::OdrFreeHirFoundation::try_new(canonical.clone()).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&artifact, &fixture.identities)
            .unwrap();
        assert!(matches!(
            foundation.default_constructor_access_subject(&target),
            Err(Error::Foundation(
                hir::TypeFoundationBindingError::Identity(_)
            ))
        ));
        let foundation = fixture
            .source
            .bind_to_foundation(&artifact, &graph)
            .unwrap();
        assert_eq!(
            foundation
                .default_constructor_access_subject(&target)
                .unwrap(),
            Subject::Constructor(*source)
        );
        let bound = foundation
            .bind_default_access_declarations(&table, &required)
            .unwrap();
        assert_eq!(
            &bound
                .source_lookup_domain(Subject::Constructor(*source))
                .unwrap(),
            record.witness().target_domain()
        );
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let domains =
            hir::DefaultSourceDomainsV1::new(&bound, &[], inputs.protocols().fundamental_types())
                .unwrap();
        assert_eq!(
            &domains
                .value_source_domain(hir::DefaultSourceValueTargetV1::Constructor(&target))
                .unwrap(),
            record.witness().target_domain(),
        );
        canonical.set_constructors(vec![]).unwrap();
        let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&artifact, &graph)
            .unwrap();
        assert!(
            matches!(foundation.default_constructor_access_subject(&target), Err(Error::MissingDeclaration(Subject::Constructor(id))) if id == *source)
        );
    });
}

#[test]
fn default_constructor_access_rejects_other_actual_generated_callable_roles() {
    with_hir_source(SOURCE, |output, core| {
        let fixture = Fixture::from_output(output);
        let export = output.output().export.module();
        let generated = export
            .functions
            .iter()
            .find_map(|(id, _)| match &export.function_identities[id] {
                hir::HirFunctionIdentity::LexicalGenerated(record) => Some(record.id()),
                _ => None,
            })
            .unwrap();
        let record = reference(output, "cell", 0);
        let target = Constructor::Class {
            declaration: ClassId::Generated(generated),
            owner_type: record.target().owner_type().clone(),
        };
        assert!(
            matches!(fixture.bind().unwrap().default_constructor_access_subject(&target), Err(Error::AdapterRole(id)) if id == generated)
        );
        let foundation = fixture.bind().unwrap();
        let empty = Table::try_new(vec![]).unwrap();
        let bound = foundation
            .bind_default_access_declarations(&empty, &BTreeSet::new())
            .unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let domains =
            hir::DefaultSourceDomainsV1::new(&bound, &[], inputs.protocols().fundamental_types())
                .unwrap();
        let error = domains
            .value_source_domain(hir::DefaultSourceValueTargetV1::Constructor(&target))
            .unwrap_err();
        assert!(
            matches!(error, hir::DefaultSourceDomainError::Target(error) if matches!(*error, Error::AdapterRole(id) if id == generated))
        );
    });
}
