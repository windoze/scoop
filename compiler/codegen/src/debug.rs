//! Standard DWARF observations. This module never reconstructs source facts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DICompileUnit, DIFile, DIFlags, DIFlagsConstants, DILocation, DIScope, DISubprogram,
    DIType, DWARFEmissionKind, DWARFSourceLanguage, DebugInfoBuilder,
};
use inkwell::module::{FlagBehavior, Module};
use inkwell::values::FunctionValue;
use scoop_source::{DebugFile, DebugInlineSite, DebugLocation, DebugOrigin, DebugScope};

use crate::ValidatedBackendProfile;

mod types;
mod variables;

use variables::Variable;

pub(crate) struct DebugContext<'a, 'ctx> {
    context: &'ctx Context,
    builder: DebugInfoBuilder<'ctx>,
    unit: DICompileUnit<'ctx>,
    optimized: bool,
    files: RefCell<HashMap<Arc<DebugFile>, DIFile<'ctx>>>,
    source_types: &'a scoop_lir::DebugTypes,
    types: RefCell<HashMap<scoop_lir::PersistentExactTypeId, DIType<'ctx>>>,
    full: bool,
}

impl<'a, 'ctx> DebugContext<'a, 'ctx> {
    pub(crate) fn new(
        context: &'ctx Context,
        module: &Module<'ctx>,
        profile: ValidatedBackendProfile,
        source: &DebugFile,
        source_types: &'a scoop_lir::DebugTypes,
    ) -> Self {
        let (filename, directory) = path_parts(source);
        let optimized = profile.optimization() == scoop_lir::OptimizationMode::Release;
        let (builder, unit) = module.create_debug_info_builder(
            true,
            DWARFSourceLanguage::C,
            filename,
            directory,
            concat!("scoopc ", env!("CARGO_PKG_VERSION")),
            optimized,
            "",
            0,
            "",
            if profile.debug_info() == scoop_lir::DebugInfoLevel::Full {
                DWARFEmissionKind::Full
            } else {
                DWARFEmissionKind::LineTablesOnly
            },
            0,
            true,
            false,
            "",
            "",
        );
        module.add_basic_value_flag(
            "Dwarf Version",
            FlagBehavior::Warning,
            context.i32_type().const_int(5, false),
        );
        module.add_basic_value_flag(
            "Debug Info Version",
            FlagBehavior::Warning,
            context
                .i32_type()
                .const_int(inkwell::debug_info::debug_metadata_version().into(), false),
        );
        Self {
            context,
            builder,
            unit,
            optimized,
            files: RefCell::default(),
            source_types,
            types: RefCell::default(),
            full: profile.debug_info() == scoop_lir::DebugInfoLevel::Full,
        }
    }

    fn file(&self, source: &Arc<DebugFile>) -> DIFile<'ctx> {
        *self
            .files
            .borrow_mut()
            .entry(source.clone())
            .or_insert_with(|| {
                let (filename, directory) = path_parts(source);
                self.builder.create_file(filename, directory)
            })
    }

    fn subprogram(
        &self,
        name: &str,
        linkage: &str,
        source: &Arc<DebugFile>,
        line: u32,
        signature: &scoop_lir::DebugSignature,
    ) -> DISubprogram<'ctx> {
        let file = self.file(source);
        let (parameters, result) = if self.full {
            match signature {
                scoop_lir::DebugSignature::Generated => (Vec::new(), None),
                scoop_lir::DebugSignature::Source { parameters, result } => (
                    parameters.iter().map(|ty| self.source_type(*ty)).collect(),
                    result.map(|ty| self.source_type(ty)),
                ),
            }
        } else {
            (Vec::new(), None)
        };
        let signature =
            self.builder
                .create_subroutine_type(file, result, &parameters, DIFlags::PUBLIC);
        self.builder.create_function(
            self.unit.as_debug_info_scope(),
            name,
            Some(linkage),
            file,
            line,
            signature,
            false,
            true,
            line,
            DIFlags::PUBLIC,
            self.optimized,
        )
    }

    pub(crate) fn function<'b>(
        &'b self,
        function: &scoop_lir::Function,
        llvm: FunctionValue<'ctx>,
    ) -> Option<FunctionDebug<'b, 'ctx>> {
        let source = source_file(function)?;
        let line = match &function.debug_origin {
            DebugOrigin::Source(origin) => origin.line,
            DebugOrigin::Generated => 0,
        };
        let name = if function.debug_name.is_empty() {
            function.symbol()
        } else {
            &function.debug_name
        };
        let subprogram = self.subprogram(
            name,
            function.symbol(),
            source,
            line,
            &function.debug_signature,
        );
        llvm.set_subprogram(subprogram);
        Some(FunctionDebug {
            context: self,
            scope: subprogram.as_debug_info_scope(),
            scopes: HashMap::new(),
            lexical_scopes: HashMap::new(),
            inline_functions: HashMap::new(),
            locations: HashMap::new(),
            variables: HashMap::new(),
        })
    }

    pub(crate) fn finish(&self) {
        self.builder.finalize();
    }
}

