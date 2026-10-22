use super::*;
use crate::imports::{CurrentUnitTarget, ExportLookupTarget};
use scoop_identity::{BindableEntity, BindingNamespace, CanonicalIdentifier, ConeIdentity};

mod identities;

impl FrontendQueries<'_, '_> {
    pub(super) fn query_provider(
        &self,
        reference: scoop_identity::DecodedPersistentId<ConeIdentity>,
    ) -> Option<ConeIdentity> {
        let current = self.lowerer.current_cone();
        if current.as_array() == reference.as_array() {
            return Some(current);
        }
        self.world?
            .direct_providers()
            .map(|provider| provider.id())
            .find(|provider| provider.as_array() == reference.as_array())
    }
}

impl Lowerer {
    pub(super) fn query_export(
        &mut self,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        provider: ConeIdentity,
        path: &[CanonicalIdentifier],
        namespace: BindingNamespace,
    ) -> Result<Vec<BindableEntity>, Diagnostic> {
        if provider != self.current_cone() {
            let world = world
                .filter(|world| world.direct_provider(provider).is_some())
                .ok_or_else(|| {
                    Diagnostic::at(
                        Span::new(0, 0),
                        "export provider is not in the current dependency view",
                    )
                })?;
            return match world.resolve_provider_export(provider, path, namespace) {
                Ok(group) => Ok(group
                    .targets()
                    .map(|binding| binding.target().persistent())
                    .collect()),
                Err(
                    hir::DirectNamespaceLookupError::NoVisiblePackage
                    | hir::DirectNamespaceLookupError::ExpectedBindingAfterPackage
                    | hir::DirectNamespaceLookupError::MissingBinding { .. }
                    | hir::DirectNamespaceLookupError::NotStaticOwner { .. },
                ) => Ok(Vec::new()),
                Err(error) => Err(Diagnostic::at(
                    Span::new(0, 0),
                    format!("cannot resolve export: {error}"),
                )),
            };
        }
        let targets = self
            .imports
            .query_public_exports(self, world, path, namespace)?;
        let mut entities = Vec::new();
        for target in targets {
            let entity = match target {
                ExportLookupTarget::Dependency(target) => target.persistent(),
                ExportLookupTarget::Current(binding) => {
                    self.query_export_identity(self.imports.binding(binding).target, namespace)?
                }
            };
            if !entities.contains(&entity) {
                entities.push(entity);
            }
        }
        Ok(entities)
    }

    fn query_export_identity(
        &mut self,
        target: CurrentUnitTarget,
        namespace: BindingNamespace,
    ) -> Result<BindableEntity, Diagnostic> {
        let owner = match target {
            CurrentUnitTarget::Class(id) => Some(Owner::Class(id)),
            CurrentUnitTarget::Struct(id) => Some(Owner::Struct(id)),
            CurrentUnitTarget::Enum(id) => Some(Owner::Enum(id)),
            CurrentUnitTarget::Interface(id) => Some(Owner::Interface(id)),
            CurrentUnitTarget::Object(id) if namespace == BindingNamespace::Type => {
                Some(Owner::Object(id))
            }
            _ => None,
        };
        if let Some(owner) = owner {
            return Ok(match self.nominal_identity(owner).declaration_id() {
                hir::SourceNominalId::Concrete(id) => BindableEntity::Type(id),
                hir::SourceNominalId::GenericTemplate(id) => BindableEntity::GenericType(id),
            });
        }
        Ok(match target {
            CurrentUnitTarget::Function(id) => match self.query_function_identity(id)? {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    BindableEntity::Function(record.id())
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    BindableEntity::GenericFunction(record.id())
                }
            },
            CurrentUnitTarget::Annotation(id) => BindableEntity::Annotation(id),
            CurrentUnitTarget::Property(id) => {
                match self.complete_property_identity(id).map_err(|error| {
                    Diagnostic::at_file(error.file(), error.span(), error.to_string())
                })? {
                    hir::HirPropertyIdentity::Ordinary(record) => {
                        BindableEntity::Property(record.id())
                    }
                    hir::HirPropertyIdentity::Extension(record) => {
                        BindableEntity::ExtensionProperty(record.id())
                    }
                }
            }
            CurrentUnitTarget::EnumVariant(variant) => {
                let application = self.enums[variant.enumeration()].self_application;
                BindableEntity::EnumVariant(
                    self.enum_variant_at(application, variant.local_index())
                        .variant,
                )
            }
            CurrentUnitTarget::Object(id) => {
                BindableEntity::ObjectValue(self.query_object_value_identity(id))
            }
            CurrentUnitTarget::TypeAlias(id) => {
                let alias = self
                    .published_type_alias(id)
                    .expect("fixed source queries follow complete alias binding");
                BindableEntity::TypeAlias(self.query_alias_identity(alias)?.id())
            }
            CurrentUnitTarget::SourceProperty(_) | CurrentUnitTarget::SourceVariant(_) => {
                unreachable!("fixed source queries follow import target completion")
            }
            CurrentUnitTarget::Class(_)
            | CurrentUnitTarget::Struct(_)
            | CurrentUnitTarget::Enum(_)
            | CurrentUnitTarget::Interface(_) => {
                unreachable!("nominal type targets were handled above")
            }
        })
    }
}
