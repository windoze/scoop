use super::*;
use crate::{ClassId, EnumId, FunctionId, InterfaceId, ObjectId, StructId};

#[derive(Default)]
pub(super) struct Declarations<'a> {
    annotations: Vec<(&'a ast::AnnotationClassDecl, usize, Option<Owner>)>,
    pub(super) structs: Vec<(StructId, &'a ast::StructDecl, usize)>,
    pub(super) enums: Vec<(EnumId, &'a ast::EnumDecl, usize)>,
    pub(super) classes: Vec<(ClassId, &'a ast::ClassDecl, usize)>,
    pub(super) interfaces: Vec<(InterfaceId, &'a ast::InterfaceDecl, usize)>,
    pub(super) objects: Vec<(ObjectId, crate::declarations::ObjectSource<'a>, usize)>,
    pub(super) methods: Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
}

impl Declarations<'_> {
    pub(super) fn owners(&self) -> Vec<Owner> {
        self.structs
            .iter()
            .map(|&(id, _, _)| Owner::Struct(id))
            .chain(self.enums.iter().map(|&(id, _, _)| Owner::Enum(id)))
            .chain(self.classes.iter().map(|&(id, _, _)| Owner::Class(id)))
            .chain(
                self.interfaces
                    .iter()
                    .map(|&(id, _, _)| Owner::Interface(id)),
            )
            .chain(self.objects.iter().map(|&(id, _, _)| Owner::Object(id)))
            .collect()
    }
}

impl Lowerer {
    pub(crate) fn lower_local_type(
        &mut self,
        declaration: &ast::LocalTypeDecl,
        anonymous: bool,
    ) -> Option<Owner> {
        let source = (self.current_text_file(), declaration.span().start);
        let available = self.capture_environment();
        if let Some(&owner) = self.local_types_by_source.get(&source) {
            self.local_types.get_mut(&owner).unwrap().available = available;
            if !anonymous {
                self.scopes
                    .declare_nominal(declaration.name().text.clone(), nominal_target(owner));
            }
            return Some(owner);
        }
        let errors = self.diagnostics.len();
        let completion = completion::Start::new(self);
        let previous_scopes = self.scopes.clone();
        let parameters = self.type_params_in_scope.clone();
        self.local_type_declaration = Some(DeclarationContext {
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(
                    if anonymous {
                        StructuralDefinitionSiteRole::AnonymousObject
                    } else {
                        StructuralDefinitionSiteRole::LocalDeclaration
                    },
                    declaration.span().start,
                ),
                [],
            ),
            parameters: parameters.clone(),
            anonymous,
        });
        let mut pending = Declarations::default();
        let file = self.current_file;
        let parent = self.current_owner;
        let target = match declaration {
            ast::LocalTypeDecl::Class(decl) => self
                .declare_class(
                    decl,
                    false,
                    &mut pending.classes,
                    &mut pending.methods,
                    file,
                    parent,
                )
                .map(NominalTarget::Class),
            ast::LocalTypeDecl::Struct(decl) => self
                .declare_struct(
                    decl,
                    &mut pending.structs,
                    &mut pending.methods,
                    file,
                    parent,
                )
                .map(NominalTarget::Struct),
            ast::LocalTypeDecl::Enum(decl) => self
                .declare_enum(
                    decl,
                    false,
                    &mut pending.enums,
                    &mut pending.methods,
                    file,
                    parent,
                )
                .map(NominalTarget::Enum),
            ast::LocalTypeDecl::Interface(decl) => self
                .declare_interface(
                    decl,
                    &mut pending.interfaces,
                    &mut pending.methods,
                    file,
                    parent,
                )
                .map(NominalTarget::Interface),
        };
        self.local_type_declaration = None;
        let owner = target?.owner();
        let scopes = self.scopes.nested_callable();
        self.local_types.insert(
            owner,
            LocalType {
                outer_parameter_count: parameters.len(),
                scopes: scopes.clone(),
                anonymous,
                available,
                receivers: self.receiver_scopes.clone(),
                captures: Vec::new(),
                facts: self.smart_casts.clone(),
                functions: self.local_function_scopes.clone(),
            },
        );
        self.local_types_by_source.insert(source, owner);
        let mut queues = crate::declarations::NestedDeclarationQueues {
            annotations: &mut pending.annotations,
            structs: &mut pending.structs,
            enums: &mut pending.enums,
            classes: &mut pending.classes,
            interfaces: &mut pending.interfaces,
            objects: &mut pending.objects,
            methods: &mut pending.methods,
        };
        match declaration {
            ast::LocalTypeDecl::Class(decl) => {
                self.declare_class_nested(owner, decl, &mut queues, file)
            }
            ast::LocalTypeDecl::Struct(decl) => {
                self.declare_struct_nested(owner, decl, &mut queues, file)
            }
            ast::LocalTypeDecl::Enum(decl) => {
                self.declare_enum_nested(owner, decl, &mut queues, file)
            }
            ast::LocalTypeDecl::Interface(decl) => {
                self.declare_interface_nested(owner, decl, &mut queues, file)
            }
        }
        self.with_declaration_inference_scope(file, |state| {
            state.scopes = scopes;
            for &(declaration, file, owner) in &pending.annotations {
                state.declare_annotation_class(declaration, file, owner);
            }
            state.resolve_local_type_declarations(&pending);
            if state.diagnostics.len() == errors {
                state.complete_local_type_captures(owner, completion);
            }
        });
        if self.diagnostics.len() != errors {
            self.scopes = previous_scopes;
            return None;
        }
        Some(owner)
    }
}

fn nominal_target(owner: Owner) -> NominalTarget {
    match owner {
        Owner::Class(id) => NominalTarget::Class(id),
        Owner::Struct(id) => NominalTarget::Struct(id),
        Owner::Enum(id) => NominalTarget::Enum(id),
        Owner::Interface(id) => NominalTarget::Interface(id),
        Owner::Object(id) => NominalTarget::Object(id),
    }
}
