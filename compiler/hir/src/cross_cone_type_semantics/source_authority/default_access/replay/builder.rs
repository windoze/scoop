use super::*;
use PersistentAccessConstraintV1 as Constraint;

pub(super) struct Builder<'a, A: SourceDomainAuthority> {
    authority: &'a A,

    persistent: Vec<Constraint>,
    generic: Vec<PersistentGenericTypeId>,
}
impl<'a, A: SourceDomainAuthority> Builder<'a, A> {
    pub fn new(authority: &'a A, count: usize, path: &WirePath) -> Result<Self, Error<A::Error>> {
        let mut persistent = Vec::new();
        let mut generic = Vec::new();

        scoop_wire::allocation::try_reserve_count(
            &mut persistent,
            (count as u64).saturating_mul(2),
            path,
        )?;
        scoop_wire::allocation::try_reserve_count(&mut generic, count as u64, path)?;
        Ok(Self {
            authority,

            persistent,
            generic,
        })
    }
    pub fn declared(&mut self, access: &DeclarationAccessSourceV1) -> Result<(), Error<A::Error>> {
        let source = access.definition_origin().origin().source();
        match (access.declared_visibility(), access.lexical_owners().last()) {
            (DeclaredVisibilityV1::Public, _) => return Ok(()),
            (DeclaredVisibilityV1::Internal, _) => self.push(Constraint::Cone(source.cone())),
            (DeclaredVisibilityV1::Private, Some(owner)) => {
                self.push(Constraint::LexicalOwner(*owner))
            }
            (DeclaredVisibilityV1::Private, None) => {
                self.push(Constraint::Cone(source.cone()));
                self.push(Constraint::File(source.clone()));
            }
            (DeclaredVisibilityV1::Protected, Some(owner)) => self.protected(*owner)?,
            (DeclaredVisibilityV1::Protected, None) => return Err(Error::ProtectedOwner),
        }
        Ok(())
    }
    fn protected(&mut self, owner: SourceNominalId) -> Result<(), Error<A::Error>> {
        let key = self
            .authority
            .nominal_key(owner)
            .map_err(Error::Authority)?;
        if key.declaration_kind() != SourceDeclarationKind::Class {
            return Err(Error::ProtectedClass(owner));
        }
        match owner {
            SourceNominalId::GenericTemplate(id) => {
                if !self.generic.contains(&id) {
                    self.generic.push(id);
                }
            }
            SourceNominalId::Concrete(id) => {
                let expected = ExactTypeKey::Nominal(id);

                let exact = PersistentExactTypeId::from_key(&expected).map_err(Error::Identity)?;
                if self.authority.exact_key(exact).map_err(Error::Authority)? != &expected {
                    return Err(Error::ExactClass(exact));
                }
                self.push(Constraint::SubclassesOf(exact));
            }
        }
        Ok(())
    }
    fn push(&mut self, constraint: Constraint) {
        if !self.persistent.contains(&constraint) {
            self.persistent.push(constraint);
        }
    }
    pub fn finish(self) -> Result<DefaultSourceAccessDomainV1, Error<A::Error>> {
        let persistent = PersistentAccessDomainV1::try_from_constraints(self.persistent)
            .map_err(Error::Domain)?;
        let generic =
            CanonicalPersistentIdsV1::try_new(self.generic).map_err(Error::GenericSubclasses)?;
        DefaultSourceAccessDomainV1::try_new(persistent, generic).map_err(Error::Build)
    }
}
