use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId, SourceDeclarationKind};

use super::{
    InheritanceSlotContractSemanticAuthority, InheritanceSlotContractSemanticError as Error,
    declarations,
};
use crate::{
    CheckedNominalInheritanceGraphV1, InheritanceQueryError, InheritanceSlotContractV1,
    InheritanceSlotImplementationV1, NominalInheritanceModalityV1,
};

pub(super) fn validate<A: InheritanceSlotContractSemanticAuthority<E>, E>(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    record: &InheritanceSlotContractV1,
    authority: &A,
) -> Result<(), Error<E>> {
    let node = graph
        .get(owner)
        .ok_or(Error::Inheritance(InheritanceQueryError::UnknownExact(
            owner,
        )))?;
    let schemas = graph
        .validate_slot_schemas(owner, authority)
        .map_err(Error::Schema)?;
    if !schemas
        .schemas()
        .get(record.role())
        .is_some_and(|schema| schema.slots().contains(&record.slot()))
    {
        return Err(Error::SlotIdentity);
    }
    let root = declarations::validate(
        graph,
        record.declaration(),
        record.signature(),
        record.declaration_access(),
        authority,
    )?;
    let source = graph.source(root.source).ok_or(Error::SlotIdentity)?;
    let expected = declarations::root_key(record.declaration(), source.key.declaration_kind())
        .ok_or(Error::SlotIdentity)?;
    if authority
        .dispatch_slot_key(record.slot())
        .map_err(Error::Foundation)?
        != &expected
        || PersistentDispatchSlotId::from_key(&expected).ok() != Some(record.slot())
    {
        return Err(Error::SlotIdentity);
    }
    let is_abstract = matches!(
        record.implementation(),
        InheritanceSlotImplementationV1::Abstract(_)
    );
    if is_abstract
        && !matches!(
            node.edges().modality(),
            NominalInheritanceModalityV1::Abstract | NominalInheritanceModalityV1::Interface
        )
    {
        return Err(Error::AbstractObligation);
    }
    let target = record.implementation().target();
    let target_data;
    let implementation = if target.declaration() == record.declaration()
        && target.signature() == record.signature()
        && target.declaration_access() == record.declaration_access()
    {
        &root
    } else {
        target_data = declarations::validate(
            graph,
            target.declaration(),
            target.signature(),
            target.declaration_access(),
            authority,
        )?;
        &target_data
    };
    if !matches!(
        target.declaration(),
        crate::InheritanceCallableDeclarationV1::DerivedEquality(_)
    ) && root.key.name() != implementation.key.name()
    {
        return Err(Error::TargetName);
    }
    let source = graph
        .source(implementation.source)
        .ok_or(Error::TargetOwner)?;
    let is_interface = source.key.declaration_kind() == SourceDeclarationKind::Interface;
    if !is_abstract
        && is_interface
            != matches!(
                record.implementation(),
                InheritanceSlotImplementationV1::InterfaceDefault(_)
            )
    {
        return Err(Error::TargetOwner);
    }
    let applicable = if implementation.exact_owner == owner {
        true
    } else if is_interface {
        schemas.supports_interface(implementation.exact_owner)
    } else if source.key.declaration_kind() == SourceDeclarationKind::Class {
        graph
            .is_subclass(owner, implementation.exact_owner)
            .map_err(Error::Inheritance)?
    } else {
        false
    };
    if !applicable {
        return Err(Error::TargetOwner);
    }
    Ok(())
}