pub(crate) struct FunctionDebug<'a, 'ctx> {
    context: &'a DebugContext<'a, 'ctx>,
    scope: DIScope<'ctx>,
    scopes: HashMap<(usize, Arc<DebugFile>), DIScope<'ctx>>,
    lexical_scopes: HashMap<(usize, Arc<DebugScope>), DIScope<'ctx>>,
    inline_functions: HashMap<Arc<DebugInlineSite>, DIScope<'ctx>>,
    locations: HashMap<Arc<DebugLocation>, DILocation<'ctx>>,
    variables: HashMap<scoop_lir::DebugStorage, Vec<Variable<'ctx>>>,
}

impl<'ctx> FunctionDebug<'_, 'ctx> {
    pub(crate) fn set_location(&mut self, builder: &Builder<'ctx>, origin: &DebugOrigin) {
        let location = self.location(origin);
        builder.set_current_debug_location(location);
    }

    fn location(&mut self, origin: &DebugOrigin) -> DILocation<'ctx> {
        let DebugOrigin::Source(location) = origin else {
            // Calls in a debug function require a location even for generated
            // protocols. Line zero prevents inheritance of a previous source line.
            return self.context.builder.create_debug_location(
                self.context.context,
                0,
                0,
                self.scope,
                None,
            );
        };
        if let Some(emitted) = self.locations.get(location) {
            return *emitted;
        }
        let (scope, inlined_at) = match &location.inlined_at {
            Some(site) => {
                let call_site = self.location(&site.call_site);
                let scope = *self
                    .inline_functions
                    .entry(site.clone())
                    .or_insert_with(|| {
                        let (file, line) = match &site.definition {
                            DebugOrigin::Source(definition) => (&definition.file, definition.line),
                            DebugOrigin::Generated => (&location.file, 0),
                        };
                        self.context
                            .subprogram(
                                &site.function,
                                "",
                                file,
                                line,
                                &scoop_lir::DebugSignature::Generated,
                            )
                            .as_debug_info_scope()
                    });
                (scope, Some(call_site))
            }
            None => (self.scope, None),
        };
        let scope = self.lexical_scope(scope, location.scope.as_ref());
        let scope = *self
            .scopes
            .entry((scope.as_mut_ptr() as usize, location.file.clone()))
            .or_insert_with(|| {
                self.context
                    .builder
                    .create_lexical_block(scope, self.context.file(&location.file), 0, 0)
                    .as_debug_info_scope()
            });
        let emitted = self.context.builder.create_debug_location(
            self.context.context,
            location.line,
            location.column,
            scope,
            inlined_at,
        );
        self.locations.insert(location.clone(), emitted);
        emitted
    }

    fn lexical_scope(
        &mut self,
        root: DIScope<'ctx>,
        source: Option<&Arc<DebugScope>>,
    ) -> DIScope<'ctx> {
        let Some(source) = source else {
            return root;
        };
        let key = (root.as_mut_ptr() as usize, source.clone());
        if let Some(scope) = self.lexical_scopes.get(&key) {
            return *scope;
        }
        let parent = self.lexical_scope(root, source.parent.as_ref());
        let scope = self
            .context
            .builder
            .create_lexical_block(
                parent,
                self.context.file(&source.file),
                source.line,
                source.column,
            )
            .as_debug_info_scope();
        self.lexical_scopes.insert(key, scope);
        scope
    }
}

pub(crate) fn source_file(function: &scoop_lir::Function) -> Option<&Arc<DebugFile>> {
    if let DebugOrigin::Source(origin) = &function.debug_origin {
        return Some(&origin.file);
    }
    for block in function.blocks.values() {
        for index in 0..block.instructions.len() {
            if let DebugOrigin::Source(origin) = block.instructions.origin(index) {
                return Some(&origin.file);
            }
        }
        if let DebugOrigin::Source(origin) = &block.terminator_origin {
            return Some(&origin.file);
        }
    }
    None
}

fn path_parts(source: &DebugFile) -> (&str, &str) {
    let path = Path::new(source.path.as_ref());
    (
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(source.path.as_ref()),
        path.parent()
            .and_then(|parent| parent.to_str())
            .unwrap_or(""),
    )
}
