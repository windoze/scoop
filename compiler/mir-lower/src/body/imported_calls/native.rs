use super::*;
use scoop_identity::{
    SourceCallingConvention, SourceExternFunctionAbi, SourceNativeExternalContract,
    SourceNativeLibraryBinding,
};

impl BodyLowerer<'_> {
    pub(super) fn release_native_target(
        &mut self,
        native: &crate::imported_callables::ImportedCFunction,
        arguments: &[hir::Expr],
    ) -> Option<mir::ExternFunctionId> {
        let source = &native.contract;
        let SourceNativeExternalContract::Function {
            symbol,
            library,
            abi: SourceExternFunctionAbi::C(_),
            calling_convention,
        } = source.contract()
        else {
            return None;
        };
        if let Some((id, _)) = self.extern_functions.iter().find(|(_, function)| {
            function.source_contract.id() == source.id()
                && function.abi == mir::ExternAbi::C(native.call_mode)
                && function.result.adaptation() == native.result.adaptation()
        }) {
            return Some(id);
        }
        let params = arguments
            .iter()
            .map(|argument| self.lower_type(argument.ty))
            .collect();
        let native_symbol = std::str::from_utf8(symbol.as_bytes())
            .expect("source symbols retain validated UTF-8")
            .to_owned();
        let library = match library {
            SourceNativeLibraryBinding::DefaultNativeNamespace => String::new(),
            SourceNativeLibraryBinding::LogicalLibrary(name) => name.as_str().to_owned(),
        };
        let result = native.result.map(|ty| self.lower_type(ty));
        Some(self.extern_functions.alloc(mir::ExternFunction {
            source_contract: source.clone(),
            source_name: native_symbol.clone(),
            native_symbol,
            library,
            abi: mir::ExternAbi::C(native.call_mode),
            calling_convention: match calling_convention {
                SourceCallingConvention::Cdecl => mir::CallingConvention::Cdecl,
            },
            gc_effect: mir::GcEffect::NoGc,
            params,
            result,
        }))
    }
}
