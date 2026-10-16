use super::*;
use scoop_identity::{ExactTypeKey, GeneratedNominalKey};

pub(super) fn check(
    public: &hir::CrossConeHirInterfaceSectionV1,
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) -> (Vec<String>, Vec<String>) {
    let closure = hir::NominalMaterializationClosure::from_declarations(
        public.nominal_interfaces(),
        public.callable_interfaces(),
    )
    .unwrap();
    let mut names = Vec::new();
    let mut local_only = Vec::new();
    let mut exported = 0;
    for (_, descriptor) in input.lir.module().meta.type_descriptors.iter() {
        let exact = descriptor.identity.exact_type();
        let local = input
            .mir
            .module()
            .meta
            .source_exact_types
            .get_by_identity(exact);
        let body_shape = match local.map(|local| local.ty()) {
            Some(ty @ mir::Type::Function(_)) => Some(mir::type_name(input.mir.module(), ty)),
            _ => input
                .mir
                .module()
                .meta
                .generated_exact_types
                .get_by_identity(exact)
                .and_then(|identity| match identity.location() {
                    mir::GeneratedExactTypeLocation::Closure(id) => Some(format!(
                        "closure {}",
                        mir::type_name(
                            input.mir.module(),
                            &mir::Type::Function(
                                input.mir.module().closure_classes[id].function_type
                            )
                        )
                    )),
                    mir::GeneratedExactTypeLocation::Class(id)
                        if matches!(
                            identity.nominal_record().key(),
                            GeneratedNominalKey::CoroutineFrame { .. }
                                | GeneratedNominalKey::ContinuationAdapterEnvironment { .. }
                        ) =>
                    {
                        Some(mir::type_name(input.mir.module(), &mir::Type::Class(id)))
                    }
                    _ => None,
                }),
        };
        if let Some(name) = body_shape {
            assert!(input.bridge.types().get(exact).is_none());
            assert!(result.descriptors().get(exact).is_none());
            local_only.push(name);
            continue;
        }
        if let Some(local) = local
            && let mir::SourceExactTypeOwner::NominalApplication(group) = local.owner()
        {
            let ty = input.bridge.types().get(exact).unwrap();
            let ExactTypeKey::NominalApplication { origin, .. } = local.identity_record().key()
            else {
                panic!("a nominal application retains its template origin")
            };
            assert_eq!(
                ty.origin(),
                &mir::MirTypeOriginV1::NominalApplication(*origin)
            );
            let published = result.descriptors().get(exact).unwrap();
            assert_eq!(published.exact(), exact);
            assert_eq!(
                published.definition().symbol(),
                descriptor.identity.symbol_request()
            );
            let member = descriptor.identity.odr_member_record().unwrap();
            assert_eq!(member.key().group(), group);
            assert_eq!(
                member.key().role(),
                scoop_identity::OdrMemberRole::TypeDescriptor
            );
            assert_eq!(
                descriptor.identity.symbol_request().linkage(),
                scoop_identity::LinkageClass::OdrWeak
            );
            exported += 1;
            continue;
        }
        if let Some(local) = local {
            if let ExactTypeKey::Nominal(nominal) = local.identity_record().key() {
                if public
                    .nominal_interfaces()
                    .declaration(hir::SourceNominalId::Concrete(*nominal))
                    .is_none()
                    && !matches!(local.ty(), mir::Type::Unit | mir::Type::Any)
                {
                    assert!(input.bridge.types().get(exact).is_none());
                    assert!(result.descriptors().get(exact).is_none());
                    local_only.push(mir::type_name(input.mir.module(), local.ty()));
                    continue;
                }
            }
        }
        let source_only = local.filter(|local| {
            let ExactTypeKey::Nominal(nominal) = local.identity_record().key() else {
                return false;
            };
            public
                .nominal_interfaces()
                .declaration(hir::SourceNominalId::Concrete(*nominal))
                .is_some()
                && !closure.contains(*nominal)
        });
        if let Some(local) = source_only {
            assert!(input.bridge.types().get(exact).is_none());
            assert!(result.descriptors().get(exact).is_none());
            names.push(mir::type_name(input.mir.module(), local.ty()));
        } else {
            assert!(
                result.descriptors().get(exact).is_some(),
                "missing {exact} ({}): {local:?}; generated: {:?}",
                descriptor.diagnostic_name,
                input
                    .mir
                    .module()
                    .meta
                    .generated_exact_types
                    .get_by_identity(exact)
                    .map(|identity| identity.nominal_record().key())
            );
            exported += 1;
        }
    }
    assert_eq!(result.descriptors().records().len(), exported);
    names.sort();
    local_only.sort();
    (names, local_only)
}
