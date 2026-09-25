//! Minimal shared source declarations, reached from public roots and actual bodies.

use super::*;
use scoop_hir::{PropertyDeclarationId, SignatureNominalWalker};
use scoop_wire::WirePath;
use std::collections::BTreeSet;

mod declarations;
mod defaults;
mod errors;
pub use errors::{CrossConeHirSourceInventoryError, SourceInventoryDeclaration};
type Error = CrossConeHirSourceInventoryError;
type Declaration = SourceInventoryDeclaration;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_shared_source_inventory(&mut self) -> Result<(), Error> {
        let interface = self.current_interface;
        let mut closure = Closure {
            world: self,
            required: BTreeSet::new(),
            pending: Vec::new(),
        };
        for record in interface.nominal_interfaces().records() {
            closure.nominal(record.declaration())?;
        }
        for record in interface.callable_interfaces().records() {
            closure.callable(record.declaration())?;
        }
        for record in interface.property_interfaces().records() {
            closure.property(record.declaration())?;
        }
        while let Some(declaration) = closure.pending.pop() {
            match declaration {
                Declaration::Nominal(id) => closure.expand_nominal(id)?,
                Declaration::Callable(id) => closure.expand_callable(id)?,
                Declaration::Property(id) => closure.expand_property(id)?,
            }
        }
        let supports = interface
            .nominal_interfaces()
            .support_records()
            .iter()
            .map(|r| Declaration::Nominal(r.declaration()))
            .chain(
                interface
                    .callable_interfaces()
                    .support_records()
                    .iter()
                    .map(|r| Declaration::Callable(r.declaration())),
            )
            .chain(
                interface
                    .property_interfaces()
                    .support_records()
                    .iter()
                    .map(|r| Declaration::Property(r.declaration())),
            );
        for declaration in supports {
            if !closure.required.contains(&declaration) {
                return Err(Error::Unrelated(declaration));
            }
        }
        Ok(())
    }
}

struct Closure<'a, 'b> {
    world: &'a mut CanonicalCrossConeHirSurfaceAuthority<'b>,
    required: BTreeSet<Declaration>,
    pending: Vec<Declaration>,
}

impl Closure<'_, '_> {
    fn add(&mut self, declaration: Declaration) -> Result<(), Error> {
        if self.required.contains(&declaration) {
            return Ok(());
        }
        let path = WirePath::root();

        scoop_wire::allocation::try_reserve(&mut self.pending, 1, &path)?;
        self.required.insert(declaration);
        self.pending.push(declaration);
        Ok(())
    }

    fn nominal(&mut self, owner: SourceNominalId) -> Result<(), Error> {
        if let SourceNominalId::Concrete(id) = owner
            && [
                scoop_identity::CoreBuiltinNominal::Unit,
                scoop_identity::CoreBuiltinNominal::Any,
            ]
            .iter()
            .any(|builtin| builtin.identity_record().id() == id)
        {
            return Ok(());
        }
        let key = self.world.source_nominal_key(owner)?;
        let provider = key.origin();
        let table = self
            .world
            .provider_interface(provider)?
            .nominal_interfaces();

        if table.declaration(owner).is_none() {
            return Err(Error::Missing(Declaration::Nominal(owner)));
        }
        if provider == self.world.current {
            self.add(Declaration::Nominal(owner))?;
        }
        Ok(())
    }

    fn callable(&mut self, id: CallableTemplateOrigin) -> Result<(), Error> {
        let provider = scoop_hir::DefaultTargetIdentityQueriesV1::source_callable_provider(
            id,
            self.world.identities,
        )?;
        let table = self
            .world
            .provider_interface(provider)?
            .callable_interfaces();

        if table.declaration(id).is_none() {
            return Err(Error::Missing(Declaration::Callable(id)));
        }
        if provider == self.world.current {
            self.add(Declaration::Callable(id))?;
        }
        Ok(())
    }

    fn property(&mut self, id: PropertyDeclarationId) -> Result<(), Error> {
        let key = match id {
            PropertyOwner::Property(id) => self
                .world
                .identities
                .canonical_key::<PersistentPropertyId, SourceDeclarationKey>(id),
            PropertyOwner::ExtensionProperty(id) => {
                self.world
                    .identities
                    .canonical_key::<PersistentExtensionPropertyId, SourceDeclarationKey>(id)
            }
        }
        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        let provider = key.origin();
        let table = self
            .world
            .provider_interface(provider)?
            .property_interfaces();

        if table.declaration(id).is_none() {
            return Err(Error::Missing(Declaration::Property(id)));
        }
        if provider == self.world.current {
            self.add(Declaration::Property(id))?;
        }
        Ok(())
    }

    fn owner(&mut self, owner: PublicDeclarationOwnerV1) -> Result<(), Error> {
        match owner {
            PublicDeclarationOwnerV1::Nominal(owner) => self.nominal(owner),
            PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => Ok(()),
        }
    }

    fn binders(&mut self, binders: &scoop_hir::CanonicalBinderListV1) -> Result<(), Error> {
        for binder in binders.binders() {
            if let scoop_hir::TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() {
                if let Some(class) = bounds.class() {
                    self.signature(class)?;
                }
                for interface in bounds.interfaces().values() {
                    self.signature(interface)?;
                }
            }
        }
        Ok(())
    }

    fn signature(&mut self, signature: &SignatureTypeKey) -> Result<(), Error> {
        let path = WirePath::root();
        let mut walk = SignatureNominalWalker::new(signature, &path)?;
        while let Some(owner) = walk.next(&path)? {
            self.nominal(owner)?;
        }
        Ok(())
    }
}
