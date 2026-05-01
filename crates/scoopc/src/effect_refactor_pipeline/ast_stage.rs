use crate::ast;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;

/// refactor AST stage 的稳定 handoff 形状。
///
/// P1 在这里固定如下 invariants，供后续 typed 阶段继续消费：
/// - AST 只保留普通 `Call` / `MemberAccess` 等源码级形状；
/// - 本阶段不执行任何 type-dependent desugar；
/// - `k.resume()` 与一般 `f()` 仍保留为零参数调用，而不是提前改写成显式 `Unit` 实参；
/// - `k.resume(())` / `f(())` 仍保留为带 `UnitLit` 的一参数调用；
/// - continuation 的 typed 语义、runtime error 传播与 effect row 解释留给 P2 之后的阶段。
#[derive(Debug)]
pub struct AstStageOutput<'a> {
    source: &'a SourceFile,
    ast: ast::File,
}

impl<'a> AstStageOutput<'a> {
    pub(crate) fn new(source: &'a SourceFile, ast: ast::File) -> Self {
        Self { source, ast }
    }

    pub fn source(&self) -> &'a SourceFile {
        self.source
    }

    pub fn ast(&self) -> &ast::File {
        &self.ast
    }

    pub fn into_ast(self) -> ast::File {
        self.ast
    }
}

pub(crate) fn run<'a>(
    session: &Session,
    source: &'a SourceFile,
) -> Result<AstStageOutput<'a>, ParseError> {
    let ast = session.parse(source)?;
    Ok(AstStageOutput::new(source, ast))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{EffectPipelineMode, SessionOptions};

    #[test]
    fn ast_stage_output_is_constructible_for_refactor_pipeline() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(std::ptr::eq(output.source(), &source));
        assert!(output.ast().package.is_some());
    }
}
