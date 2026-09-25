use super::*;
use scoop_identity::PropertyOwner;

pub(super) fn target(provider: &Loaded, target: CallableTemplateOrigin) -> BindingTarget {
    match target {
        CallableTemplateOrigin::Function(id) => {
            let key = provider
                .identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap();
            if provider
                .public
                .callable_interfaces()
                .declaration(target)
                .unwrap()
                .receiver()
                .is_some()
            {
                BindingTarget::extension_function(&key).unwrap()
            } else {
                BindingTarget::function(&key).unwrap()
            }
        }
        CallableTemplateOrigin::Accessor(id) => {
            let key = provider
                .identities
                .canonical_key::<_, PropertyAccessorKey>(id)
                .unwrap();
            let PropertyOwner::Property(property) = key.owner() else {
                panic!("fixture accessors use ordinary properties");
            };
            BindingTarget::property(
                provider
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(property)
                    .unwrap()
                    .as_ref(),
            )
            .unwrap()
        }
        CallableTemplateOrigin::Constructor(_) => {
            let owner = provider
                .public
                .callable_interfaces()
                .declaration(target)
                .unwrap()
                .owner()
                .nominal_owner()
                .unwrap();
            let key = match owner {
                SourceNominalId::Concrete(owner) => provider
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(owner),
                SourceNominalId::GenericTemplate(owner) => provider
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(owner),
            }
            .unwrap();
            BindingTarget::type_name(&key).unwrap()
        }
        CallableTemplateOrigin::VariantConstructor(id) => BindingTarget::enum_variant(id),
        CallableTemplateOrigin::GenericFunction(_) => panic!("fixture source calls are concrete"),
    }
}
