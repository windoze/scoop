use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};

mod access;

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
            signature: Vec::new(),
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
            signature: Vec::new(),
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
            signature: Vec::new(),
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
            signature: Vec::new(),
        }
    }

    pub(crate) fn fixed_representation_access(&self, owner: Owner) -> hir::DeclarationAccess {
        hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Public,
            lookup: hir::EffectiveLookupDomain(self.owner_lookup_domain(owner).clone()),
            slot: None,
            signature: Vec::new(),
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
            signature: Vec::new(),
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
                let enumeration = self.enum_applications[variant.application()].template;
                self.enums[enumeration].access.lookup.0.clone()
            }
        }
    }

    pub(crate) fn field_access_domain(&self, target: hir::FieldRef) -> hir::AccessDomain {
        match target {
            hir::FieldRef::ClassField { field, .. } => self.properties
                [self.class_fields[field].property]
                .access
                .lookup
                .0
                .clone(),
            hir::FieldRef::StructField(field) => {
                let owner = self.struct_applications[field.application()].template;
                self.structs[owner].access.lookup.0.clone()
            }
            hir::FieldRef::ImportedStruct { .. } | hir::FieldRef::TupleIndex(_) => {
                hir::AccessDomain::universal()
            }
        }
    }

    fn type_parameter_signature_types(
        &self,
        parameters: &[hir::TypeParamDecl],
    ) -> Vec<hir::TypeId> {
        let mut result = Vec::new();
        for parameter in parameters {
            for bound in parameter.nominal_bounds_in_source_order() {
                let ty = match bound {
                    hir::NominalBoundRef::Class(bound) => {
                        self.class_applications[bound.application].canonical_type
                    }
                    hir::NominalBoundRef::Interface(bound) => {
                        self.interface_applications[bound.application].canonical_type
                    }
                };
                result.push(ty);
            }
        }
        result
    }

    pub(crate) fn signature_exposure_witnesses(
        &mut self,
        access: &hir::DeclarationAccess,
        signature_types: &[hir::TypeId],
        span: ast::Span,
        declaration: &str,
    ) -> Vec<hir::SignatureExposureWitness> {
        let mut requirements = vec![access.lookup.0.clone()];
        if let Some(slot) = &access.slot
            && !requirements.contains(&slot.0)
        {
            requirements.push(slot.0.clone());
        }
        let mut dependencies = Vec::new();
        for &ty in signature_types {
            self.collect_type_dependencies(ty, &mut dependencies);
        }
        let mut witnesses = Vec::new();
        for (dependency, provided) in dependencies {
            for required in &requirements {
                if !self.access_domain_is_subset(required, &provided) {
                    let dependency_name = self.type_name(dependency);
                    self.error(
                        span,
                        format!(
                            "signature of {declaration} exposes type `{dependency_name}` outside its access domain"
                        ),
                    );
                    continue;
                }
                witnesses.push(hir::SignatureExposureWitness {
                    dependency,
                    required: required.clone(),
                    provided: provided.clone(),
                });
            }
        }
        witnesses
    }

    pub(crate) fn validate_signature_exposure(&mut self) {
        let functions = self
            .signatures
            .iter()
            .map(|(&id, signature)| {
                let mut types = self.type_parameter_signature_types(&signature.type_params);
                types.extend(signature.params.iter().map(|parameter| parameter.ty));
                types.push(signature.return_ty);
                (id, types)
            })
            .collect::<Vec<_>>();
        for (id, types) in functions {
            self.current_file = self.function_files[&id];
            let mut access = self.functions[id].access.clone();
            let name = self.functions[id].name.clone();
            access.signature = self.signature_exposure_witnesses(
                &access,
                &types,
                self.functions[id].span,
                &format!("function `{name}`"),
            );
            self.functions[id].access = access;
        }

        let properties = self
            .properties
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value.ty,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                    value.owner,
                )
            })
            .collect::<Vec<_>>();
        for (id, ty, span, name, mut access, owner) in properties {
            self.current_file = match owner {
                hir::PropertyOwner::TopLevel => self.property_files[&id],
                hir::PropertyOwner::Extension(_) => self.property_files[&id],
                hir::PropertyOwner::Class(owner) => self.class_files[&owner],
                hir::PropertyOwner::Struct(owner) => self.struct_files[&owner],
                hir::PropertyOwner::Enum(owner) => self.enum_files[&owner],
                hir::PropertyOwner::Interface(owner) => self.interface_files[&owner],
                hir::PropertyOwner::Object(owner) => self.object_files[&owner],
            };
            let mut signature_types = vec![ty];
            if let hir::PropertyOwner::Extension(extension) = owner {
                signature_types.insert(0, self.extension_properties[extension].receiver_ty);
            }
            access.signature = self.signature_exposure_witnesses(
                &access,
                &signature_types,
                span,
                &format!("property `{name}`"),
            );
            self.properties[id].access = access;

            let getter = self.properties[id].capability.getter();
            let mut getter_access = self.property_getters[getter].access.clone();
            getter_access.signature = self.signature_exposure_witnesses(
                &getter_access,
                &[ty],
                self.property_getters[getter].span,
                &format!("getter of property `{name}`"),
            );
            self.property_getters[getter].access = getter_access;

            if let Some(setter) = self.properties[id].capability.setter() {
                let mut setter_access = self.property_setters[setter].access.clone();
                setter_access.signature = self.signature_exposure_witnesses(
                    &setter_access,
                    &[ty],
                    self.property_setters[setter].span,
                    &format!("setter of property `{name}`"),
                );
                self.property_setters[setter].access = setter_access;
            }
        }

        let class_constructors = self
            .class_constructors
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect::<Vec<_>>(),
                    value.span,
                    self.classes[value.owner].name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, owner, mut access) in class_constructors {
            self.current_file = self.class_files[&self.class_constructors[id].owner];
            access.signature = self.signature_exposure_witnesses(
                &access,
                &types,
                span,
                &format!("constructor of class `{owner}`"),
            );
            self.class_constructors[id].access = access;
        }

        let struct_constructors = self
            .struct_constructors
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect::<Vec<_>>(),
                    value.span,
                    self.structs[value.owner].name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, owner, mut access) in struct_constructors {
            self.current_file = self.struct_files[&self.struct_constructors[id].owner];
            access.signature = self.signature_exposure_witnesses(
                &access,
                &types,
                span,
                &format!("constructor of struct `{owner}`"),
            );
            self.struct_constructors[id].access = access;
        }

        self.validate_nominal_signature_exposure();
    }

    fn validate_nominal_signature_exposure(&mut self) {
        let structs = self
            .structs
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(value.semantic_fields().iter().map(|field| field.ty));
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, mut access) in structs {
            self.current_file = self.struct_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
                signature: Vec::new(),
            };
            access.signature = self.signature_exposure_witnesses(
                &declaration,
                &types,
                span,
                &format!("struct `{name}`"),
            );
            self.structs[id].access = access;
        }

        let enums = self
            .enums
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(
                    value
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty)),
                );
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, mut access) in enums {
            self.current_file = self.enum_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
                signature: Vec::new(),
            };
            access.signature = self.signature_exposure_witnesses(
                &declaration,
                &types,
                span,
                &format!("enum `{name}`"),
            );
            self.enums[id].access = access;
        }

        let classes = self
            .classes
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(value.base_class);
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, mut access) in classes {
            self.current_file = self.class_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
                signature: Vec::new(),
            };
            access.signature = self.signature_exposure_witnesses(
                &declaration,
                &types,
                span,
                &format!("class `{name}`"),
            );
            self.classes[id].access = access;
        }

        let interfaces = self
            .interfaces
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(
                    value
                        .parents
                        .iter()
                        .map(|parent| self.interface_applications[*parent].canonical_type),
                );
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, mut access) in interfaces {
            self.current_file = self.interface_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
                signature: Vec::new(),
            };
            access.signature = self.signature_exposure_witnesses(
                &declaration,
                &types,
                span,
                &format!("interface `{name}`"),
            );
            self.interfaces[id].access = access;
        }
    }
}
