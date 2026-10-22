//! Original nominal-body productions and their exact declaration owner.

use std::collections::HashMap;

use scoop_ast as ast;
use scoop_syntax::{DeclarationData, DeclarationIndex, DeclarationSyntax, SyntaxKind};

use super::{Event, Parser};

#[derive(Debug)]
pub(super) enum NominalHeader {
    Class(Box<ast::ClassHeader>),
    Struct(Box<ast::StructHeader>),
    Interface(Box<ast::InterfaceHeader>),
}

impl NominalHeader {
    fn recovered(self, end: u32) -> ast::RecoveredNominal {
        match self {
            Self::Class(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Class(Box::new(ast::RecoveredClass {
                    header: *header,
                    span,
                    members: Vec::new(),
                    recovery: Default::default(),
                }))
            }
            Self::Struct(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Struct(Box::new(ast::RecoveredStruct {
                    header: *header,
                    span,
                    members: Vec::new(),
                    recovery: Default::default(),
                }))
            }
            Self::Interface(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Interface(Box::new(ast::RecoveredInterface {
                    header: *header,
                    span,
                    methods: Vec::new(),
                    properties: Vec::new(),
                    nested: Vec::new(),
                    companion: None,
                    recovery: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum NominalMember {
    Class(Box<ast::ClassMember>),
    Struct(Box<ast::StructMember>),
    Function(Box<ast::FunctionDecl>),
    Property(Box<ast::PropertyDecl>),
    Nested(Box<ast::NestedTypeDecl>),
    Companion(Box<ast::CompanionObjectDecl>),
}

impl NominalMember {
    pub(crate) fn span(&self) -> ast::Span {
        match self {
            Self::Class(value) => value.span(),
            Self::Struct(value) => value.span(),
            Self::Function(value) => value.span,
            Self::Property(value) => value.span,
            Self::Nested(value) => value.span(),
            Self::Companion(value) => value.span,
        }
    }
}

#[derive(Debug)]
pub(super) enum NominalEvent {
    Header(NominalHeader),
    BeginMember,
    MemberKind(SyntaxKind),
    EndMember(Option<NominalMember>),
    Unavailable,
}

struct Member {
    declaration: Option<DeclarationIndex>,
    value: Option<NominalMember>,
}

struct ActiveMember {
    owner: Option<DeclarationIndex>,
    declaration_start: usize,
    kind: Option<SyntaxKind>,
}

#[derive(Default)]
pub(super) struct NominalRecorder {
    headers: HashMap<DeclarationIndex, NominalHeader>,
    members: HashMap<DeclarationIndex, Vec<Member>>,
    active: Vec<ActiveMember>,
}

impl NominalRecorder {
    pub(super) fn record(
        &mut self,
        event: NominalEvent,
        owner: Option<DeclarationIndex>,
        declarations: &[DeclarationSyntax],
    ) {
        match event {
            NominalEvent::Header(header) => {
                self.headers
                    .insert(owner.expect("nominal header owner"), header);
            }
            NominalEvent::BeginMember => self.active.push(ActiveMember {
                owner,
                declaration_start: declarations.len(),
                kind: None,
            }),
            NominalEvent::MemberKind(kind) => {
                let active = self.active.last_mut().expect("nominal member production");
                active.kind = Some(kind);
                active.declaration_start = declarations.len();
            }
            NominalEvent::EndMember(value) => {
                let active = self.active.pop().expect("nominal member production");
                let Some(owner) = active.owner else { return };
                if !matches!(
                    declarations[owner.0].kind,
                    SyntaxKind::ClassDecl | SyntaxKind::StructDecl | SyntaxKind::InterfaceDecl
                ) {
                    return;
                }
                let declaration = declarations.get(active.declaration_start).and_then(|item| {
                    (Some(item.kind) == active.kind && item.parent == Some(owner))
                        .then_some(DeclarationIndex(active.declaration_start))
                });
                self.members
                    .entry(owner)
                    .or_default()
                    .push(Member { declaration, value });
            }
            NominalEvent::Unavailable => {
                if let Some(owner) = owner {
                    self.headers.remove(&owner);
                }
            }
        }
    }

    pub(super) fn recover(
        mut self,
        declarations: &[DeclarationSyntax],
        headers: &[DeclarationData<ast::FunctionHeader>],
        bodies: &[DeclarationData<ast::RecoveredFunctionBody>],
    ) -> Vec<DeclarationData<ast::RecoveredNominal>> {
        let headers = headers
            .iter()
            .map(|item| (item.declaration, &item.value))
            .collect();
        let bodies = bodies
            .iter()
            .map(|item| (item.declaration, &item.value))
            .collect();
        declarations
            .iter()
            .enumerate()
            .filter_map(|(index, declaration)| {
                if declaration.parent.is_some()
                    || !matches!(
                        declaration.kind,
                        SyntaxKind::ClassDecl | SyntaxKind::StructDecl | SyntaxKind::InterfaceDecl
                    )
                {
                    return None;
                }
                let declaration = DeclarationIndex(index);
                let value = self.recover_nominal(declaration, declarations, &headers, &bodies)?;
                Some(DeclarationData { declaration, value })
            })
            .collect()
    }

    fn recover_nominal(
        &mut self,
        index: DeclarationIndex,
        declarations: &[DeclarationSyntax],
        headers: &HashMap<DeclarationIndex, &ast::FunctionHeader>,
        bodies: &HashMap<DeclarationIndex, &ast::RecoveredFunctionBody>,
    ) -> Option<ast::RecoveredNominal> {
        let header = self.headers.remove(&index)?;
        let mut result = header.recovered(declarations[index.0].span.end);
        for member in self.members.remove(&index).unwrap_or_default() {
            if let Some(value) = member.value {
                match (&mut result, value) {
                    (ast::RecoveredNominal::Class(class), NominalMember::Class(value)) => {
                        class.members.push(*value)
                    }
                    (ast::RecoveredNominal::Struct(structure), NominalMember::Struct(value)) => {
                        structure.members.push(*value)
                    }
                    (
                        ast::RecoveredNominal::Interface(interface),
                        NominalMember::Function(value),
                    ) => interface.methods.push(*value),
                    (
                        ast::RecoveredNominal::Interface(interface),
                        NominalMember::Property(value),
                    ) => interface.properties.push(*value),
                    (ast::RecoveredNominal::Interface(interface), NominalMember::Nested(value)) => {
                        interface.nested.push(*value)
                    }
                    (
                        ast::RecoveredNominal::Interface(interface),
                        NominalMember::Companion(value),
                    ) => interface.companion = Some(*value),
                    _ => unreachable!("each nominal body records its own member syntax"),
                }
                continue;
            }
            let declaration = member.declaration?;
            match declarations[declaration.0].kind {
                SyntaxKind::FunctionDecl => {
                    let header = (*headers.get(&declaration)?).clone();
                    let span = ast::Span::new(
                        header.signature.span.start,
                        declarations[declaration.0].span.end,
                    );
                    result.recovery_mut().methods.push(ast::RecoveredMethod {
                        header,
                        body: bodies.get(&declaration).map(|body| (*body).clone()),
                        span,
                    });
                }
                SyntaxKind::ClassDecl | SyntaxKind::StructDecl | SyntaxKind::InterfaceDecl => {
                    result.recovery_mut().nested.push(self.recover_nominal(
                        declaration,
                        declarations,
                        headers,
                        bodies,
                    )?)
                }
                _ => return None,
            }
        }
        Some(result)
    }
}

impl Parser {
    pub(crate) fn record_interface_header(
        &mut self,
        header: &ast::InterfaceHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax.events.push(Event::Nominal(NominalEvent::Header(
                NominalHeader::Interface(Box::new(header.clone())),
            )));
        }
    }

    pub(crate) fn record_struct_header(
        &mut self,
        header: &ast::StructHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Nominal(NominalEvent::Header(NominalHeader::Struct(
                    Box::new(header.clone()),
                ))));
        }
    }

    pub(crate) fn record_class_header(
        &mut self,
        header: &ast::ClassHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Nominal(NominalEvent::Header(NominalHeader::Class(
                    Box::new(header.clone()),
                ))));
        }
    }

    pub(crate) fn begin_nominal_member(&mut self) {
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::BeginMember));
    }

