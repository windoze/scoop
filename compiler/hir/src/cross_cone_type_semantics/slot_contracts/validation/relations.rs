use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId, SourceDeclarationKind};

use super::{
    InheritanceSlotContractSemanticAuthority, InheritanceSlotContractSemanticError as Error,
    declarations,
};
use crate::{
    CheckedNominalInheritanceGraphV1, DeclaredVisibilityV1, InheritanceQueryError,
    InheritanceSlotContractV1, InheritanceSlotImplementationV1, NominalInheritanceModalityV1,
    SourceNominalId,
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
    let mut member = false;
    for schema in schemas.schemas().records() {
        member |= schema.slots().contains(&record.slot());
    }
    if !member {
        return Err(Error::SlotIdentity);
    }
    let (root, root_domains) = declarations::validate(
        graph,
        record.declaration(),
        record.declaration_owner(),
        record.signature(),
        record.declaration_access(),
        authority,
    )?;
    let source = graph
        .source(SourceNominalId::Concrete(record.declaration_owner()))
        .ok_or(Error::SlotIdentity)?;
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
    let domain = graph
        .validate_access_domain(record.domain().domain())
        .map_err(Error::Access)?;
    if domain.domain() != root_domains.lookup().domain() {
        return Err(Error::Domain);
    }
    match record.implementation() {
        InheritanceSlotImplementationV1::Abstract => {
            if !matches!(
                node.edges().modality(),
                NominalInheritanceModalityV1::Abstract | NominalInheritanceModalityV1::Interface
            ) {
                return Err(Error::AbstractObligation);
            }
            let owner_domains = graph.replay_nominal_domains(owner).map_err(Error::Access)?;
            if !domain
                .covers(owner_domains.inheritance())
                .map_err(Error::Access)?
            {
                return Err(Error::AbstractObligation);
            }
        }
        InheritanceSlotImplementationV1::Concrete(target)
        | InheritanceSlotImplementationV1::InterfaceDefault(target) => {
            let (implementation, access) = declarations::validate(
                graph,
                target.declaration(),
                target.owner(),
                target.signature(),
                target.declaration_access(),
                authority,
            )?;
            if root.key.name() != implementation.key.name() {
                return Err(Error::TargetName);
            }
            let preserves_protected = record.declaration_access().declared_visibility()
                == DeclaredVisibilityV1::Protected
                && target.declaration_access().declared_visibility()
                    == DeclaredVisibilityV1::Protected
                && graph
                    .is_subclass(implementation.exact_owner, root.exact_owner)
                    .map_err(Error::Inheritance)?;
            if !preserves_protected && !access.declared().covers(&domain).map_err(Error::Access)? {
                return Err(Error::Domain);
            }
            let source = graph
                .source(SourceNominalId::Concrete(target.owner()))
                .ok_or(Error::TargetOwner)?;
            let is_interface = source.key.declaration_kind() == SourceDeclarationKind::Interface;
            if is_interface
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
        }
    }
    Ok(())
}
