use super::*;

pub(crate) struct NestedDeclarationQueues<'queues, 'source> {
    pub(crate) structs: &'queues mut Vec<(StructId, &'source ast::StructDecl, usize)>,
    pub(crate) enums: &'queues mut Vec<(EnumId, &'source ast::EnumDecl, usize)>,
    pub(crate) classes: &'queues mut Vec<(ClassId, &'source ast::ClassDecl, usize)>,
    pub(crate) interfaces: &'queues mut Vec<(InterfaceId, &'source ast::InterfaceDecl, usize)>,
    pub(crate) objects: &'queues mut Vec<(ObjectId, super::objects::ObjectSource<'source>, usize)>,
    pub(crate) methods: &'queues mut Vec<(FunctionId, &'source ast::FunctionDecl, usize, Owner)>,
}

impl Lowerer {
    pub(crate) fn declare_class_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::ClassDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for member in &declaration.members {
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
                | ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::Function(_) => {}
            }
        }
    }

    pub(crate) fn declare_struct_nested<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::StructDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        for member in &declaration.members {
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
                | ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::Function(_) => {}
            }
        }
    }

    fn declare_nested_nominal<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::NestedNominalDecl,
        queues: &mut NestedDeclarationQueues<'_, 'a>,
        file: usize,
    ) {
        let is_core = self.source_is_core(file);
        match declaration {
            ast::NestedNominalDecl::Struct(source) => {
                if let Some(id) =
                    self.declare_struct(source, queues.structs, queues.methods, file, Some(owner))
                {
                    self.declare_struct_nested(Owner::Struct(id), source, queues, file);
                }
            }
            ast::NestedNominalDecl::Enum(source) => {
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
            ast::NestedNominalDecl::Class(source) => {
                if let Some(id) = self.declare_class(
                    source,
                    is_core,
                    queues.classes,
                    queues.methods,
                    file,
                    Some(owner),
                ) {
                    self.declare_class_nested(Owner::Class(id), source, queues, file);
                }
            }
            ast::NestedNominalDecl::Interface(source) => {
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
            ast::NestedNominalDecl::Object(source) => {
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
}
