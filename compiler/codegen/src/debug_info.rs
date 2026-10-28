//! CodeView declaration records for Scoop machine functions.

use inkwell::debug_info::{
    AsDIScope, DIFlags, DIFlagsConstants, DILocation, DWARFEmissionKind, DWARFSourceLanguage,
    DebugInfoBuilder,
};
use inkwell::module::FlagBehavior;

use super::*;

pub(super) fn create_builder<'a, 'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    functions: impl IntoIterator<Item = &'a Function>,
) -> Option<DebugInfoBuilder<'ctx>> {
    if profile.optimization() != scoop_lir::OptimizationMode::Debug
        || profile.lir_target_profile().native_object_format()
            != scoop_lir::NativeObjectFormat::Coff64
    {
        return None;
    }
    let source = functions
        .into_iter()
        .find_map(|function| function.debug_source.as_ref())?;
    llvm.add_basic_value_flag(
        "Debug Info Version",
        FlagBehavior::Warning,
        context.i32_type().const_int(
            u64::from(inkwell::debug_info::debug_metadata_version()),
            false,
        ),
    );
    llvm.add_basic_value_flag(
        "CodeView",
        FlagBehavior::Warning,
        context.i32_type().const_int(1, false),
    );
    let (debug, _) = llvm.create_debug_info_builder(
        true,
        // CodeView has no Scoop language code. Line-only records use the
        // conventional C code without claiming C source types or variables.
        DWARFSourceLanguage::C,
        &source_path(&source.file),
        "",
        "Scoop",
        false,
        "",
        0,
        "",
        DWARFEmissionKind::LineTablesOnly,
        0,
        false,
        false,
        "",
        "",
    );
    Some(debug)
}

pub(super) fn attach_function<'ctx>(
    context: &'ctx Context,
    debug: &DebugInfoBuilder<'ctx>,
    function: &Function,
    llvm_function: inkwell::values::FunctionValue<'ctx>,
) -> Option<(DILocation<'ctx>, DILocation<'ctx>)> {
    let source = function.debug_source.as_ref()?;
    let file = debug.create_file(&source_path(&source.file), "");
    let signature = debug.create_subroutine_type(file, None, &[], DIFlags::ZERO);
    let scope = debug.create_function(
        file.as_debug_info_scope(),
        &source.name,
        Some(function.symbol()),
        file,
        source.line,
        signature,
        false,
        true,
        source.line,
        DIFlags::ZERO,
        false,
    );
    llvm_function.set_subprogram(scope);
    let entry = debug.create_debug_location(
        context,
        source.line,
        source.column,
        scope.as_debug_info_scope(),
        None,
    );
    // A declaration is not the location of every statement in its body.
    let unknown = debug.create_debug_location(context, 0, 0, scope.as_debug_info_scope(), None);
    Some((entry, unknown))
}

fn source_path(path: &str) -> std::borrow::Cow<'_, str> {
    // LLVM's CodeView path normalization does not understand Windows verbatim
    // prefixes. Use the equivalent DOS/UNC spelling for debugger lookup.
    dunce::simplified(std::path::Path::new(path)).to_string_lossy()
}
