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
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    use InheritanceInterfaceSemanticError as Error;
    let required = authority
        .required_inheritance_constructors(record.owner())
        .map_err(Error::Foundation)?;
    meter
        .charge_work(
            (record.constructors().records().len() as u64)
                .saturating_add(required.values().len() as u64),
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
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
            .validate_source(graph, authority, meter)
            .map_err(Error::Constructor)?;
        compare(
            source,
            authority
                .constructor_source(constructor.declaration())
                .map_err(Error::Foundation)?,
            meter,
        )?;
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedDeclarationRefV1::Constructor(constructor.declaration());
            charge_lookup(protected, meter)?;
            let Some(ProtectedDeclarationInterfaceV1::Constructor(declaration)) =
                protected.table().get(reference)
            else {
                return Err(Error::ProtectedDeclaration);
            };
            compare(
                source.declaration_access(),
                declaration.declaration_access(),
                meter,
            )?;
            let payload: &NominalSourceCallablePayloadV1 = declaration.payload();
            compare(source.payload(), payload, meter)?;
        }
    }
    let required = authority
        .required_inheritance_protected_members(record.owner())
        .map_err(Error::Foundation)?;
    compare(record.protected_members(), required, meter).map_err(|error| match error {
        Error::SourceContract => Error::Inventory,
        other => other,
    })?;
    for reference in record.protected_members().values() {
        charge_lookup(protected, meter)?;
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
fn charge_lookup<E>(
    protected: CheckedProtectedDeclarationSourcesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    let depth = (u64::BITS - (protected.table().records().len() as u64).leading_zeros()) as u64;
    meter
        .charge_work(depth.saturating_mul(128), &WirePath::root())
        .map_err(InheritanceInterfaceSemanticError::Resource)
}
