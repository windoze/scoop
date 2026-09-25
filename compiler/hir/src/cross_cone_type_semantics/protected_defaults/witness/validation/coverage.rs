use super::*;
use crate::{
    AccessDomainSemanticError, CanonicalProtectedSlotRefsV1, CheckedNominalInheritanceGraphV1,
    CheckedNominalInheritanceInterfacesV1, CheckedPersistentAccessDomainV1,
    InheritanceCallableDeclarationV1, InheritanceQueryError, PersistentAccessDomainV1,
};

/// The complete root-slot inventory is supplied by definition-side source
/// metadata, independently of the witness and its claimed domain sequence.
pub trait ProtectedDefaultRootSlotSemanticAuthority<E> {
    fn default_root_slots(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, E>;
}

/// Source-owner and complete call-domain coverage only. The enclosing reference
/// pass must still join `target` to its typed declaration and validate every use.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedDefaultDomainCoverageV1<'a> {
    witness: &'a ParamFreeProtectedDefaultAccessWitnessV1,
}
impl CheckedProtectedDefaultDomainCoverageV1<'_> {
    pub const fn witness(&self) -> &ParamFreeProtectedDefaultAccessWitnessV1 {
        self.witness
    }
}
impl<'w> CheckedParamFreeProtectedDefaultWitnessSourceV1<'w, '_> {
    pub fn validate_domains<'g, 'a, A: ProtectedDefaultRootSlotSemanticAuthority<E>, E>(
        self,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,
        inheritance: CheckedNominalInheritanceInterfacesV1<'_>,
        target: &CheckedPersistentAccessDomainV1<'g, 'a>,
        authority: &A,
    ) -> Result<CheckedProtectedDefaultDomainCoverageV1<'w>, ProtectedDefaultDomainCoverageError<E>>
    {
        use ProtectedDefaultDomainCoverageError as Error;
        let direct = self
            .source
            .declaration_access()
            .replay(graph)
            .map_err(Error::Domain)?;
        compare_domain(
            self.witness.direct_call_domain.domain(),
            direct.lookup().domain(),
        )?;
        compare_domain(self.witness.target_domain.domain(), target.domain())?;
        if !target.covers(direct.lookup()).map_err(Error::Domain)? {
            return Err(Error::DirectCoverage);
        }
        let roots = authority
            .default_root_slots(self.source.declaration())
            .map_err(Error::Foundation)?;
        let claimed = self.witness.slot_call_domains.records();
        let source_roots = self.source.payload().slot_relations().slots();

        if source_roots != roots.slots()
            || !claimed
                .iter()
                .map(|record| record.slot())
                .eq(roots.slots().iter().copied())
        {
            return Err(Error::SlotInventory);
        }
        if !claimed.is_empty() {
            let exact = graph
                .source_exact(self.source.payload().owner())
                .map_err(Error::Inheritance)?;
            let interfaces = inheritance.table();

            let owner = interfaces.get(exact).ok_or(Error::InheritanceOwner)?;
            for domain in claimed {
                let slot = owner
                    .slots()
                    .get(domain.slot())
                    .ok_or(Error::SlotInventory)?;
                let callable = self.source.declaration();
                let matches = |declaration| matches!((declaration, callable), (InheritanceCallableDeclarationV1::Function(left), CallableTemplateOrigin::Function(right)) if left == right);
                if !matches(slot.declaration())
                    && !slot
                        .implementation()
                        .target()
                        .is_some_and(|target| matches(target.declaration()))
                {
                    return Err(Error::SlotOwner);
                }
                compare_domain(domain.domain().domain(), slot.domain().domain())?;
                let required = graph
                    .validate_access_domain(slot.domain().domain())
                    .map_err(Error::Domain)?;
                if !target.covers(&required).map_err(Error::Domain)? {
                    return Err(Error::SlotCoverage(domain.slot()));
                }
            }
        }
        Ok(CheckedProtectedDefaultDomainCoverageV1 {
            witness: self.witness,
        })
    }
}
fn compare_domain<E>(
    left: &PersistentAccessDomainV1,
    right: &PersistentAccessDomainV1,
) -> Result<(), ProtectedDefaultDomainCoverageError<E>> {
    if left != right {
        Err(ProtectedDefaultDomainCoverageError::DomainMismatch)
    } else {
        Ok(())
    }
}
#[derive(Debug)]
pub enum ProtectedDefaultDomainCoverageError<E> {
    Resource(WireError),
    Foundation(E),
    Domain(AccessDomainSemanticError),
    Inheritance(InheritanceQueryError),
    DomainMismatch,
    DirectCoverage,
    SlotInventory,
    InheritanceOwner,
    SlotOwner,
    SlotCoverage(scoop_identity::PersistentDispatchSlotId),
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultDomainCoverageError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f), Self::Foundation(error) => error.fmt(f), Self::Domain(error) => error.fmt(f), Self::Inheritance(error) => error.fmt(f),
            Self::DomainMismatch => f.write_str("default witness domain disagrees with replayed source/target access"),
            Self::DirectCoverage => f.write_str("default target does not cover the entire direct call domain"),
            Self::SlotInventory => f.write_str("default witness slot domains do not cover the complete source root-slot inventory"),
            Self::InheritanceOwner => f.write_str("default owner has no checked inheritance interface"),
            Self::SlotOwner => f.write_str("default owner is neither the root declaration nor the selected slot implementation"),
            Self::SlotCoverage(slot) => write!(f, "default target does not cover the complete call domain of slot {slot:?}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultDomainCoverageError<E> {}
