//! Source visibility queries borrow shared declarations, including generic owners.
//! Their results are temporary conjunctions, never lookup or machine capabilities.

use super::*;
use scoop_hir::{DeclaredVisibilityV1, NominalInterfaceRecordV1};
use scoop_identity::{DefinitionOriginSubject, SourceDeclarationKind, SourceIdentity};
use scoop_wire::WirePath;

mod relations;
mod types;
mod values;

type Error = CrossConeHirNominalAuthorityError;

use scoop_hir::SourceAccessConstraintV1 as Constraint;

impl<'a> CanonicalCrossConeHirSurfaceAuthority<'a> {
    pub(crate) fn validate_property_setter_domains(&mut self) -> Result<(), Error> {
        for property in self
            .current_interface
            .property_interfaces()
            .all_declarations()
        {
            self.visibility_work(1)?;
            let Some(setter) = property.accessors().setter() else {
                continue;
            };
            let declaration = CallableTemplateOrigin::Accessor(setter);
            let invalid = |reason| Error::CallableDeclaration {
                declaration,
                reason,
            };
            self.visibility_work(
                u64::from(
                    self.current_interface
                        .callable_interfaces()
                        .declaration_count()
                        .max(1)
                        .ilog2(),
                ) + 1,
            )?;
            let callable = self
                .current_interface
                .callable_interfaces()
                .declaration(declaration)
                .ok_or_else(|| invalid("property setter has no shared callable declaration"))?;
            let key = self.property_key(property.declaration())?;
            let subject = match property.declaration() {
                PropertyOwner::Property(id) => DefinitionOriginSubject::Property(id),
                PropertyOwner::ExtensionProperty(id) => {
                    DefinitionOriginSubject::ExtensionProperty(id)
                }
            };
            let property_domain =
                self.visibility_domain(&key, subject, property.declared_visibility())?;
            let setter_domain =
                self.visibility_domain(&key, subject, callable.declared_visibility())?;
            for wider in &property_domain {
                let mut covered = false;
                for narrower in &setter_domain {
                    self.visibility_work(1)?;
                    if self.visibility_implies(narrower, wider)? {
                        covered = true;
                        break;
                    }
                }
                if !covered {
                    return Err(invalid(
                        "setter effective lookup domain is wider than its property domain",
                    ));
                }
            }
        }
        Ok(())
    }

    fn visibility_domain(
        &mut self,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
        visibility: DeclaredVisibilityV1,
    ) -> Result<Vec<Constraint>, Error> {
        let path = WirePath::root();
        let count = key.owners().owners().len() as u64 + 1;
        self.meter
            .check_semantic_depth(count, &path)
            .map_err(Error::Resource)?;
        self.meter
            .charge_nodes(count, &path)
            .map_err(Error::Resource)?;
        let mut domain = Vec::new();
        self.meter
            .try_reserve_exact(
                &mut domain,
                count.saturating_mul(2),
                std::mem::size_of::<Constraint>() as u64,
                &path,
            )
            .map_err(Error::Resource)?;
        self.visibility_declared(&mut domain, key, subject, visibility)?;
        for atom in key.owners().owners() {
            let owner = nominal_owner(atom)?;
            let (record, key) = self.visibility_nominal(owner)?;
            self.visibility_declared(
                &mut domain,
                &key,
                nominal_subject(owner),
                record.declaration_details().declared_visibility(),
            )?;
        }
        Ok(domain)
    }

