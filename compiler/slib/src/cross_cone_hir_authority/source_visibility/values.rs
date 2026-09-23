use super::*;
use scoop_hir::{
    DefaultFieldRefV1, DefaultSourceFieldAccessSubjectV1, DefaultSourceIndirectTargetV1,
    DefaultSourceValueTargetV1 as Target, DefaultTargetIdentityQueriesV1, SourceAccessDomainV1,
};
use scoop_identity::DefinitionOriginSubject as Subject;

use crate::cross_cone_hir_authority::CrossConeHirDefaultValueAccessError as ValueError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(in crate::cross_cone_hir_authority) fn source_value_access_domain(
        &mut self,
        target: Target<'_>,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, ValueError> {
        self.meter.charge_nodes(1, path)?;
        self.meter.charge_work(1, path)?;
        if matches!(target, Target::Field(DefaultFieldRefV1::Tuple { .. })) {
            return Ok(SourceAccessDomainV1::universal());
        }
        let provider = target.source_provider(self.current, self.identities, self.meter, path)?;
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
        let query = DefaultTargetIdentityQueriesV1::new(provider, foundation, self.identities);
        let subject = match target {
            Target::Constructor(target) => {
                query.default_constructor_access_subject(target, self.meter)?
            }
            Target::Global(id) => query.default_global_access_subject(id, self.meter)?,
            Target::Singleton(id) => query.default_indirect_access_subject(
                DefaultSourceIndirectTargetV1::Singleton(id),
                self.meter,
            )?,
            Target::Field(target) => {
                match query.default_field_access_subject(target, self.meter)? {
                    DefaultSourceFieldAccessSubjectV1::Declaration(subject) => subject,
                    DefaultSourceFieldAccessSubjectV1::TupleElement { .. } => {
                        return Ok(SourceAccessDomainV1::universal());
                    }
                }
            }
        };
        let key = query.source_declaration_key(subject, self.meter)?;
        self.value_subject_domain(subject, key, path)
            .map_err(Into::into)
    }

    fn value_subject_domain(
        &mut self,
        subject: Subject,
        key: &SourceDeclarationKey,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, Error> {
        self.visibility_work(self.dependencies.len() as u64 + 1)?;
        let interface = self.provider_interface(key.origin())?;
        let visibility = match subject {
            Subject::Type(_) | Subject::GenericType(_) => {
                let declaration = match subject {
                    Subject::Type(id) => SourceNominalId::Concrete(id),
                    Subject::GenericType(id) => SourceNominalId::GenericTemplate(id),
                    _ => {
                        return Err(Error::DeclarationOrigin {
                            subject,
                            reason: "value access owner must be a source nominal",
                        });
                    }
                };
                self.visibility_work(
                    u64::from(
                        interface
                            .nominal_interfaces()
                            .declaration_count()
                            .max(1)
                            .ilog2(),
                    ) + 1,
                )?;
                interface
                    .nominal_interfaces()
                    .declaration(declaration)
                    .ok_or(Error::MissingNominalInterface {
                        origin: key.origin(),
                        declaration,
                    })?
                    .declaration_details()
                    .declared_visibility()
            }
            Subject::Property(id) => {
                let declaration = PropertyOwner::Property(id);
                self.visibility_work(
                    u64::from(
                        interface
                            .property_interfaces()
                            .declaration_count()
                            .max(1)
                            .ilog2(),
                    ) + 1,
                )?;
                interface
                    .property_interfaces()
                    .declaration(declaration)
                    .ok_or(Error::MissingPropertyInterface { declaration })?
                    .declared_visibility()
            }
            Subject::Constructor(id) => {
                let declaration = CallableTemplateOrigin::Constructor(id);
                self.visibility_work(
                    u64::from(
                        interface
                            .callable_interfaces()
                            .declaration_count()
                            .max(1)
                            .ilog2(),
                    ) + 1,
                )?;
                interface
                    .callable_interfaces()
                    .declaration(declaration)
                    .ok_or(Error::CallableDeclaration {
                        declaration,
                        reason: "constructor has no shared callable declaration",
                    })?
                    .declared_visibility()
            }
            _ => {
                return Err(Error::DeclarationOrigin {
                    subject,
                    reason: "value access requires a nominal, constructor or logical property",
                });
            }
        };
        let constraints = self.visibility_domain(key, subject, visibility)?;
        SourceAccessDomainV1::from_constraints(constraints, self.meter, path)
            .map_err(Error::Resource)
    }
}
