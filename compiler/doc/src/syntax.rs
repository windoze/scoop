use scoop_ast::{
    Diagnostic, DocComment, DocCommentTarget, DocDocument, SourceDocumentation, SourceFile,
};

use crate::{markdown, normalize, tags};

/// Prepare only the parser's actual document comments. This function does not
/// discover declarations, resolve symbols, or inspect other source files.
pub fn prepare_source(file: &SourceFile, source: &str) -> SourceDocumentation {
    prepare_comments(&file.documentation, source)
}

/// Prepare the actual comment attachments retained by a recovering parse.
pub fn prepare_comments(comments: &[DocComment], source: &str) -> SourceDocumentation {
    let mut output = SourceDocumentation::default();
    let mut offset = 0;
    while offset < comments.len() {
        let comment = &comments[offset];
        let DocCommentTarget::Declaration(declaration) = comment.target else {
            output.warnings.push(Diagnostic::warning_at(
                comment.span,
                "documentation comment is not attached to a declaration",
            ));
            offset += 1;
            continue;
        };
        let start = offset;
        offset += 1;
        while offset < comments.len()
            && comments[offset].target == DocCommentTarget::Declaration(declaration)
        {
            offset += 1;
        }
        let text = normalize::normalize(&comments[start..offset], source);
        let definitions = markdown::definitions(&text);
        let (body, tags) = tags::split(&text, &definitions, &mut output.warnings);
        output.documents.push(DocDocument {
            declaration,
            span: text.span,
            body,
            tags,
        });
    }
    output
}