    fn visibility_declared(
        &mut self,
        domain: &mut Vec<Constraint>,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
        visibility: DeclaredVisibilityV1,
    ) -> Result<(), Error> {
        let source = self.visibility_source(key.origin(), subject)?;
        match visibility {
            DeclaredVisibilityV1::Public => self.visibility_work(1)?,
            DeclaredVisibilityV1::Internal => domain.push(Constraint::Cone(source.cone())),
            DeclaredVisibilityV1::Private => match key.owners().owners().last() {
                Some(owner) => domain.push(Constraint::LexicalOwner(nominal_owner(owner)?)),
                None => {
                    let bytes = source.logical_path().as_str().len() as u64;
                    self.meter
                        .charge_owned_bytes(bytes, &WirePath::root())
                        .map_err(Error::Resource)?;
                    self.visibility_work(bytes)?;
                    domain.push(Constraint::Cone(source.cone()));
                    domain.push(Constraint::File(source.clone()));
                }
            },
            DeclaredVisibilityV1::Protected => {
                let owner = key
                    .owners()
                    .owners()
                    .last()
                    .ok_or(Error::DeclarationOrigin {
                        subject,
                        reason: "protected declaration requires a class owner",
                    })?;
                let owner = nominal_owner(owner)?;
                let (_, key) = self.visibility_nominal(owner)?;
                if key.declaration_kind() != SourceDeclarationKind::Class {
                    return Err(Error::DeclarationOrigin {
                        subject,
                        reason: "protected declaration requires a class owner",
                    });
                }
                domain.push(Constraint::SubclassesOf(owner));
            }
        }
        Ok(())
    }

    fn visibility_nominal(
        &mut self,
        declaration: SourceNominalId,
    ) -> Result<(&'a NominalInterfaceRecordV1, SourceDeclarationKey), Error> {
        let key = match declaration {
            SourceNominalId::Concrete(id) => {
                self.identities.canonical_key::<_, SourceDeclarationKey>(id)
            }
            SourceNominalId::GenericTemplate(id) => {
                self.identities.canonical_key::<_, SourceDeclarationKey>(id)
            }
        }
        .map_err(Error::Identity)?;
        let bytes =
            scoop_wire::encoded_length(key.as_ref()).map_err(|_| Error::NominalDeclaration {
                declaration,
                reason: "cannot encode nominal source declaration key",
            })?;
        self.meter
            .charge_owned_bytes(bytes, &WirePath::root())
            .map_err(Error::Resource)?;
        self.visibility_work(bytes.saturating_add(65))?;
        let key = key.as_ref().clone();
        let interface = self.provider_interface(key.origin())?;
        self.visibility_work(
            u64::from(
                interface
                    .nominal_interfaces()
                    .declaration_count()
                    .max(1)
                    .ilog2(),
            ) + 1,
        )?;
        let record = interface
            .nominal_interfaces()
            .declaration(declaration)
            .ok_or(Error::MissingNominalInterface {
                origin: key.origin(),
                declaration,
            })?;
        Ok((record, key))
    }

    fn visibility_source(
        &mut self,
        provider: ConeIdentity,
        subject: DefinitionOriginSubject,
    ) -> Result<&'a SourceIdentity, Error> {
        self.visibility_work(self.dependencies.len() as u64 + 1)?;
        let foundation = if provider == self.current {
            self.current_foundation
        } else {
            self.dependencies
                .iter()
                .find(|entry| entry.identity == provider)
                .map(|entry| entry.foundation)
                .ok_or(Error::UnreachableProvider { origin: provider })?
        };
        let origin = foundation
            .definition_origin(subject)
            .ok_or(Error::MissingDefinitionOrigin { subject })?;
        let source = origin.origin().source();
        if source.cone() != provider {
            return Err(Error::DeclarationOrigin {
                subject,
                reason: "source origin differs from its typed provider",
            });
        }
        Ok(source)
    }

    fn visibility_work(&mut self, work: u64) -> Result<(), Error> {
        self.meter
            .charge_work(work, &WirePath::root())
            .map_err(Error::Resource)
    }
}

fn nominal_owner(atom: &DefinitionOwnerAtom) -> Result<SourceNominalId, Error> {
    match atom {
        DefinitionOwnerAtom::Type(id) => Ok(SourceNominalId::Concrete(*id)),
        DefinitionOwnerAtom::GenericType(id) => Ok(SourceNominalId::GenericTemplate(*id)),
        owner => Err(Error::InvalidDeclarationOwner {
            entity: "source visibility",
            owner: owner.clone(),
        }),
    }
}

fn nominal_subject(owner: SourceNominalId) -> DefinitionOriginSubject {
    match owner {
        SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
        SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
    }
}
