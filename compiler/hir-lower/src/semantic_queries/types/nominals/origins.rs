use super::*;

impl Lowerer {
    pub(crate) fn semantic_nominal_identity(
        &self,
        ty: hir::TypeId,
    ) -> Option<hir::SourceNominalId> {
        let target = self.nominal_target_for_type(ty).or_else(|| {
            if self.types[ty] != hir::Type::Any {
                return None;
            }
            self.intrinsic_type_owners
                .get(&hir::IntrinsicTypeKind::Any)
                .map(|(owner, _)| match owner {
                    crate::IntrinsicTypeOwner::Struct(id) => crate::NominalTarget::Struct(*id),
                    crate::IntrinsicTypeOwner::Class(id) => crate::NominalTarget::Class(*id),
                })
        });
        target
            .map(|target| self.nominal_identity(target.owner()).declaration_id())
            .or_else(|| {
                self.nominal_application(ty)
                    .map(|application| application.template)
            })
            .or_else(|| self.imported_nominal_owner(ty))
            .or_else(|| match (&self.types[ty], &self.core) {
                (hir::Type::FunPtr(_), CoreLoweringAuthority::Imported(core)) => {
                    Some(hir::SourceNominalId::GenericTemplate(
                        core.fundamental_types().fun_ptr().persistent(),
                    ))
                }
                _ => None,
            })
    }

    pub(crate) fn semantic_nominal_origin(
        &self,
        declaration: hir::SourceNominalId,
    ) -> Result<hir::SemanticSourceLocation, Diagnostic> {
        if let Some(&owner) = self.nominal_owners.get(&declaration) {
            let span = match owner {
                Owner::Struct(id) => self.structs[id].span,
                Owner::Enum(id) => self.enums[id].span,
                Owner::Class(id) => self.classes[id].span,
                Owner::Interface(id) => self.interfaces[id].span,
                Owner::Object(id) => self.objects[id].span,
            };
            return Ok(self.query_member_location(owner, span));
        }
        let dependency = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(declaration))
            .expect("a dependency nominal retains its declaration");
        imported_location(dependency.origin.origin())
    }
}
