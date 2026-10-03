use super::*;
use scoop_identity::{ExactTypeKey, SourceDeclarationKey};

#[test]
fn generic_fields_keep_each_application_and_reject_a_different_owner() {
    let source = "public enum Parcel<T> { Item(T) }\n\
                  public fun first(): Parcel<Int> = Parcel.Item(1)\n\
                  public fun second(): Parcel<Long> = Parcel.Item(2L)\n";
    with_hir_source(source, |output, core| {
        let interface = public_interface(output);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
            )
            .unwrap();
        let local = output.output().local.module();
        let mut fields = BTreeSet::new();
        let mut owners = BTreeSet::new();
        let mut values = BTreeSet::new();
        for site in interface
            .external_references()
            .records()
            .iter()
            .flat_map(|reference| reference.type_sites().records())
        {
            let Site::EnumVariantFieldStorage {
                owner,
                field,
                exact,
            } = site
            else {
                continue;
            };
            fields.insert(*field);
            owners.insert(*owner);
            values.insert(*exact);
            foundation
                .validate_declaration_type_position(
                    local.cone,
                    site.position(),
                    &[core.source_foundation.as_ref()],
                )
                .unwrap();
            assert!(
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        Position::EnumVariantFieldStorage(*exact, *field),
                        &[],
                    )
                    .is_err()
            );
        }
        assert_eq!(fields.len(), 1);
        assert_eq!(owners.len(), 2);
        assert_eq!(values.len(), 2);
    });
}

#[test]
fn storage_and_generated_type_sites_cover_actual_materialized_dependencies() {
    for name in [
        "storage-standalone",
        "storage-combined",
        "declaration-combined",
    ] {
        with_hir_source(&source(name), |output, core| {
            let local = &output.output().local;
            let interface = public_interface(output);
            let mut canonical =
                hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            canonical
                .complete_cross_cone_interface_source_points(
                    output.output().export.module(),
                    &interface,
                )
                .unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, canonical.clone());
            let foundation = canonical;
            let mut counts = [0; 4];
            for site in interface
                .external_references()
                .records()
                .iter()
                .flat_map(|r| r.type_sites().records())
            {
                let index = match site {
                    Site::FieldStorage { .. } => 0,
                    Site::EnumVariantFieldStorage { .. } => 1,
                    Site::ConstructorInitializerResult { .. } => 2,
                    Site::InitializationCycleMessage { .. } => 3,
                    _ => continue,
                };
                let current = match site {
                    Site::FieldStorage { owner, .. }
                    | Site::EnumVariantFieldStorage { owner, .. } => {
                        let key = identities.canonical_key::<_, ExactTypeKey>(*owner).unwrap();
                        match key.as_ref() {
                            ExactTypeKey::Nominal(id) => {
                                identities
                                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*id)
                                    .unwrap()
                                    .origin()
                                    == local.cone
                            }
                            ExactTypeKey::NominalApplication { origin, .. } => {
                                identities
                                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(
                                        *origin,
                                    )
                                    .unwrap()
                                    .origin()
                                    == local.cone
                            }
                            _ => panic!("source storage belongs to a nominal type"),
                        }
                    }
                    _ => true,
                };
                counts[index] += usize::from(current);
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        site.position(),
                        &[core.source_foundation.as_ref()],
                    )
                    .unwrap_or_else(|error| match site {
                        Site::FieldStorage { owner, field, .. } => panic!(
                            "{name}: {error:?}; owner={:?}; field={:?}",
                            identities.canonical_key::<_, ExactTypeKey>(*owner),
                            identities.canonical_key::<_, scoop_identity::FieldIdentityKey>(*field),
                        ),
                        _ => panic!("{name}: {error:?}"),
                    });
                if current {
                    assert!(
                        foundation
                            .validate_declaration_type_position(
                                ConeIdentity::CORE,
                                site.position(),
                                &[core.source_foundation.as_ref()],
                            )
                            .is_err()
                    );
                }
                let bytes = scoop_wire::encode(site).unwrap();
                let raw: hir::DecodedHirDependencyTypeSiteV1 =
                    scoop_wire::decode_canonical(&bytes).unwrap();
                assert_eq!(raw.resolve(&mut identities).unwrap(), *site);
            }
            if name == "storage-standalone" {
                assert_eq!(counts, [0, 0, 1, 0]);
            } else if name == "storage-combined" {
                assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
            }
            let actual = local
                .materialized_type_closure()
                .unwrap()
                .into_iter()
                .filter_map(|ty| {
                    let exact = &local.exact_type_identities[ty];
                    let ExactTypeKey::Nominal(owner) = exact.key() else {
                        return None;
                    };
                    let source = identities
                        .canonical_key::<_, SourceDeclarationKey>(*owner)
                        .unwrap();
                    (source.origin() != local.cone).then_some((source.origin(), exact.id()))
                })
                .collect::<BTreeSet<_>>();
            let shared = interface
                .external_references()
                .materialized_type_dependencies(local.cone, &identities)
                .unwrap();
            let shared = shared.into_iter().collect::<BTreeSet<_>>();
            // MIR adds coroutine failure construction from the actual core
            // exception hierarchy; those implicit types have no HIR source site.
            assert!(shared.is_subset(&actual), "{name}: {shared:?}");
        });
    }
}

#[test]
fn initializer_type_position_rejects_a_function_and_a_value_constructor() {
    with_hir_source(&source("storage-combined"), |output, _| {
        let local = &output.output().local;
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        let roots = local
            .functions
            .iter()
            .map(|(_, f)| f.materialization)
            .chain(
                local
                    .struct_constructors
                    .iter()
                    .map(|(_, c)| c.materialization),
            );
        for root in roots {
            assert!(
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        Position::ConstructorInitializerResult(root),
                        &[],
                    )
                    .is_err()
            );
        }
    });
}
