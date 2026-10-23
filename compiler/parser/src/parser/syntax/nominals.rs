//! Original nominal-body productions and their exact declaration owner.

use std::collections::HashMap;

use scoop_ast as ast;
use scoop_syntax::{DeclarationData, DeclarationIndex, DeclarationSyntax, SyntaxKind};

use super::{Event, Parser};

mod body;
use body::RecoveredBody;
mod events;
pub(super) use events::NominalEvent;
use events::NominalHeader;
pub(crate) use events::NominalMember;

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
                    SyntaxKind::ClassDecl
                        | SyntaxKind::StructDecl
                        | SyntaxKind::InterfaceDecl
                        | SyntaxKind::EnumDecl
                        | SyntaxKind::ObjectDecl
                        | SyntaxKind::CompanionDecl
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
        declarations: &mut [DeclarationSyntax],
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
        let roots = declarations
            .iter()
            .enumerate()
            .filter_map(|(index, declaration)| {
                if declaration.parent.is_some()
                    || !matches!(
                        declaration.kind,
                        SyntaxKind::ClassDecl
                            | SyntaxKind::StructDecl
                            | SyntaxKind::InterfaceDecl
                            | SyntaxKind::EnumDecl
                            | SyntaxKind::ObjectDecl
                    )
                {
                    return None;
                }
                Some(DeclarationIndex(index))
            })
            .collect::<Vec<_>>();
        roots
            .into_iter()
            .filter_map(|declaration| {
                let RecoveredBody::Nominal(value) =
                    self.recover_body(declaration, declarations, &headers, &bodies)?
                else {
                    return None;
                };
                Some(DeclarationData { declaration, value })
            })
            .collect()
    }

    fn recover_body(
        &mut self,
        index: DeclarationIndex,
        declarations: &mut [DeclarationSyntax],
        headers: &HashMap<DeclarationIndex, &ast::FunctionHeader>,
        bodies: &HashMap<DeclarationIndex, &ast::RecoveredFunctionBody>,
    ) -> Option<RecoveredBody> {
        let header = self.headers.remove(&index)?;
        let mut result = header.recovered(declarations[index.0].span.end);
        for member in self.members.remove(&index).unwrap_or_default() {
            if let Some(value) = member.value {
                result.push_member(value)?;
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
                SyntaxKind::ClassDecl
                | SyntaxKind::StructDecl
                | SyntaxKind::InterfaceDecl
                | SyntaxKind::EnumDecl
                | SyntaxKind::ObjectDecl
                | SyntaxKind::CompanionDecl => {
                    let member = self.recover_body(declaration, declarations, headers, bodies)?;
                    result.push_recovered(member)?;
                }
                _ => return None,
            }
        }
        declarations[index.0]
            .source_span
            .get_or_insert(result.span());
        Some(result)
    }
}

impl Parser {
    pub(crate) fn record_object_header(
        &mut self,
        header: &ast::ObjectHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Nominal(NominalEvent::Header(NominalHeader::Object(
                    Box::new(header.clone()),
                ))));
        }
    }

    pub(crate) fn record_companion_header(
        &mut self,
        header: &ast::CompanionObjectHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax.events.push(Event::Nominal(NominalEvent::Header(
                NominalHeader::Companion(Box::new(header.clone())),
            )));
        }
    }

    pub(crate) fn record_enum_header(
        &mut self,
        header: &ast::EnumHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Nominal(NominalEvent::Header(NominalHeader::Enum(
                    Box::new(header.clone()),
                ))));
        }
    }

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

    pub(crate) fn nominal_member_kind(&mut self, interface_implementation: bool) {
        let kind = match self.peek().kind {
            crate::lexer::TokenKind::Fun => SyntaxKind::FunctionDecl,
            crate::lexer::TokenKind::Class => SyntaxKind::ClassDecl,
            crate::lexer::TokenKind::Struct => SyntaxKind::StructDecl,
            crate::lexer::TokenKind::Interface if interface_implementation => {
                SyntaxKind::InterfaceImplementation
            }
            crate::lexer::TokenKind::Interface => SyntaxKind::InterfaceDecl,
            crate::lexer::TokenKind::Enum => SyntaxKind::EnumDecl,
            crate::lexer::TokenKind::Ident(ref text) if text == "object" => SyntaxKind::ObjectDecl,
            crate::lexer::TokenKind::Ident(ref text) if text == "companion" => {
                SyntaxKind::CompanionDecl
            }
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
