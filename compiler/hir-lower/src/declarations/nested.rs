use super::*;

pub(crate) struct NestedDeclarationQueues<'queues, 'source> {
    pub(crate) annotations:
        &'queues mut Vec<(&'source ast::AnnotationClassDecl, usize, Option<Owner>)>,
    pub(crate) structs: &'queues mut Vec<(StructId, ast::StructSource<'source>, usize)>,
    pub(crate) enums: &'queues mut Vec<(EnumId, &'source ast::EnumDecl, usize)>,
    pub(crate) classes: &'queues mut Vec<(ClassId, ast::ClassSource<'source>, usize)>,
    pub(crate) interfaces: &'queues mut Vec<(InterfaceId, &'source ast::InterfaceDecl, usize)>,
    pub(crate) objects: &'queues mut Vec<(ObjectId, super::objects::ObjectSource<'source>, usize)>,
    pub(crate) methods: &'queues mut Vec<(FunctionId, &'source ast::FunctionDecl, usize, Owner)>,
}

impl Lowerer {
    pub(crate) fn declare_class_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: ast::ClassSource<'a>,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for member in declaration.members {
            match member {
                ast::ClassMember::Nested(nested) => {
                    self.declare_nested_nominal(owner, nested, queues, file)
                }
                ast::ClassMember::Companion(companion) => {
                    if let Some(id) = self.declare_companion(
                        companion,
                        queues.objects,
                        queues.methods,
                        file,
                        owner,
                    ) {
                        self.declare_object_nested(
                            Owner::Object(id),
                            super::objects::ObjectSource::Companion(companion),
                            queues,
                            file,
                        );
                    }
                }
                ast::ClassMember::StoredProperty(_)
                | ast::ClassMember::InitBlock(_)
                | ast::ClassMember::ReleaseBlock(_)
                | ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::Function(_) => {}
            }
        }
        for nested in &declaration.recovery.nested {
            self.declare_recovered_nested_nominal(owner, nested, queues, file);
        }
    }

    pub(crate) fn declare_struct_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: ast::StructSource<'a>,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for member in declaration.members {
            match member {
                ast::StructMember::Nested(nested) => {
                    self.declare_nested_nominal(owner, nested, queues, file)
                }
                ast::StructMember::Companion(companion) => {
                    if let Some(id) = self.declare_companion(
                        companion,
                        queues.objects,
                        queues.methods,
                        file,
                        owner,
                    ) {
                        self.declare_object_nested(
                            Owner::Object(id),
                            super::objects::ObjectSource::Companion(companion),
                            queues,
                            file,
                        );
                    }
                }
                ast::StructMember::SecondaryConstructor(_)
                | ast::StructMember::Function(_)
                | ast::StructMember::Property(_) => {}
            }
        }
        for nested in &declaration.recovery.nested {
            self.declare_recovered_nested_nominal(owner, nested, queues, file);
        }
    }

    pub(crate) fn declare_enum_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::EnumDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for nested in &declaration.nested {
            self.declare_nested_nominal(owner, nested, queues, file);
        }
        if let Some(companion) = &declaration.companion {
            if let Some(id) =
                self.declare_companion(companion, queues.objects, queues.methods, file, owner)
            {
                self.declare_object_nested(
                    Owner::Object(id),
                    super::objects::ObjectSource::Companion(companion),
                    queues,
                    file,
                );
            }
        }
    }

    pub(crate) fn declare_interface_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::InterfaceDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for nested in &declaration.nested {
            self.declare_nested_nominal(owner, nested, queues, file);
        }
        if let Some(companion) = &declaration.companion {
            if let Some(id) =
                self.declare_companion(companion, queues.objects, queues.methods, file, owner)
            {
                self.declare_object_nested(
                    Owner::Object(id),
                    super::objects::ObjectSource::Companion(companion),
                    queues,
                    file,
                );
            }
        }
    }

    pub(crate) fn declare_object_nested<'a>(
        &mut self,
        owner: Owner,
        source: super::objects::ObjectSource<'a>,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for member in source.members() {
            match member {
                ast::ClassMember::Nested(nested) => {
                    self.declare_nested_nominal(owner, nested, queues, file)
                }
                ast::ClassMember::Companion(companion) => {
                    if let Some(id) = self.declare_companion(
                        companion,
                        queues.objects,
                        queues.methods,
                        file,
                        owner,
                    ) {
                        self.declare_object_nested(
                            Owner::Object(id),
                            super::objects::ObjectSource::Companion(companion),
                            queues,
                            file,
                        );
                    }
                }
                ast::ClassMember::StoredProperty(_)
                | ast::ClassMember::InitBlock(_)
                | ast::ClassMember::ReleaseBlock(_)
                | ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::Function(_) => {}
            }
        }
    }

    fn declare_nested_nominal<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::NestedTypeDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        let is_core = self.source_is_core(file);
        match declaration {
            ast::NestedTypeDecl::TypeAlias(source) => {
                self.declare_nested_type_alias(source, file, owner);
            }
            ast::NestedTypeDecl::AnnotationClass(source) => {
                queues.annotations.push((source, file, Some(owner)))
            }
            ast::NestedTypeDecl::Struct(source) => {
                if let Some(id) = self.declare_struct(
                    source.source(),
                    queues.structs,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_struct_nested(Owner::Struct(id), source.source(), queues, file);
                }
            }
            ast::NestedTypeDecl::Enum(source) => {
                if let Some(id) = self.declare_enum(
                    source,
                    is_core,
                    queues.enums,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_enum_nested(Owner::Enum(id), source, queues, file);
                }
            }
            ast::NestedTypeDecl::Class(source) => {
                if let Some(id) = self.declare_class(
                    source.source(),
                    is_core,
                    queues.classes,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_class_nested(Owner::Class(id), source.source(), queues, file);
                }
            }
            ast::NestedTypeDecl::Interface(source) => {
                if let Some(id) = self.declare_interface(
                    source,
                    queues.interfaces,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_interface_nested(Owner::Interface(id), source, queues, file);
                }
            }
            ast::NestedTypeDecl::Object(source) => {
                if let Some(id) =
                    self.declare_object(source, queues.objects, queues.methods, file, Some(owner))
                {
                    self.declare_object_nested(
                        Owner::Object(id),
                        super::objects::ObjectSource::Object(source),
                        queues,
                        file,
                    );
                }
            }
        }
    }

    fn declare_recovered_nested_nominal<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::RecoveredNominal,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        match declaration {
            ast::RecoveredNominal::Class(declaration) => {
                let source = declaration.source();
                if let Some(id) = self.declare_class(
                    source,
                    self.source_is_core(file),
                    queues.classes,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_class_nested(Owner::Class(id), source, queues, file);
                }
            }
            ast::RecoveredNominal::Struct(declaration) => {
                let source = declaration.source();
                if let Some(id) =
                    self.declare_struct(source, queues.structs, queues.methods, file, Some(owner))
                {
                    self.declare_struct_nested(Owner::Struct(id), source, queues, file);
                }
            }
        }
    }
}
