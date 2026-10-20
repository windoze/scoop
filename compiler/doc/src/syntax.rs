use scoop_ast::{Diagnostic, DocCommentTarget, DocDocument, SourceDocumentation, SourceFile};

use crate::{markdown, normalize, tags};

/// Prepare only the parser's actual document comments. This function does not
/// discover declarations, resolve symbols, or inspect other source files.
pub fn prepare_source(file: &SourceFile, source: &str) -> SourceDocumentation {
    let mut output = SourceDocumentation::default();
    let mut offset = 0;
    while offset < file.documentation.len() {
        let comment = &file.documentation[offset];
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
        while offset < file.documentation.len()
            && file.documentation[offset].target == DocCommentTarget::Declaration(declaration)
        {
            offset += 1;
        }
        let text = normalize::normalize(&file.documentation[start..offset], source);
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
