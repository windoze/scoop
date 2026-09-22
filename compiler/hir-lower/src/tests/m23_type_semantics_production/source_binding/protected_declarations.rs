use super::nominal_parameters::support::Sources;
use super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    ProtectedDeclarationInterfaceV1 as Declaration,
    ProtectedDeclarationSourceProductionV1 as Production,
};
use scoop_identity::CallableTemplateOrigin;

mod binding;
mod rejection;
pub(super) mod support;
mod type_production;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/protected-declarations.scoop"
));

#[test]
fn complete_protected_production_roundtrips_and_matches_independent_sources() {
    for source in [
        SOURCE,
        "public class Plain {}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/nested-binding.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/roots.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
        )),
    ] {
        with_hir_source(source, |output, core| {
            let export = &output.output().export;
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let produced = Production::from_export_hir(export, &mut meter()).unwrap();
            let repeated = Production::from_export_hir(export, &mut meter()).unwrap();
            assert_eq!(produced.required(), repeated.required());
            assert_eq!(produced.declarations(), repeated.declarations());
            assert_eq!(produced.protocols(), repeated.protocols());
            let required = expected(&fixture, &sources);
            assert_eq!(produced.required(), &required);
            let (declarations, protocols) = restore(&mut fixture, &produced);
            let foundation = fixture.bind().unwrap();
            let core = core.foundation.import_core_inputs(&core.interface).unwrap();
            sources.with_bound(&foundation, core.protocols().fundamental_types(), |members, constructors| {
                let mut authority = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                let mut owners = BTreeSet::new();
                for record in declarations.records() {
                    match record {
                        Declaration::Callable(r) => {
                            assert_eq!(r.as_ref(), &hir::ProtectedCallableInterfaceV1::try_from(members.callable_source(r.declaration()).unwrap().clone()).unwrap());
                            if !matches!(r.declaration(), CallableTemplateOrigin::Accessor(_)) { owners.insert(r.declaration()); }
                        }
                        Declaration::Constructor(r) => {
                            assert_eq!(r.as_ref(), &hir::ProtectedConstructorInterfaceV1::try_from(constructors.constructor_source(r.declaration()).unwrap().clone()).unwrap());
                            owners.insert(CallableTemplateOrigin::Constructor(r.declaration()));
                        }
                        Declaration::Property(r) => assert_eq!(r.as_ref(), &hir::ProtectedPropertyInterfaceV1::try_from(members.property_source(r.declaration()).unwrap().clone()).unwrap()),
                        Declaration::NestedNominal(r) => {
                            let support = hir::NominalSupportNestedInterfaceV1::try_new(r.declaration(), r.declaration_access().clone(), r.payload().clone()).unwrap();
                            let checked = authority.validate_nested_source(&support, &protocols, &fixture.source.entries().representations, &mut meter()).unwrap();
                            owners.extend(checked.protocols().map(|r| r.record().owner()));
                        }
                    }
                }
                assert_eq!(protocols.records().iter().map(|r| r.owner()).collect::<BTreeSet<_>>(), owners);
                for protocol in protocols.records() {
                    authority.validate_source_protocol(protocol, &mut meter()).unwrap();
                }
                let checked = authority.validate_protected_declarations(
                    &declarations, &protocols, &fixture.source.entries().representations, &mut meter(),
                ).unwrap();
                assert_eq!(checked.provider(), fixture.source.entries().provider);
                assert!(std::ptr::eq(checked.members(), members));
                assert!(std::ptr::eq(checked.constructors(), constructors));
                assert!(std::ptr::eq(checked.parameter_sources(), &sources.protocols));
                assert!(std::ptr::eq(checked.table(), &declarations));
                assert!(std::ptr::eq(checked.representations(), &fixture.source.entries().representations));
                assert_eq!(checked.protocols().map(|r| r.record().owner()).collect::<BTreeSet<_>>(), owners);
                for protocol in protocols.records() {
                    assert_eq!(checked.protocol(protocol.owner()).unwrap().record(), protocol);
                }
                if source == SOURCE {
                    assert_eq!(outline(&foundation, &declarations), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-nominals/protected-declarations.snap")));
                }
            });
        });
    }
}
