use super::*;
use std::collections::BTreeSet;

pub(super) fn validate(
    callables: &BoundInheritanceProtectedCallableSourcesV1<'_, '_>,
    constructors: &BoundInheritanceConstructorSourcesV1<'_, '_>,
    protocols: &CanonicalInheritanceSourceParameterProtocolsV1,
) -> Result<(), InheritanceParameterBindingError> {
    use InheritanceParameterBindingError as Error;

    if callables.inventory != constructors.inventory {
        return Err(Error::Inventory);
    }
    let mut required = BTreeSet::new();
    for constructor in constructors.table().records() {
        insert(
            &mut required,
            CallableTemplateOrigin::Constructor(constructor.declaration()),
        )?;
    }
    for callable in callables.table().records() {
        if !matches!(callable.declaration(), CallableTemplateOrigin::Accessor(_)) {
            insert(&mut required, callable.declaration())?;
        }
    }

    if !required.into_iter().eq(protocols
        .records()
        .iter()
        .map(InheritanceSourceParameterProtocolV1::owner))
    {
        return Err(Error::Inventory);
    }
    Ok(())
}
fn insert(
    required: &mut BTreeSet<CallableTemplateOrigin>,
    declaration: CallableTemplateOrigin,
) -> Result<(), InheritanceParameterBindingError> {
    if !required.insert(declaration) {
        return Err(InheritanceParameterBindingError::Inventory);
    }
    Ok(())
}
