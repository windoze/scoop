use super::*;
use crate::declarations::ObjectSource;

/// Own the exact declaration input, including independently recovered methods.
struct NominalInput<H, M> {
    header: H,
    members: Vec<M>,
    recovery: ast::RecoveredNominalMembers,
    span: ast::Span,
}

impl<H: Clone, M: Clone> NominalInput<H, M> {
    fn new(
        header: &H,
        members: &[M],
        recovery: &ast::RecoveredNominalMembers,
        span: ast::Span,
    ) -> Self {
        Self {
            header: header.clone(),
            members: members.to_vec(),
            recovery: recovery.clone(),
            span,
        }
    }
}

enum Syntax {
    Class(
        hir::ClassId,
        NominalInput<ast::ClassHeader, ast::ClassMember>,
    ),
    Struct(
        hir::StructId,
        NominalInput<ast::StructHeader, ast::StructMember>,
    ),
    Object(
        hir::ObjectId,
        NominalInput<ast::ObjectHeader, ast::ClassMember>,
    ),
    Companion(
        hir::ObjectId,
        NominalInput<ast::CompanionObjectHeader, ast::ClassMember>,
    ),
}

pub(super) struct ConstructionInput {
    pub(super) file: usize,
    syntax: Syntax,
}

impl ConstructionInput {
    pub(super) fn owner(&self) -> Owner {
        match self.syntax {
            Syntax::Class(id, _) => Owner::Class(id),
            Syntax::Struct(id, _) => Owner::Struct(id),
            Syntax::Object(id, _) | Syntax::Companion(id, _) => Owner::Object(id),
        }
    }

    pub(super) fn origin(&self, lowerer: &Lowerer) -> hir::SemanticSourceLocation {
        let span = match &self.syntax {
            Syntax::Class(_, input) => input.span,
            Syntax::Struct(_, input) => input.span,
            Syntax::Object(_, input) => input.span,
            Syntax::Companion(_, input) => input.span,
        };
        hir::SemanticSourceLocation {
            source: lowerer.source_identity(self.file).clone(),
            span,
        }
    }

    pub(super) fn members(&self) -> &[ast::ClassMember] {
        match &self.syntax {
            Syntax::Class(_, input) => &input.members,
            Syntax::Object(_, input) => &input.members,
            Syntax::Companion(_, input) => &input.members,
            Syntax::Struct(_, _) => &[],
        }
    }

    pub(super) fn secondary(&self) -> Vec<&ast::SecondaryConstructorDecl> {
        match &self.syntax {
            Syntax::Struct(_, input) => input
                .members
                .iter()
                .filter_map(|member| {
                    if let ast::StructMember::SecondaryConstructor(source) = member {
                        Some(source)
                    } else {
                        None
                    }
                })
                .collect(),
            _ => self
                .members()
                .iter()
                .filter_map(|member| {
                    if let ast::ClassMember::SecondaryConstructor(source) = member {
                        Some(source)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    pub(super) fn lower(&self, lowerer: &mut Lowerer) {
        match &self.syntax {
            Syntax::Class(id, input) => {
                let source = ast::ClassSource {
                    header: &input.header,
                    members: &input.members,
                    recovery: &input.recovery,
                    span: input.span,
                };
                lowerer.resolve_constructor_graphs(&[(*id, source, self.file)], &[], &[]);
                lowerer.lower_constructor_initialization(&[(*id, source, self.file)], &[], &[]);
            }
            Syntax::Struct(id, input) => {
                let source = ast::StructSource {
                    header: &input.header,
                    members: &input.members,
                    recovery: &input.recovery,
                    span: input.span,
                };
                lowerer.resolve_constructor_graphs(&[], &[(*id, source, self.file)], &[]);
                lowerer.lower_constructor_initialization(&[], &[(*id, source, self.file)], &[]);
            }
            Syntax::Object(id, input) => {
                let source = ObjectSource::Object(ast::ObjectSource {
                    header: &input.header,
                    members: &input.members,
                    recovery: &input.recovery,
                    span: input.span,
                });
                lowerer.resolve_constructor_graphs(&[], &[], &[(*id, source, self.file)]);
                lowerer.lower_constructor_initialization(&[], &[], &[(*id, source, self.file)]);
            }
            Syntax::Companion(id, input) => {
                let source = ObjectSource::Companion(ast::CompanionObjectSource {
                    header: &input.header,
                    members: &input.members,
                    recovery: &input.recovery,
                    span: input.span,
                });
                lowerer.resolve_constructor_graphs(&[], &[], &[(*id, source, self.file)]);
                lowerer.lower_constructor_initialization(&[], &[], &[(*id, source, self.file)]);
            }
        }
    }
}

impl Lowerer {
    pub(crate) fn remember_class_construction(
        &mut self,
        id: hir::ClassId,
        source: ast::ClassSource<'_>,
    ) {
        if let Some(inputs) = &mut self.editing_constructions {
            inputs.sources.insert(
                Owner::Class(id),
                Rc::new(ConstructionInput {
                    file: self.current_file,
                    syntax: Syntax::Class(
                        id,
                        NominalInput::new(
                            source.header,
                            source.members,
                            source.recovery,
                            source.span,
                        ),
                    ),
                }),
            );
        }
    }

    pub(crate) fn remember_struct_construction(
        &mut self,
        id: hir::StructId,
        source: ast::StructSource<'_>,
    ) {
        if let Some(inputs) = &mut self.editing_constructions {
            inputs.sources.insert(
                Owner::Struct(id),
                Rc::new(ConstructionInput {
                    file: self.current_file,
                    syntax: Syntax::Struct(
                        id,
                        NominalInput::new(
                            source.header,
                            source.members,
                            source.recovery,
                            source.span,
                        ),
                    ),
                }),
            );
        }
    }

    pub(crate) fn remember_object_construction(
        &mut self,
        id: hir::ObjectId,
        source: ObjectSource<'_>,
    ) {
        if let Some(inputs) = &mut self.editing_constructions {
            let syntax = match source {
                ObjectSource::Object(source) => Syntax::Object(
                    id,
                    NominalInput::new(source.header, source.members, source.recovery, source.span),
                ),
                ObjectSource::Companion(source) => Syntax::Companion(
                    id,
                    NominalInput::new(source.header, source.members, source.recovery, source.span),
                ),
            };
            inputs.sources.insert(
                Owner::Object(id),
                Rc::new(ConstructionInput {
                    file: self.current_file,
                    syntax,
                }),
            );
        }
    }

    pub(crate) fn remember_construction_property(
        &mut self,
        property: hir::PropertyId,
        source: &ast::PropertyDecl,
    ) {
        let Some(inputs) = &mut self.editing_constructions else {
            return;
        };
        let expression = match &source.body {
            ast::PropertyBodySyntax::Initializer { expression, .. }
            | ast::PropertyBodySyntax::Delegated { expression, .. } => expression,
            _ => return,
        };
        inputs
            .properties
            .insert(property, (self.current_file, expression.span()));
    }

    pub(crate) fn remember_construction_base(
        &mut self,
        class: hir::ClassId,
        source: &ast::SupertypeSpec,
    ) {
        if let Some(inputs) = &mut self.editing_constructions {
            inputs.bases.insert(class, source.clone());
        }
    }
}
