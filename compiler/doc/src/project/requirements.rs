use super::*;

impl Projection<'_> {
    pub(super) fn receiver(
        &self,
        item: &hir::DocumentationDeclaration,
    ) -> Result<Option<SignatureToken>> {
        let ty = match item.target {
            Target::Declaration(Source::Callable(Callable::Function(id))) => {
                let receiver = Source::Receiver(hir::SourceAnnotationReceiver::Callable(
                    Callable::Function(id),
                ));
                self.declarations
                    .contains_key(&receiver.into())
                    .then(|| self.export.functions[id].params[0].ty)
            }
            Target::Declaration(Source::Property(id)) => match self.export.properties[id].owner {
                hir::PropertyOwner::Extension(id) => {
                    Some(self.export.extension_properties[id].receiver_ty)
                }
                _ => None,
            },
            _ => None,
        };
        ty.map(|ty| self.component(ty, item)).transpose()
    }

    pub(super) fn equality_requirements(
        &self,
        item: &hir::DocumentationDeclaration,
    ) -> Result<Option<EqualityRequirements>> {
        let Target::DerivedFunction(_) = item.target else {
            return Ok(None);
        };
        let types: Vec<_> = match item.parent.expect("a derived member retains its owner") {
            Target::Declaration(Source::Nominal(hir::NominalOwner::Struct(id))) => {
                self.export.structs[id]
                    .semantic_fields()
                    .iter()
                    .map(|field| field.ty)
                    .collect()
            }
            Target::Declaration(Source::Nominal(hir::NominalOwner::Enum(id))) => self.export.enums
                [id]
                .variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .map(|field| field.ty)
                .collect(),
            _ => unreachable!("source derived equality belongs to a struct or enum"),
        };
        let mut components = Vec::new();
        for ty in types {
            let component = self.component(ty, item)?;
            if !components.contains(&component) {
                components.push(component);
            }
        }
        Ok(Some(EqualityRequirements { components }))
    }

    fn component(
        &self,
        ty: hir::TypeId,
        item: &hir::DocumentationDeclaration,
    ) -> Result<SignatureToken> {
        Ok(SignatureToken {
            text: hir::type_name_with_params(self.export, ty, &item.type_parameters),
            link: self.type_link(ty)?,
        })
    }
}
