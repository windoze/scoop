use super::*;
use crate::Owner;
use hir::{AnnotationTargetV1 as Imported, SourceAnnotationTarget as Current};

impl Lowerer {
    pub(super) fn editing_nominal_declaration(
        &mut self,
        id: hir::SourceNominalId,
    ) -> Option<(String, Option<Symbol>)> {
        let parameters = self.nominal_declaration_parameters(id).ok()?;
        let (kind, name, symbol) = if let Some(owner) = self.nominal_owners.get(&id).copied() {
            let kind = match owner {
                Owner::Class(_) => "class",
                Owner::Struct(_) => "struct",
                Owner::Enum(_) => "enum",
                Owner::Interface(_) => "interface",
                Owner::Object(_) => "object",
            };
            let prefix = self.local_type_parameter_count(owner).max(match owner {
                Owner::Class(class) => self.enclosing_parameter_count(class),
                Owner::Object(_) => parameters.len(),
                _ => 0,
            });
            (
                kind,
                format!(
                    "{}{}",
                    owner.describe_name(self),
                    generics(&parameters[prefix..])
                ),
                Symbol::Current(Current::Nominal(owner.as_nominal_owner()).into()),
            )
        } else {
            let declaration = self.dependencies.as_ref()?.nominal_declaration(id)?;
            let kind = match declaration.interface.kind() {
                hir::PublicNominalKindV1::Class => "class",
                hir::PublicNominalKindV1::Struct => "struct",
                hir::PublicNominalKindV1::Enum => "enum",
                hir::PublicNominalKindV1::Interface => "interface",
                hir::PublicNominalKindV1::Object => "object",
            };
            (
                kind,
                format!("{}{}", declaration.name(), generics(&parameters)),
                Symbol::Imported(Imported::Nominal(id)),
            )
        };
        Some((format!("{kind} {name}"), Some(symbol)))
    }

    pub(super) fn editing_alias_declaration(
        &mut self,
        id: scoop_identity::PersistentTypeAliasId,
    ) -> Option<(String, Option<Symbol>)> {
        if let Some((&alias, _)) = self
            .query_alias_identities
            .iter()
            .find(|(_, identity)| identity.id() == id)
        {
            let declaration = &self.type_aliases[alias];
            let target = self.type_name_with_params(declaration.target, &declaration.type_params);
            let name = declaration.owner.map_or_else(
                || declaration.name.clone(),
                |owner| {
                    format!(
                        "{}.{}",
                        Owner::from_nominal_owner(owner).describe_name(self),
                        declaration.name
                    )
                },
            );
            let parameters = self.type_alias_own_parameters(alias)?;
            return Some((
                format!("typealias {name}{} = {target}", generics(parameters)),
                Some(Symbol::Current(Current::TypeAlias(alias).into())),
            ));
        }
        let declaration = self
            .dependencies
            .as_ref()?
            .type_alias_declaration(id)?
            .clone();
        let (parameters, target) = match self.imported_alias_templates.get(&id) {
            Some(template) => (template.parameters.clone(), template.target),
            None => self
                .imported_alias_declaration_types(
                    &declaration.interface,
                    &declaration.expansion,
                    ast::Span::new(0, 0),
                )
                .ok()?,
        };
        let target = self.type_name_with_params(target, &parameters);
        let own = &parameters[declaration.interface.owner_parameter_count() as usize..];
        Some((
            format!("typealias {}{} = {target}", declaration.name, generics(own)),
            Some(Symbol::Imported(Imported::TypeAlias(id))),
        ))
    }

    pub(super) fn editing_annotation_declaration(
        &self,
        id: scoop_identity::PersistentAnnotationId,
    ) -> Option<(String, Option<Symbol>)> {
        let (name, symbol) = if let Some(source) = self.source_annotations.get(&id) {
            (
                source.declaration.name.text.as_str(),
                Symbol::Current(Current::Annotation(id).into()),
            )
        } else {
            let declaration = self.dependencies.as_ref()?.annotation_declaration(id)?;
            let scoop_identity::DeclarationName::Named(name) = declaration.source.name() else {
                unreachable!("annotation declarations have names");
            };
            (name.as_str(), Symbol::Imported(Imported::Annotation(id)))
        };
        Some((format!("annotation class {name}"), Some(symbol)))
    }
}