    pub(crate) fn nominal_member_kind(&mut self) {
        let kind = match self.peek().kind {
            crate::lexer::TokenKind::Fun => SyntaxKind::FunctionDecl,
            crate::lexer::TokenKind::Class => SyntaxKind::ClassDecl,
            crate::lexer::TokenKind::Struct => SyntaxKind::StructDecl,
            crate::lexer::TokenKind::Interface => SyntaxKind::InterfaceDecl,
            _ => return,
        };
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::MemberKind(kind)));
    }

    pub(crate) fn end_class_member(
        &mut self,
        value: &Result<ast::ClassMember, ast::Diagnostic>,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        let value = value
            .as_ref()
            .ok()
            .filter(|_| self.fragment_is_complete(checkpoint));
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::EndMember(
                value.cloned().map(Box::new).map(NominalMember::Class),
            )));
    }

    pub(crate) fn end_struct_member(
        &mut self,
        value: &Result<ast::StructMember, ast::Diagnostic>,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        let value = value
            .as_ref()
            .ok()
            .filter(|_| self.fragment_is_complete(checkpoint));
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::EndMember(
                value.cloned().map(Box::new).map(NominalMember::Struct),
            )));
    }

    pub(crate) fn invalidate_nominal_projection(&mut self) {
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::Unavailable));
    }

    pub(crate) fn end_type_member(
        &mut self,
        value: &Result<NominalMember, ast::Diagnostic>,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        let value = value
            .as_ref()
            .ok()
            .filter(|_| self.fragment_is_complete(checkpoint))
            .cloned();
        self.syntax
            .events
            .push(Event::Nominal(NominalEvent::EndMember(value)));
    }
}
