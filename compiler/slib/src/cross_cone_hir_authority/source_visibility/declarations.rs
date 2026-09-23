//! Shared declaration visibility for value and callable access queries.
use super::*;
use scoop_hir::SourceAccessDomainV1;
use scoop_identity::DefinitionOriginSubject as Subject;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(in crate::cross_cone_hir_authority) fn source_declaration_access_domain(
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
            Subject::Property(_) | Subject::ExtensionProperty(_) => {
                let declaration = match subject {
                    Subject::Property(id) => PropertyOwner::Property(id),
                    Subject::ExtensionProperty(id) => PropertyOwner::ExtensionProperty(id),
                    _ => {
                        return Err(Error::DeclarationOrigin {
                            subject,
                            reason: "property access requires a logical property",
                        });
                    }
                };
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
            Subject::Constructor(_)
            | Subject::Function(_)
            | Subject::GenericFunction(_)
            | Subject::PropertyAccessor(_) => {
                let declaration = match subject {
                    Subject::Constructor(id) => CallableTemplateOrigin::Constructor(id),
                    Subject::Function(id) => CallableTemplateOrigin::Function(id),
                    Subject::GenericFunction(id) => CallableTemplateOrigin::GenericFunction(id),
                    Subject::PropertyAccessor(id) => CallableTemplateOrigin::Accessor(id),
                    _ => {
                        return Err(Error::DeclarationOrigin {
                            subject,
                            reason: "callable access requires a source callable",
                        });
                    }
                };
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
                        reason: "source callable has no shared declaration",
                    })?
                    .declared_visibility()
            }
            _ => {
                return Err(Error::DeclarationOrigin {
                    subject,
                    reason: "source access requires a nominal, callable or logical property",
                });
            }
        };
        let constraints = self.visibility_domain(key, subject, visibility)?;
        SourceAccessDomainV1::from_constraints(constraints, self.meter, path)
            .map_err(Error::Resource)
    }
}
