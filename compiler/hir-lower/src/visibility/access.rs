use super::*;

impl Lowerer {
    pub(crate) fn imported_nominal_access_domain(
        &self,
        declaration: &hir::ImportedNominalDeclaration,
    ) -> hir::AccessDomain {
        match declaration
            .interface
            .declaration_details()
            .declared_visibility()
        {
            hir::DeclaredVisibilityV1::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibilityV1::Internal => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::Cone(
                    declaration.identity.key().origin(),
                )])
            }
            hir::DeclaredVisibilityV1::Private | hir::DeclaredVisibilityV1::Protected => {
                hir::AccessDomain::empty()
            }
        }
    }

    pub(super) fn protected_scope_classes(
        &self,
        owner: Owner,
    ) -> impl Iterator<Item = hir::ClassId> + '_ {
        std::iter::successors(Some(owner), |owner| self.owner_parent(*owner)).filter_map(|owner| {
            match owner {
                Owner::Class(class) => Some(class),
                Owner::Object(object) => Some(self.objects[object].backing_class),
                Owner::Interface(_) | Owner::Struct(_) | Owner::Enum(_) => None,
            }
        })
    }

    pub(crate) fn protected_access_class(&self, base: hir::ClassId) -> Option<hir::ClassId> {
        self.protected_scope_classes(self.current_owner?)
            .find(|class| self.class_is_same_or_subclass_of(*class, base))
    }

    pub(super) fn lexical_scope_implies_subclass(
        &self,
        owner: hir::VisibilityOwner,
        base: hir::ClassId,
    ) -> bool {
        let owner = match owner {
            hir::VisibilityOwner::Class(id) => Owner::Class(id),
            hir::VisibilityOwner::Interface(id) => Owner::Interface(id),
            hir::VisibilityOwner::Struct(id) => Owner::Struct(id),
            hir::VisibilityOwner::Enum(id) => Owner::Enum(id),
            hir::VisibilityOwner::Object(id) => Owner::Object(id),
        };
        self.protected_scope_classes(owner)
            .any(|class| self.class_is_same_or_subclass_of(class, base))
    }

    pub(crate) fn class_is_same_or_subclass_of(
        &self,
        mut class: hir::ClassId,
        base: hir::ClassId,
    ) -> bool {
        let mut seen = Vec::new();
        loop {
            if class == base {
                return true;
            }
            if seen.contains(&class) {
                return false;
            }
            seen.push(class);
            let Some(base_ty) = self.classes[class].base_class else {
                return false;
            };
            let hir::Type::Class(application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            class = self.class_applications[application].template;
        }
    }

    pub(crate) fn receiver_class(&self, ty: hir::TypeId) -> Option<hir::ClassId> {
        match self.types[ty] {
            hir::Type::Class(application) => Some(self.class_applications[application].template),
            _ => None,
        }
    }

    pub(crate) fn access_domain_allows(&self, domain: &hir::AccessDomain) -> bool {
        if domain.is_empty() {
            return false;
        }
        let site = self.visibility_file(self.current_file);
        domain
            .constraints()
            .iter()
            .all(|constraint| match constraint {
                hir::AccessConstraint::Cone(cone) => site.cone() == *cone,
                hir::AccessConstraint::File(file) => site == *file,
                hir::AccessConstraint::LexicalOwner(owner) => self
                    .current_owner
                    .is_some_and(|current| self.lexical_owner_contains(current, *owner)),
                hir::AccessConstraint::SubclassesOf(base) => {
                    self.current_owner.is_some_and(|owner| {
                        self.protected_scope_classes(owner)
                            .any(|current| self.class_is_same_or_subclass_of(current, *base))
                    })
                }
            })
    }

    pub(crate) fn declaration_access_allows(
        &self,
        access: &hir::DeclarationAccess,
        declaring_class: Option<hir::ClassId>,
        explicit_receiver: Option<hir::TypeId>,
    ) -> bool {
        if !self.access_domain_allows(&access.lookup.0) {
            return false;
        }
        if access.declared != hir::DeclaredVisibility::Protected {
            return true;
        }
        let Some(receiver) = explicit_receiver else {
            return true;
        };
        let (Some(base), Some(scope), Some(receiver)) = (
            declaring_class,
            self.current_owner,
            self.receiver_class(receiver),
        ) else {
            return false;
        };
        self.protected_scope_classes(scope).any(|current| {
            self.class_is_same_or_subclass_of(current, base)
                && self.class_is_same_or_subclass_of(receiver, current)
        })
    }

    pub(crate) fn property_is_accessible(
        &self,
        property: hir::PropertyId,
        receiver: Option<hir::TypeId>,
    ) -> bool {
        self.property_accessor_is_accessible(property, &self.properties[property].access, receiver)
    }

    pub(crate) fn property_accessor_is_accessible(
        &self,
        property: hir::PropertyId,
        access: &hir::DeclarationAccess,
        receiver: Option<hir::TypeId>,
    ) -> bool {
        let owner = match self.properties[property].owner {
            hir::PropertyOwner::Class(class) => Some(class),
            hir::PropertyOwner::Object(object) => Some(self.objects[object].backing_class),
            hir::PropertyOwner::Interface(_)
            | hir::PropertyOwner::Struct(_)
            | hir::PropertyOwner::Enum(_)
            | hir::PropertyOwner::TopLevel
            | hir::PropertyOwner::Extension(_) => None,
        };
        self.declaration_access_allows(access, owner, receiver)
    }
    pub(crate) fn function_is_accessible(
        &self,
        function: hir::FunctionId,
        explicit_receiver: Option<hir::TypeId>,
    ) -> bool {
        self.declaration_access_allows(
            &self.functions[function].access,
            self.functions[function]
                .method
                .and_then(|method| self.receiver_class(method.owner)),
            explicit_receiver,
        )
    }

    pub(crate) fn function_lookup_witness(
        &self,
        function: hir::FunctionId,
    ) -> hir::LookupAccessWitness {
        hir::LookupAccessWitness {
            declaration: hir::AccessDeclaration::Function(function),
            domain: self.functions[function].access.lookup.clone(),
            site: self.visibility_file(self.current_file),
        }
    }

    pub(crate) fn nominal_is_accessible(&self, ty: hir::TypeId) -> bool {
        let domain = match self.types[ty] {
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
        domain.is_none_or(|domain| self.access_domain_allows(&domain))
    }

    pub(crate) fn enum_is_accessible(&self, enumeration: hir::EnumId) -> bool {
        self.access_domain_allows(&self.enums[enumeration].access.lookup.0)
    }

    pub(crate) fn constructor_is_accessible(
        &self,
        source: crate::call_resolution::candidates::NominalConstructorSource,
    ) -> bool {
        use crate::call_resolution::candidates::NominalConstructorSource;
        let domain = match source {
            NominalConstructorSource::Struct(constructor) => {
                &self.struct_constructors[constructor].access.lookup.0
            }
            NominalConstructorSource::Class(constructor) => {
                &self.class_constructors[constructor].access.lookup.0
            }
            NominalConstructorSource::IntrinsicClass(class) => &self.classes[class].access.lookup.0,
            NominalConstructorSource::Variant(variant) => {
                &self.enums[variant.enumeration()].access.lookup.0
            }
        };
        self.access_domain_allows(domain)
    }
}
