use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};

mod access;
mod imported;
mod signatures;

pub(crate) enum MemberSlotAccess {
    None,
    Declared,
    Override,
}

impl Lowerer {
    pub(crate) fn declaration_is_exported(access: &hir::DeclarationAccess) -> bool {
        access.declared == hir::DeclaredVisibility::Public && access.lookup.0.is_universal()
    }

    fn nominal_is_exported(access: &hir::NominalAccess) -> bool {
        access.declared == hir::DeclaredVisibility::Public && access.lookup.0.is_universal()
    }

    pub(crate) fn public_semantic_surface(&self) -> hir::PublicSemanticSurface {
        let object_backings = self
            .objects
            .iter()
            .map(|(_, declaration)| declaration.backing_class)
            .collect::<std::collections::HashSet<_>>();
        let accessor_functions = self
            .property_accessor_sources
            .iter()
            .map(|source| source.function)
            .collect::<std::collections::HashSet<_>>();
        let functions = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                (self.source_function_declarations.contains_key(&id)
                    && !accessor_functions.contains(&id)
                    && Self::declaration_is_exported(&function.access))
                .then_some(id)
            })
            .collect::<Vec<_>>();
        let generic_functions = self
            .generic_functions
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        let generic_methods = self
            .generic_methods
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        hir::PublicSemanticSurface {
            functions: functions.clone(),
            properties: self
                .properties
                .iter()
                .filter_map(|(id, property)| {
                    Self::declaration_is_exported(&property.access).then_some(id)
                })
                .collect(),
            property_getters: self
                .property_getters
                .iter()
                .filter_map(|(id, getter)| {
                    Self::declaration_is_exported(&getter.access).then_some(id)
                })
                .collect(),
            property_setters: self
                .property_setters
                .iter()
                .filter_map(|(id, setter)| {
                    Self::declaration_is_exported(&setter.access).then_some(id)
                })
                .collect(),
            generic_functions,
            generic_methods,
            structs: self
                .structs
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            struct_constructors: self
                .struct_constructors
                .iter()
                .filter_map(|(id, constructor)| {
                    Self::declaration_is_exported(&constructor.access).then_some(id)
                })
                .collect(),
            enums: self
                .enums
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            classes: self
                .classes
                .iter()
                .filter_map(|(id, declaration)| {
                    (!object_backings.contains(&id)
                        && Self::nominal_is_exported(&declaration.access))
                    .then_some(id)
                })
                .collect(),
            class_constructors: self
                .class_constructors
                .iter()
                .filter_map(|(id, constructor)| {
                    (!object_backings.contains(&constructor.owner)
                        && Self::declaration_is_exported(&constructor.access))
                    .then_some(id)
                })
                .collect(),
            interfaces: self
                .interfaces
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            interface_methods: self
                .interface_method_entities
                .iter()
                .filter_map(|(id, method)| functions.contains(&method.function).then_some(id))
                .collect(),
            objects: self
                .objects
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            object_types: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    Self::nominal_is_exported(&declaration.access)
                        .then_some(declaration.object_type)
                })
                .collect(),
            companion_relations: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    if !Self::nominal_is_exported(&declaration.access) {
                        return None;
                    }
                    match declaration.kind {
                        hir::ObjectKind::Standalone => None,
                        hir::ObjectKind::Companion(relation) => Some(relation),
                    }
                })
                .collect(),
            singleton_values: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    Self::nominal_is_exported(&declaration.access)
                        .then_some(declaration.singleton_value)
                })
                .collect(),
            type_aliases: self
                .type_aliases
                .iter()
                .filter_map(|(id, alias)| {
                    Self::declaration_is_exported(&alias.access).then_some(id)
                })
                .collect(),
        }
    }

    pub(crate) fn visibility_file(&self, file: usize) -> scoop_identity::SourceIdentity {
        self.intrinsic_sources[file].identity.clone()
    }

    pub(crate) fn normalized_visibility(syntax: ast::VisibilitySyntax) -> hir::DeclaredVisibility {
        match syntax {
            ast::VisibilitySyntax::Explicit { visibility, .. } => match visibility {
                ast::DeclaredVisibility::Public => hir::DeclaredVisibility::Public,
                ast::DeclaredVisibility::Internal => hir::DeclaredVisibility::Internal,
                ast::DeclaredVisibility::Private => hir::DeclaredVisibility::Private,
                ast::DeclaredVisibility::Protected => hir::DeclaredVisibility::Protected,
            },
            ast::VisibilitySyntax::Omitted => hir::DeclaredVisibility::Internal,
        }
    }

    pub(crate) fn top_level_domain(
        &self,
        visibility: hir::DeclaredVisibility,
        file: usize,
    ) -> hir::AccessDomain {
        let source = self.visibility_file(file);
        match visibility {
            hir::DeclaredVisibility::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibility::Internal => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::Cone(source.cone())])
            }
            hir::DeclaredVisibility::Private => hir::AccessDomain::from_constraints([
                hir::AccessConstraint::Cone(source.cone()),
                hir::AccessConstraint::File(source),
            ]),
            hir::DeclaredVisibility::Protected => {
                unreachable!("top-level protected is rejected before domain construction")
            }
        }
    }

    pub(crate) fn top_level_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        file: usize,
    ) -> hir::DeclarationAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected {
            self.error(
                span,
                format!("top-level {declaration_kind} cannot be protected"),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        hir::DeclarationAccess {
            declared,
            lookup: hir::EffectiveLookupDomain(self.top_level_domain(declared, file)),
            slot: None,
        }
    }

    pub(crate) fn nominal_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        file: usize,
    ) -> hir::NominalAccess {
        let direct = self.top_level_access(syntax, span, declaration_kind, file);
        hir::NominalAccess {
            declared: direct.declared,
            inheritance: hir::InheritanceDomain(direct.lookup.0.clone()),
            lookup: direct.lookup,
        }
    }

    pub(crate) fn nested_nominal_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        owner: Owner,
        file: usize,
    ) -> hir::NominalAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected && !matches!(owner, Owner::Class(_)) {
            self.error(
                span,
                format!(
                    "nested {declaration_kind} on {} cannot be protected",
                    owner.describe(self)
                ),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        let declared_domain = self.member_declared_domain(declared, owner, file);
        let effective = declared_domain.intersect(self.owner_lookup_domain(owner));
        hir::NominalAccess {
            declared,
            inheritance: hir::InheritanceDomain(effective.clone()),
            lookup: hir::EffectiveLookupDomain(effective),
        }
    }

    fn owner_visibility(&self, owner: Owner) -> hir::VisibilityOwner {
        match owner {
            Owner::Class(id) => hir::VisibilityOwner::Class(id),
            Owner::Interface(id) => hir::VisibilityOwner::Interface(id),
            Owner::Struct(id) => hir::VisibilityOwner::Struct(id),
            Owner::Enum(id) => hir::VisibilityOwner::Enum(id),
            Owner::Object(id) => hir::VisibilityOwner::Object(id),
        }
    }

    pub(crate) fn owner_lookup_domain(&self, owner: Owner) -> &hir::AccessDomain {
        match owner {
            Owner::Class(id) => &self.classes[id].access.lookup.0,
            Owner::Interface(id) => &self.interfaces[id].access.lookup.0,
            Owner::Struct(id) => &self.structs[id].access.lookup.0,
            Owner::Enum(id) => &self.enums[id].access.lookup.0,
            Owner::Object(id) => &self.objects[id].access.lookup.0,
        }
    }

    pub(crate) fn member_declared_domain(
        &self,
        visibility: hir::DeclaredVisibility,
        owner: Owner,
        file: usize,
    ) -> hir::AccessDomain {
        let source = self.visibility_file(file);
        match visibility {
            hir::DeclaredVisibility::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibility::Internal => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::Cone(source.cone())])
            }
            hir::DeclaredVisibility::Private => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::LexicalOwner(
                    self.owner_visibility(owner),
                )])
            }
            hir::DeclaredVisibility::Protected => {
                let Owner::Class(class) = owner else {
                    unreachable!("non-class protected is rejected before domain construction")
                };
                hir::AccessDomain::from_constraints([hir::AccessConstraint::SubclassesOf(class)])
            }
        }
    }

    pub(crate) fn member_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        owner: Owner,
        file: usize,
        slot_access: MemberSlotAccess,
    ) -> hir::DeclarationAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected && !matches!(owner, Owner::Class(_)) {
            self.error(
                span,
                format!(
                    "{declaration_kind} on {} cannot be protected",
                    owner.describe(self)
                ),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        let declared_domain = self.member_declared_domain(declared, owner, file);
        let effective = declared_domain.intersect(self.owner_lookup_domain(owner));
        let slot = match slot_access {
            MemberSlotAccess::None => None,
            MemberSlotAccess::Declared => Some(hir::SlotContractDomain(effective.clone())),
            MemberSlotAccess::Override => Some(hir::SlotContractDomain(declared_domain)),
        };
        hir::DeclarationAccess {
            declared,
            lookup: hir::EffectiveLookupDomain(effective),
            slot,
        }
    }

    pub(crate) fn fixed_representation_access(&self, owner: Owner) -> hir::DeclarationAccess {
        hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Public,
            lookup: hir::EffectiveLookupDomain(self.owner_lookup_domain(owner).clone()),
            slot: None,
        }
    }

    pub(crate) fn local_declaration_access(&self) -> hir::DeclarationAccess {
        let source = self.visibility_file(self.current_file);
        hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Private,
            lookup: hir::EffectiveLookupDomain(hir::AccessDomain::from_constraints([
                hir::AccessConstraint::Cone(source.cone()),
                hir::AccessConstraint::File(source),
            ])),
            slot: None,
        }
    }

    fn owner_parent(&self, owner: Owner) -> Option<Owner> {
        let parent = match owner {
            Owner::Class(id) => self.classes[id].owner,
            Owner::Interface(id) => self.interfaces[id].owner,
            Owner::Struct(id) => self.structs[id].owner,
            Owner::Enum(id) => self.enums[id].owner,
            Owner::Object(id) => self.objects[id].owner,
        }?;
        Some(Owner::from_nominal_owner(parent))
    }

    fn lexical_owner_contains(&self, mut current: Owner, required: hir::VisibilityOwner) -> bool {
        loop {
            if self.owner_visibility(current) == required {
                return true;
            }
            let Some(parent) = self.owner_parent(current) else {
                return false;
            };
            current = parent;
        }
    }

    fn visibility_owner_is_within(
        &self,
        current: hir::VisibilityOwner,
        required: hir::VisibilityOwner,
    ) -> bool {
        let current = match current {
            hir::VisibilityOwner::Class(id) => Owner::Class(id),
            hir::VisibilityOwner::Interface(id) => Owner::Interface(id),
            hir::VisibilityOwner::Struct(id) => Owner::Struct(id),
            hir::VisibilityOwner::Enum(id) => Owner::Enum(id),
            hir::VisibilityOwner::Object(id) => Owner::Object(id),
        };
        self.lexical_owner_contains(current, required)
    }

    fn owner_definition_file(&self, owner: hir::VisibilityOwner) -> scoop_identity::SourceIdentity {
        let file = match owner {
            hir::VisibilityOwner::Class(id) => self.class_files[&id],
            hir::VisibilityOwner::Interface(id) => self.interface_files[&id],
            hir::VisibilityOwner::Struct(id) => self.struct_files[&id],
            hir::VisibilityOwner::Enum(id) => self.enum_files[&id],
            hir::VisibilityOwner::Object(id) => self.object_files[&id],
        };
        self.visibility_file(file)
    }

    fn constraint_implies(
        &self,
        narrower: &hir::AccessConstraint,
        wider: &hir::AccessConstraint,
    ) -> bool {
        if narrower == wider {
            return true;
        }
        match (narrower, wider) {
            (hir::AccessConstraint::File(source), hir::AccessConstraint::Cone(cone)) => {
                source.cone() == *cone
            }
            (hir::AccessConstraint::LexicalOwner(owner), hir::AccessConstraint::Cone(cone)) => {
                self.owner_definition_file(*owner).cone() == *cone
            }
            (hir::AccessConstraint::LexicalOwner(owner), hir::AccessConstraint::File(file)) => {
                self.owner_definition_file(*owner) == *file
            }
            (
                hir::AccessConstraint::LexicalOwner(current),
                hir::AccessConstraint::LexicalOwner(required),
            ) => self.visibility_owner_is_within(*current, *required),
            (
                hir::AccessConstraint::LexicalOwner(owner),
                hir::AccessConstraint::SubclassesOf(base),
            ) => self.lexical_scope_implies_subclass(*owner, *base),
            (
                hir::AccessConstraint::SubclassesOf(derived),
                hir::AccessConstraint::SubclassesOf(base),
            ) => self.class_is_same_or_subclass_of(*derived, *base),
            (
                hir::AccessConstraint::SubclassesOf(derived),
                hir::AccessConstraint::ImportedSubclassesOf(base),
            ) => self.class_inherits_imported(*derived, *base),
            (
                hir::AccessConstraint::LexicalOwner(owner),
                hir::AccessConstraint::ImportedSubclassesOf(base),
            ) => {
                let owner = match owner {
                    hir::VisibilityOwner::Class(id) => Owner::Class(*id),
                    hir::VisibilityOwner::Interface(id) => Owner::Interface(*id),
                    hir::VisibilityOwner::Struct(id) => Owner::Struct(*id),
                    hir::VisibilityOwner::Enum(id) => Owner::Enum(*id),
                    hir::VisibilityOwner::Object(id) => Owner::Object(*id),
                };
                self.protected_scope_classes(owner)
                    .any(|class| self.class_inherits_imported(class, *base))
            }
            (
                hir::AccessConstraint::ImportedSubclassesOf(derived),
                hir::AccessConstraint::ImportedSubclassesOf(base),
            ) => self.imported_class_is_same_or_subclass_of(*derived, *base),
            _ => false,
        }
    }

    /// Set containment used by override, signature and default-access proofs.
    /// `narrower` is a subset of `wider` when every wider constraint is
    /// implied by at least one constraint of the narrower region.
    pub(crate) fn access_domain_is_subset(
        &self,
        narrower: &hir::AccessDomain,
        wider: &hir::AccessDomain,
    ) -> bool {
        if narrower.is_empty() {
            return true;
        }
        if wider.is_empty() {
            return false;
        }
        wider.constraints().iter().all(|wider_constraint| {
            narrower.constraints().iter().any(|narrower_constraint| {
                self.constraint_implies(narrower_constraint, wider_constraint)
            })
        })
    }

    fn intrinsic_type_access_domain(
        &self,
        kind: hir::IntrinsicTypeKind,
    ) -> Option<hir::AccessDomain> {
        let &(owner, _) = self.intrinsic_type_owners.get(&kind)?;
        Some(match owner {
            crate::IntrinsicTypeOwner::Struct(owner) => self.structs[owner].access.lookup.0.clone(),
            crate::IntrinsicTypeOwner::Class(owner) => self.classes[owner].access.lookup.0.clone(),
        })
    }

    fn collect_type_dependencies(
        &self,
        ty: hir::TypeId,
        dependencies: &mut Vec<(hir::TypeId, hir::AccessDomain)>,
    ) {
        let provided = match self.types[ty] {
            hir::Type::ImportedStruct(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedEnum(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedClass(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedInterface(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::Integer(kind) => {
                self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::Integer(kind))
            }
            hir::Type::Boolean => {
                self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::Boolean)
            }
            hir::Type::String => self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::String),
            hir::Type::Struct(application) => Some(
                self.structs[self.struct_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Enum(application) => Some(
                self.enums[self.enum_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Class(application) => Some(
                self.classes[self.class_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Interface(application) => Some(
                self.interfaces[self.interface_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Ptr(_) => self
                .ffi_ptr
                .map(|owner| self.structs[owner].access.lookup.0.clone()),
            hir::Type::FunPtr(_) => self
                .ffi_fun_ptr
                .map(|owner| self.structs[owner].access.lookup.0.clone()),
            hir::Type::Unit
            | hir::Type::Any
            | hir::Type::Tuple(_)
            | hir::Type::Function(_)
            | hir::Type::Param(_) => None,
        };
        if let Some(provided) = provided
            && !dependencies.iter().any(|(dependency, _)| *dependency == ty)
        {
            dependencies.push((ty, provided));
        }

        match self.types[ty].clone() {
            hir::Type::Struct(application) => {
                for argument in self.struct_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Enum(application) => {
                for argument in self.enum_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Class(application) => {
                for argument in self.class_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Interface(application) => {
                for argument in self.interface_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Tuple(elements) => {
                for element in elements {
                    self.collect_type_dependencies(element, dependencies);
                }
            }
            hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                let function = self.function_types[function].clone();
                for parameter in function.parameter_types {
                    self.collect_type_dependencies(parameter, dependencies);
                }
                self.collect_type_dependencies(function.return_type, dependencies);
            }
            hir::Type::Ptr(pointee) => self.collect_type_dependencies(pointee, dependencies),
            hir::Type::ImportedStruct(_)
            | hir::Type::ImportedEnum(_)
            | hir::Type::ImportedClass(_)
            | hir::Type::ImportedInterface(_)
            | hir::Type::Unit
            | hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Any
            | hir::Type::Param(_) => {}
        }
    }

    pub(crate) fn type_access_domain(&self, ty: hir::TypeId) -> hir::AccessDomain {
        let mut dependencies = Vec::new();
        self.collect_type_dependencies(ty, &mut dependencies);
        dependencies
            .into_iter()
            .fold(hir::AccessDomain::universal(), |domain, (_, dependency)| {
                domain.intersect(&dependency)
            })
    }

    pub(crate) fn default_call_domain(&self, owner: hir::ExportParameterOwner) -> hir::CallDomain {
        let access = match owner {
            hir::ExportParameterOwner::Function(function) => &self.functions[function].access,
            hir::ExportParameterOwner::StructConstructor(constructor) => {
                &self.struct_constructors[constructor].access
            }
            hir::ExportParameterOwner::ClassConstructor(constructor) => {
                &self.class_constructors[constructor].access
            }
            hir::ExportParameterOwner::VariantConstructor(variant) => {
                return hir::CallDomain {
                    direct: self.enums[variant.enumeration()].access.lookup.clone(),
                    slot: None,
                };
            }
        };
        hir::CallDomain {
            direct: access.lookup.clone(),
            slot: access.slot.clone(),
        }
    }

    pub(crate) fn function_access_domain(&self, function: hir::FunctionId) -> hir::AccessDomain {
        self.functions[function].access.lookup.0.clone()
    }

    pub(crate) fn constructor_access_domain(
        &self,
        target: hir::ExportDefaultConstructorTarget,
    ) -> hir::AccessDomain {
        match target {
            hir::ExportDefaultConstructorTarget::Imported { .. } => hir::AccessDomain::universal(),
            hir::ExportDefaultConstructorTarget::Struct(application) => {
                let constructor = self.struct_constructor_applications[application].constructor;
                self.struct_constructors[constructor]
                    .access
                    .lookup
                    .0
                    .clone()
            }
            hir::ExportDefaultConstructorTarget::Class(application) => {
                let constructor = self.class_constructor_applications[application].constructor;
                self.class_constructors[constructor].access.lookup.0.clone()
            }
            hir::ExportDefaultConstructorTarget::Variant(variant) => {
                self.type_access_domain(variant.owner)
            }
        }
    }

    pub(crate) fn field_access_domain(&self, target: hir::FieldRef) -> hir::AccessDomain {
        match target {
            hir::FieldRef::ClassField { owner, field } => match self.types[owner] {
                hir::Type::Class(_) => {
                    let field = self
                        .field_identity_builder
                        .class_declaration(field)
                        .expect("a current class field has its declaration identity");
                    self.properties[self.class_fields[field].property]
                        .access
                        .lookup
                        .0
                        .clone()
                }
                hir::Type::ImportedClass(_) => hir::AccessDomain::universal(),
                _ => unreachable!("a class field retains its declaring class"),
            },
            hir::FieldRef::StructField { owner, .. } => match self.types[owner] {
                hir::Type::Struct(application) => self.structs
                    [self.struct_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
                hir::Type::ImportedStruct(_) => hir::AccessDomain::universal(),
                _ => unreachable!("a struct field retains its declaring struct"),
            },
            hir::FieldRef::TupleIndex(_) => hir::AccessDomain::universal(),
        }
    }
}
