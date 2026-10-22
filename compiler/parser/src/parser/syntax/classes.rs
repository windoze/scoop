//! Original class-body productions, including their exact declaration owner.

use std::collections::HashMap;

use scoop_ast as ast;
use scoop_syntax::{DeclarationData, DeclarationIndex, DeclarationSyntax, SyntaxKind};

use super::{Event, Parser};

#[derive(Debug)]
pub(super) enum ClassEvent {
    Header(Box<ast::ClassHeader>),
    BeginMember,
    MemberKind(SyntaxKind),
    EndMember(Option<Box<ast::ClassMember>>),
    Unavailable,
}

struct Member {
    declaration: Option<DeclarationIndex>,
    value: Option<ast::ClassMember>,
}

struct ActiveMember {
    owner: Option<DeclarationIndex>,
    declaration_start: usize,
    kind: Option<SyntaxKind>,
}

#[derive(Default)]
pub(super) struct ClassRecorder {
    headers: HashMap<DeclarationIndex, ast::ClassHeader>,
    members: HashMap<DeclarationIndex, Vec<Member>>,
    active: Vec<ActiveMember>,
}

impl ClassRecorder {
    pub(super) fn record(
        &mut self,
        event: ClassEvent,
        owner: Option<DeclarationIndex>,
        declarations: &[DeclarationSyntax],
    ) {
        match event {
            ClassEvent::Header(header) => {
                self.headers
                    .insert(owner.expect("class header owner"), *header);
            }
            ClassEvent::BeginMember => self.active.push(ActiveMember {
                owner,
                declaration_start: declarations.len(),
                kind: None,
            }),
            ClassEvent::MemberKind(kind) => {
                let active = self.active.last_mut().expect("class member production");
                active.kind = Some(kind);
                active.declaration_start = declarations.len();
            }
            ClassEvent::EndMember(value) => {
                let active = self.active.pop().expect("class member production");
                let Some(owner) = active.owner else { return };
                if declarations[owner.0].kind != SyntaxKind::ClassDecl {
                    return;
                }
                let declaration = declarations.get(active.declaration_start).and_then(|item| {
                    (Some(item.kind) == active.kind && item.parent == Some(owner))
                        .then_some(DeclarationIndex(active.declaration_start))
                });
                self.members.entry(owner).or_default().push(Member {
                    declaration,
                    value: value.map(|value| *value),
                });
            }
            ClassEvent::Unavailable => {
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
    ) -> Vec<DeclarationData<ast::RecoveredClass>> {
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
                if declaration.parent.is_some() || declaration.kind != SyntaxKind::ClassDecl {
                    return None;
                }
                let declaration = DeclarationIndex(index);
                let value = self.recover_class(declaration, declarations, &headers, &bodies)?;
                Some(DeclarationData { declaration, value })
            })
            .collect()
    }

    fn recover_class(
        &mut self,
        index: DeclarationIndex,
        declarations: &[DeclarationSyntax],
        headers: &HashMap<DeclarationIndex, &ast::FunctionHeader>,
        bodies: &HashMap<DeclarationIndex, &ast::RecoveredFunctionBody>,
    ) -> Option<ast::RecoveredClass> {
        let header = self.headers.remove(&index)?;
        let span = ast::Span::new(header.span.start, declarations[index.0].span.end);
        let mut result = ast::RecoveredClass {
            header,
            span,
            members: Vec::new(),
            methods: Vec::new(),
            classes: Vec::new(),
        };
        for member in self.members.remove(&index).unwrap_or_default() {
            if let Some(value) = member.value {
                result.members.push(value);
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
                    result.methods.push(ast::RecoveredMethod {
                        header,
                        body: bodies.get(&declaration).map(|body| (*body).clone()),
                        span,
                    });
                }
                SyntaxKind::ClassDecl => result.classes.push(self.recover_class(
                    declaration,
                    declarations,
                    headers,
                    bodies,
                )?),
                _ => return None,
            }
        }
        Some(result)
    }
}

impl Parser {
    pub(crate) fn record_class_header(
        &mut self,
        header: &ast::ClassHeader,
        checkpoint: super::super::checkpoint::Checkpoint,
    ) {
        if self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Class(ClassEvent::Header(Box::new(header.clone()))));
        }
    }

    pub(crate) fn begin_class_member(&mut self) {
        self.syntax
            .events
            .push(Event::Class(ClassEvent::BeginMember));
    }

    pub(crate) fn class_member_kind(&mut self, kind: SyntaxKind) {
        self.syntax
            .events
            .push(Event::Class(ClassEvent::MemberKind(kind)));
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
        self.syntax.events.push(Event::Class(ClassEvent::EndMember(
            value.cloned().map(Box::new),
        )));
    }

    pub(crate) fn invalidate_class_projection(&mut self) {
        self.syntax
            .events
            .push(Event::Class(ClassEvent::Unavailable));
    }
}
