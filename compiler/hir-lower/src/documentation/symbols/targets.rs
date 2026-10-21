use super::*;
use scoop_identity::{CallableTemplateOrigin as Function, SignatureTypeKey};

impl Lowerer {
    pub(super) fn documentation_named_targets(
        &self,
        targets: Vec<NamedCallTarget>,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Vec<Symbol> {
        targets
            .into_iter()
            .filter_map(|target| {
                Some(match target {
                    NamedCallTarget::Function(id) => {
                        Symbol::Current(self.documentation_function_target(id))
                    }
                    NamedCallTarget::Value(ValueTarget::Property(id))
                    | NamedCallTarget::ExtensionProperty(id) => {
                        Symbol::Current(Source::Property(id).into())
                    }
                    NamedCallTarget::Value(ValueTarget::Object(id)) => {
                        Symbol::Current(Source::Nominal(hir::NominalOwner::Object(id)).into())
                    }
                    NamedCallTarget::Value(ValueTarget::Variant(id)) => {
                        Symbol::Current(Source::Variant(id).into())
                    }
                    NamedCallTarget::Type(TopLevelTypeTarget::Nominal(nominal)) => {
                        Symbol::Current(Source::Nominal(nominal.owner().as_nominal_owner()).into())
                    }
                    NamedCallTarget::Type(TopLevelTypeTarget::Alias(id)) => {
                        Symbol::Current(Source::TypeAlias(self.published_type_alias(id)?).into())
                    }
                    NamedCallTarget::Type(TopLevelTypeTarget::Annotation(id)) => {
                        Symbol::Current(Source::Annotation(id).into())
                    }
                    NamedCallTarget::ImportedDependency(target) => Symbol::Imported(match target {
                        hir::ImportedTarget::Type(id) => {
                            Imported::Nominal(hir::SourceNominalId::Concrete(id.persistent()))
                        }
                        hir::ImportedTarget::GenericType(id) => Imported::Nominal(
                            hir::SourceNominalId::GenericTemplate(id.persistent()),
                        ),
                        hir::ImportedTarget::ObjectValue(id) => Imported::Nominal(
                            world?.object_value(id.persistent())?.owner().persistent(),
                        ),
                        hir::ImportedTarget::Function(id) => {
                            imported_function(Function::Function(id.persistent()))
                        }
                        hir::ImportedTarget::GenericFunction(id) => {
                            imported_function(Function::GenericFunction(id.persistent()))
                        }
                        hir::ImportedTarget::Property(id) => Imported::Property(id.persistent()),
                        hir::ImportedTarget::ExtensionProperty(id) => {
                            Imported::ExtensionProperty(id.persistent())
                        }
                        hir::ImportedTarget::TypeAlias(id) => Imported::TypeAlias(id.persistent()),
                        hir::ImportedTarget::Annotation(id) => {
                            Imported::Annotation(id.persistent())
                        }
                        hir::ImportedTarget::EnumVariant(id) => Imported::Variant(id.persistent()),
                    }),
                })
            })
            .collect()
    }

    pub(in crate::documentation) fn documentation_symbol_type(
        &mut self,
        symbol: Symbol,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Option<hir::TypeId> {
        match symbol {
            Symbol::Current(Target::Declaration(Source::Nominal(owner))) => {
                Some(self.owner_ty(Owner::from_nominal_owner(owner)))
            }
            Symbol::Current(Target::Declaration(Source::TypeAlias(id))) => {
                Some(self.type_aliases[id].target)
            }
            Symbol::Current(Target::TypeParameter { parameter, .. }) => {
                Some(self.intern_type(hir::Type::Param(parameter)))
            }
            Symbol::Imported(Imported::Nominal(owner)) => match owner {
                hir::SourceNominalId::Concrete(id) => self
                    .imported_signature_type(&SignatureTypeKey::Nominal(id))
                    .ok(),
                hir::SourceNominalId::GenericTemplate(_) => {
                    let parameters = self.nominal_declaration_parameters(owner).ok()?;
                    let arguments = parameters
                        .iter()
                        .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
                        .collect();
                    self.imported_nominal_application(owner, arguments).ok()
                }
            },
            Symbol::Imported(Imported::TypeAlias(id)) => {
                let alias = world?.type_alias(id)?;
                let parameters = alias
                    .record()
                    .type_parameters()
                    .binders()
                    .iter()
                    .collect::<Vec<_>>();
                let signatures = (0..parameters.len())
                    .map(|index| SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    })
                    .collect::<Vec<_>>();
                let (_, bindings) = self
                    .prepare_imported_type_parameters(
                        &parameters,
                        &signatures,
                        ast::Span::new(0, 0),
                    )
                    .ok()?;
                self.imported_signature_type_with_bindings(alias.expansion().target(), &bindings)
                    .ok()
            }
            _ => None,
        }
    }
}

pub(super) fn imported_function(origin: Function) -> Imported {
    Imported::Callable(hir::AnnotationCallableV1::Declaration(origin))
}
