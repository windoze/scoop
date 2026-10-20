use super::*;
use hir::ImportedCallableSource;
use scoop_identity::BindingNamespace;

impl Lowerer {
    pub(super) fn documentation_members(
        &mut self,
        symbol: Symbol,
        name: &str,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Vec<Symbol> {
        let Some(ty) = self.documentation_symbol_type(symbol, world) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        if let Some(target) = self.nominal_target_for_type(ty) {
            let derived = match target {
                crate::NominalTarget::Struct(id) => self.structs[id].derived_equality,
                crate::NominalTarget::Enum(id) => self.enums[id].derived_equality,
                _ => None,
            };
            if let Some(function) = derived
                && self.source_function_name(function) == name
                && self.function_is_accessible(function, Some(ty))
            {
                result.push(Symbol::Current(Target::DerivedFunction(function)));
            }
            let targets = self
                .imports
                .documentation_static_targets(self, target.owner(), name);
            result.extend(self.documentation_named_targets(targets, world));
            if let Some(alias) = self
                .nested_aliases_by_owner
                .get(&(target.owner(), name.into()))
                .copied()
                && self.source_type_alias_is_accessible(alias)
            {
                result.extend(self.documentation_named_targets(
                    vec![NamedCallTarget::Type(TopLevelTypeTarget::Alias(alias))],
                    world,
                ));
            }
            if let Some(nested) = self.nested_nominal_target(target.owner(), name)
                && self.top_level_type_target_is_accessible(TopLevelTypeTarget::Nominal(nested))
            {
                result.push(Symbol::Current(
                    Source::Nominal(nested.owner().as_nominal_owner()).into(),
                ));
            }
            if let crate::NominalTarget::Struct(id) = target
                && let Some(field) = self.structs[id]
                    .semantic_fields()
                    .iter()
                    .position(|field| field.name == name)
                && let Some(field) = hir::StructFieldRef::checked(&self.structs, id, field as u32)
            {
                // The automatically exposed property is the field's access
                // mechanism, not a second source declaration.
                return vec![Symbol::Current(Source::Field(field).into())];
            }
        } else if let Some(owner) = self.imported_nominal_owner(ty) {
            let targets = [BindingNamespace::Type, BindingNamespace::Value]
                .into_iter()
                .flat_map(|namespace| {
                    self.dependencies
                        .as_ref()
                        .into_iter()
                        .flat_map(move |dependencies| {
                            dependencies.static_bindings(owner, namespace, name)
                        })
                        .map(|binding| NamedCallTarget::ImportedDependency(binding.target()))
                })
                .collect();
            result.extend(self.documentation_named_targets(targets, world));
            if let Some(hir::FieldRef::StructField { field, .. }) =
                self.struct_field(ty, name).map(|field| field.reference)
            {
                return vec![Symbol::Imported(Imported::Field(field))];
            }
        }
        result.extend(self.methods_by_name(ty, name).into_iter().map(|candidate| {
            Symbol::Current(self.documentation_function_target(candidate.function))
        }));
        if let Some((property, _, _)) = self.find_accessible_nominal_property(ty, name) {
            result.push(Symbol::Current(Source::Property(property).into()));
        }
        if let Ok(methods) =
            self.imported_member_candidates(ty, hir::ImportedMemberLookup::Name(name))
        {
            result.extend(methods.into_iter().map(|method| {
                Symbol::Imported(targets::imported_function(method.interface().declaration()))
            }));
        }
        if let Ok(getters) =
            self.imported_member_candidates(ty, hir::ImportedMemberLookup::PropertyGetter(name))
        {
            for getter in getters {
                let scoop_identity::CallableTemplateOrigin::Accessor(accessor) =
                    getter.interface().declaration()
                else {
                    continue;
                };
                let Some(property) = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.property_for_accessor(accessor))
                else {
                    continue;
                };
                let property = match property.declaration() {
                    scoop_identity::PropertyOwner::Property(id) => Imported::Property(id),
                    scoop_identity::PropertyOwner::ExtensionProperty(id) => {
                        Imported::ExtensionProperty(id)
                    }
                };
                result.push(Symbol::Imported(property));
            }
        }
        result
    }
}
