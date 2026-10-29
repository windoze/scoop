//! Consume bound directory inputs without leaving runtime argument evaluation.

use super::*;

impl Lowerer {
    pub(super) fn normalize_include_directory(
        &mut self,
        arguments: &[hir::Expr],
        sources: &[Option<ast::Expr>],
        result_type: hir::TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let [path, include, exclude] = arguments else {
            unreachable!("include_dir has three checked parameters")
        };
        let path_source = sources[0].as_ref();
        let path_span = path_source.map_or(path.span, ast::Expr::span);
        let Some(path_value) = self.input_string(path, path_source) else {
            self.error(
                path_span,
                "`includeDir` requires a compile-time constant String path".into(),
            );
            return None;
        };
        let path = match hir::CompileTimePath::parse(&path_value) {
            Ok(path) => path,
            Err(message) => {
                self.error(path_span, message.into());
                return None;
            }
        };
        let (include, include_spans) =
            self.directory_patterns("include", include, sources[1].as_ref())?;
        let (exclude, exclude_spans) =
            self.directory_patterns("exclude", exclude, sources[2].as_ref())?;
        let Some(inputs) = &self.compile_time_inputs else {
            self.error(
                span,
                "no compile-time input host was supplied for `includeDir`".into(),
            );
            return None;
        };
        let files = match inputs.directory(
            self.source_identity(self.current_file),
            &path,
            &hir::CompileTimeDirectoryFilter { include, exclude },
        ) {
            Ok(files) => files,
            Err(error) => {
                let (location, message) = match error {
                    hir::CompileTimeDirectoryError::Input(message) => (path_span, message),
                    hir::CompileTimeDirectoryError::IncludePattern { index, message } => (
                        include_spans[index],
                        format!("invalid include pattern: {message}"),
                    ),
                    hir::CompileTimeDirectoryError::ExcludePattern { index, message } => (
                        exclude_spans[index],
                        format!("invalid exclude pattern: {message}"),
                    ),
                };
                self.error(location, message);
                return None;
            }
        };
        let (application, files_type) = self
            .embedded_directory_types(result_type)
            .expect("include_dir returns the checked ordinary directory struct");
        Some(hir::Expr {
            kind: hir::ExprKind::StructConstruct {
                application,
                fields: vec![hir::Expr {
                    kind: hir::ExprKind::EmbeddedFileArrayLiteral(files.to_vec()),
                    ty: files_type,
                    span,
                    origin: self.expression_origin(span),
                }],
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn input_string(&mut self, value: &hir::Expr, source: Option<&ast::Expr>) -> Option<String> {
        if let Some(source) = source {
            return self.compiler_input_string(source);
        }
        match self.expression_constant(value)? {
            hir::HirConstantImage::String(value) => Some(value),
            _ => None,
        }
    }

    fn directory_patterns(
        &mut self,
        name: &str,
        value: &hir::Expr,
        source: Option<&ast::Expr>,
    ) -> Option<(Vec<String>, Vec<Span>)> {
        let elements = match (&value.kind, source) {
            (
                hir::ExprKind::ArrayLiteral(elements),
                None | Some(ast::Expr::ArrayLiteral { .. }),
            ) => elements,
            _ => {
                self.error(source.map_or(value.span, ast::Expr::span), format!(
                    "`includeDir` {name} requires an array literal of compile-time constant Strings"
                ));
                return None;
            }
        };
        let source_elements = match source {
            Some(ast::Expr::ArrayLiteral { elements, .. }) => Some(elements),
            _ => None,
        };
        let mut patterns = Vec::with_capacity(elements.len());
        let mut spans = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let source = source_elements.map(|elements| &elements[index]);
            let span = source.map_or(element.span, ast::Expr::span);
            let Some(pattern) = self.input_string(element, source) else {
                self.error(
                    span,
                    format!("`includeDir` {name} elements must be compile-time constant Strings"),
                );
                return None;
            };
            patterns.push(pattern);
            spans.push(span);
        }
        Some((patterns, spans))
    }
}
