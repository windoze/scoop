use super::*;
use crate::{
    DeclaredVisibilityV1, NominalSourceCallablePayloadV1, ProtectedDeclarationInterfaceV1,
    SourceNominalId,
};

pub(super) fn validate<A: NominalInheritanceInterfaceSemanticAuthority<E>, E>(
    record: &NominalInheritanceInterfaceV1,
    owner: SourceNominalId,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    protected: CheckedProtectedDeclarationSourcesV1<'_>,
    authority: &mut A,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    use InheritanceInterfaceSemanticError as Error;
    let required = authority
        .required_inheritance_constructors(record.owner())
        .map_err(Error::Foundation)?;

    if !record
        .constructors()
        .records()
        .iter()
        .map(InheritanceConstructorInterfaceV1::declaration)
        .eq(required.values().iter().copied())
    {
        return Err(Error::Inventory);
    }
    for constructor in record.constructors().records() {
        let source = constructor.source();
        if source.payload().owner() != owner {
            return Err(Error::ConstructorOwner);
        }
        source
            .validate_source(graph, authority)
            .map_err(Error::Constructor)?;
        compare(
            source,
            authority
                .constructor_source(constructor.declaration())
                .map_err(Error::Foundation)?,
        )?;
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedDeclarationRefV1::Constructor(constructor.declaration());

            let Some(ProtectedDeclarationInterfaceV1::Constructor(declaration)) =
                protected.table().get(reference)
            else {
                return Err(Error::ProtectedDeclaration);
            };
            compare(
                source.declaration_access(),
                declaration.declaration_access(),
            )?;
            let payload: &NominalSourceCallablePayloadV1 = declaration.payload();
            compare(source.payload(), payload)?;
        }
    }
    let required = authority
        .required_inheritance_protected_members(record.owner())
        .map_err(Error::Foundation)?;
    compare(record.protected_members(), required).map_err(|error| match error {
        Error::SourceContract => Error::Inventory,
        other => other,
    })?;
    for reference in record.protected_members().values() {
        let declaration = protected
            .table()
            .get(*reference)
            .ok_or(Error::ProtectedDeclaration)?;
        if declaration
            .declaration_access()
            .lexical_owners()
            .last()
            .copied()
            != Some(owner)
        {
            return Err(Error::ProtectedDeclaration);
        }
    }
    Ok(())
}
