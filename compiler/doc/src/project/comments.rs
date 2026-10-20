use super::*;

impl Projection<'_> {
    pub(super) fn warning(&self, warning: &scoop_ast::Diagnostic) -> Result<DocWarning> {
        Ok(DocWarning {
            location: self.location(
                warning.file,
                warning
                    .span
                    .expect("documentation warnings identify an input span"),
            )?,
            message: warning.message.clone(),
            notes: warning
                .notes
                .iter()
                .map(|note| {
                    Ok(DocWarningNote {
                        location: self.location(note.file, note.span)?,
                        message: note.message.clone(),
                    })
                })
                .collect::<Result<_>>()?,
        })
    }

    pub(super) fn comment(&self, comment: &hir::DocumentationComment) -> Result<Comment> {
        Ok(Comment {
            body: self.markdown(&comment.body)?,
            tags: comment
                .tags
                .iter()
                .map(|tag| self.tag(tag))
                .collect::<Result<_>>()?,
        })
    }

    fn tag(&self, tag: &hir::DocumentationTag) -> Result<CommentTag> {
        use hir::DocumentationTagKind as T;
        let kind = match &tag.kind {
            T::Param(_) => TagKind::Param,
            T::Property(_) => TagKind::Property,
            T::Constructor(_) => TagKind::Constructor,
            T::Receiver(_) => TagKind::Receiver,
            T::Return(_) => TagKind::Return,
            T::Throws(_) => TagKind::Throws,
            T::See(_) => TagKind::See,
            T::Since => TagKind::Since,
        };
        let target = match &tag.kind {
            T::Param(target)
            | T::Property(target)
            | T::Constructor(target)
            | T::Receiver(target)
            | T::Return(target) => Some(LinkTarget::Symbols(vec![self.target(*target)?])),
            T::Throws(target) => Some(LinkTarget::Symbols(vec![self.symbol(*target)?])),
            T::See(link) => Some(self.link(link)?),
            T::Since => None,
        };
        Ok(CommentTag {
            kind,
            label: tag.label.clone(),
            target,
            body: self.markdown(&tag.body)?,
        })
    }

    fn markdown(&self, markdown: &hir::DocumentationMarkdown) -> Result<Markdown> {
        Ok(Markdown {
            text: markdown.text.text.clone(),
            definitions: markdown
                .definitions
                .iter()
                .map(|text| text.text.clone())
                .collect(),
            references: markdown
                .references
                .iter()
                .map(|reference| {
                    Ok(SymbolReference {
                        range: reference.range.into(),
                        target: self.link(&reference.resolution)?,
                    })
                })
                .collect::<Result<_>>()?,
        })
    }

    fn link(&self, link: &hir::DocumentationLink) -> Result<LinkTarget> {
        Ok(match link {
            hir::DocumentationLink::Symbols(symbols) => LinkTarget::Symbols(
                symbols
                    .iter()
                    .map(|&symbol| self.symbol(symbol))
                    .collect::<Result<_>>()?,
            ),
            hir::DocumentationLink::Url(url) => LinkTarget::Url(url.clone()),
            hir::DocumentationLink::Unresolved => LinkTarget::Unresolved,
        })
    }
}
