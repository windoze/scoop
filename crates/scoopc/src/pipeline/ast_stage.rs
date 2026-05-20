use crate::ast;
use crate::cone::SourceConeCompilationUnit;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;

/// AST stage 的单文件 worker 输出。
///
/// 正式 project/frontend handoff 是 `AstCompilationUnitOutput`；这个类型只表示
/// 一个 source file 的解析结果，供 dump/helper 路径和 cone-level worker 复用。
/// P1 在这里固定如下 contract：
/// - AST 只保留普通 `Call` / `MemberAccess` 等源码级形状；
/// - 本阶段不执行任何 type-dependent desugar；
/// - `k.resume()` 与一般 `f()` 仍保留为零参数调用，而不是提前改写成显式 `Unit` 实参；
/// - `k.resume(())` / `f(())` 仍保留为带 `UnitLit` 的一参数调用；
/// - `k.resume()` <=> `k.resume(())` 与 `f()` <=> `f(())` 的等价性，只允许在 P2 typed 阶段解释；
/// - `Continuation` 的 typed 含义、runtime error 传播与 effect row 解释都不属于 P1。
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

/// AST stage 的 cone-level source compilation-unit handoff。
///
/// 该输出绑定一个 source cone compilation unit 及其中全部 source file 的 AST；
/// 文件顺序只来自 project model 的稳定遍历，用于确定诊断/dump 顺序，不表达语义依赖。
#[derive(Debug)]
pub struct AstCompilationUnitOutput<'a> {
    unit: SourceConeCompilationUnit<'a>,
    files: Vec<AstStageOutput<'a>>,
}

impl<'a> AstCompilationUnitOutput<'a> {
    pub(crate) fn new(unit: SourceConeCompilationUnit<'a>, files: Vec<AstStageOutput<'a>>) -> Self {
        Self { unit, files }
    }

    pub fn unit(&self) -> SourceConeCompilationUnit<'a> {
        self.unit
    }

    pub fn files(&self) -> &[AstStageOutput<'a>] {
        &self.files
    }

    pub fn into_asts(self) -> Vec<ast::File> {
        self.files
            .into_iter()
            .map(AstStageOutput::into_ast)
            .collect()
    }
}

pub(crate) fn run<'a>(
    session: &Session,
    source: &'a SourceFile,
) -> Result<AstStageOutput<'a>, ParseError> {
    // P1 的 AST stage 只负责产出稳定的 surface handoff，不在这里猜测 typed 语义。
    let ast = session.parse(source)?;
    Ok(AstStageOutput::new(source, ast))
}

pub(crate) fn run_compilation_unit<'a>(
    session: &Session,
    unit: SourceConeCompilationUnit<'a>,
) -> Result<AstCompilationUnitOutput<'a>, ParseError> {
    let mut files = Vec::with_capacity(unit.sources().len());
    for source in unit.sources() {
        files.push(run(session, source)?);
    }
    Ok(AstCompilationUnitOutput::new(unit, files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionOptions;

    #[test]
    fn ast_stage_output_is_constructible_for_pipeline() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(std::ptr::eq(output.source(), &source));
        assert!(output.ast().package.is_some());
    }

    #[test]
    fn ast_compilation_unit_output_contains_all_cone_sources() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let first = SourceFile::new_virtual(
            "/tmp/scoop-ast-unit/src/a.scoop",
            "package sample\nfun a() {}\n",
        );
        let second = SourceFile::new_virtual(
            "/tmp/scoop-ast-unit/src/b.scoop",
            "package sample\nfun b() {}\n",
        );
        let manifest = crate::cone::ConeManifest {
            cone: crate::cone::ConeSection {
                name: "sample".to_string(),
                version: "0.0.0".to_string(),
                kind: crate::cone::ConeKind::Bin,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: crate::cone::ConeNativeBuildConfig::default(),
        };
        let graph = crate::cone::SourceConeGraph::from_nodes(
            vec![crate::cone::SourceConeNode {
                id: crate::cone::CONSUMER_CONE_ID,
                role: crate::cone::SourceConeRole::Consumer,
                root: std::path::PathBuf::from("/tmp/scoop-ast-unit"),
                manifest_path: std::path::PathBuf::new(),
                kind: manifest.cone.kind,
                native_build: manifest.native_build.clone(),
                manifest,
                trust: crate::cone::SourceConeTrust::Untrusted,
                sources: vec![first, second],
                entry_main: Some(std::path::PathBuf::from("/tmp/scoop-ast-unit/src/a.scoop")),
                dependencies: Vec::new(),
            }],
            crate::cone::CONSUMER_CONE_ID,
        )
        .unwrap();

        let output = run_compilation_unit(&session, graph.consumer_compilation_unit()).unwrap();

        assert_eq!(output.unit().id(), crate::cone::CONSUMER_CONE_ID);
        assert_eq!(output.files().len(), 2);
        assert_eq!(
            output
                .files()
                .iter()
                .map(|file| file.source().path().file_name().unwrap().to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["a.scoop", "b.scoop"]
        );
    }
}
