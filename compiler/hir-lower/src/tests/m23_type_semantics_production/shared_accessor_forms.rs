use super::*;
use hir::PropertyAccessorImplementationV1 as Form;
use scoop_identity::{PropertyOwner, SourceDeclarationKey};
use scoop_wire::{decode_canonical, encode};

#[test]
fn shared_accessor_forms_preserve_actual_source_bodies_storage_constants_and_slots() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-core-layout-exports");
    for (case, expected) in [("standalone", [3, 0, 2, 0]), ("combined", [11, 1, 8, 2])] {
        let source =
            std::fs::read_to_string(directory.join(format!("shared-accessors-{case}.scoop")))
                .unwrap();
        source_dispatch::with_hir_source(&source, |output, _| {
            let interface = public_interface(output);
            let properties = interface.property_interfaces();
            let export = output.output().export.module();
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, foundation);
            let decoded: hir::DecodedCanonicalPropertyInterfacesV1 =
                decode_canonical(&encode(properties).unwrap(), DecodeLimits::default()).unwrap();
            let restored = decoded.resolve(&mut identities).unwrap();
            assert_eq!(&restored, properties);
            restored
                .validate_accessor_closure(interface.callable_interfaces())
                .unwrap();
            let actual: BTreeMap<_, _> = export
                .property_getters
                .iter()
                .map(|(id, getter)| {
                    (
                        export
                            .property_accessor_identities
                            .get_getter(id)
                            .unwrap()
                            .id(),
                        getter.implementation,
                    )
                })
                .chain(export.property_setters.iter().map(|(id, setter)| {
                    (
                        export
                            .property_accessor_identities
                            .get_setter(id)
                            .unwrap()
                            .id(),
                        setter.implementation,
                    )
                }))
                .collect();
            let mut counts = [0; 4];
            let mut rows = Vec::new();
            for property in restored.all_declarations() {
                for source in std::iter::once(property.accessors().getter_source())
                    .chain(property.accessors().setter_source())
                {
                    assert_eq!(
                        source.implementation(),
                        Form::from_source(actual[&source.accessor()])
                    );
                    counts[match source.implementation() {
                        Form::Storage => 0,
                        Form::Constant => 1,
                        Form::Body => 2,
                        Form::AbstractSlot => 3,
                    }] += 1;
                }
                let key = match property.declaration() {
                    PropertyOwner::Property(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                    PropertyOwner::ExtensionProperty(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                };
                let owner = match property.owner() {
                    hir::PublicDeclarationOwnerV1::Nominal(owner) => {
                        declaration_dump::nominal(owner, &identities)
                    }
                    other => format!("{other:?}"),
                };
                rows.push(format!(
                    "{owner}.{}: {:?} getter={:?} setter={:?} lookup={}\n",
                    declaration_dump::named(&key),
                    property.declared_visibility(),
                    property.accessors().getter_source().implementation(),
                    property
                        .accessors()
                        .setter_source()
                        .map(|source| source.implementation()),
                    restored.get(property.declaration()).is_some()
                ));
            }
            assert_eq!(counts, expected, "{case}");
            rows.sort();
            let snapshot = directory.join(format!("shared-accessors-{case}.hir.snap"));
            if std::env::var_os("SCOOP_UPDATE_SHARED_ACCESSOR_FORMS").is_some() {
                std::fs::write(&snapshot, rows.concat()).unwrap();
            }
            assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
        });
    }
}
