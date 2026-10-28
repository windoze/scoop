use super::*;

impl Concretizer<'_> {
    pub(super) fn function_debug_source(
        &self,
        key: &FunctionKey,
        signature: &export::CallableSignature,
    ) -> Option<scoop_source::FunctionDebugSource> {
        let (file, span) = match self.function_source(key) {
            FunctionSource::Local(id) => {
                let context = self
                    .source
                    .source_contexts
                    .iter()
                    .find(|(_, context)| {
                        context.subject() == &export::SourceContextSubject::Function(id)
                    })?
                    .1;
                let file = self
                    .source
                    .source_files
                    .iter()
                    .find(|file| &file.identity == context.source())?;
                (file, signature.span)
            }
            FunctionSource::Imported(id) => {
                let origin = self.source.imported_generic_templates[id].origin;
                (&self.source.source_files[origin.file as usize], origin.span)
            }
            FunctionSource::Companion(..) | FunctionSource::AnnotationDescription { .. } => {
                return None;
            }
        };
        let (line, column) = if let Some(record) = &file.canonical_record {
            let point = record
                .point(u64::from(span.start))
                .expect("a declaration retains its canonical source point");
            (point.line(), point.column())
        } else {
            if file.source.is_empty() {
                return None;
            }
            let prefix = file.source.get(..span.start as usize)?;
            let line = prefix.bytes().filter(|&byte| byte == b'\n').count() as u64 + 1;
            let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() as u64 + 1;
            (line, column)
        };
        Some(scoop_source::FunctionDebugSource {
            name: signature.name.clone(),
            file: file
                .debug_path
                .as_deref()
                .unwrap_or_else(|| file.identity.logical_path().as_str())
                .to_owned(),
            line: u32::try_from(line).expect("HIR source coordinates fit u32"),
            column: u32::try_from(column).expect("HIR source coordinates fit u32"),
        })
    }
}
