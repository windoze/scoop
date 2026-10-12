//! Native records retain checked C geometry across physical ABI planning.

use super::*;

pub(super) struct PreparedScoopExtern {
    pub(super) id: mir::ExternFunctionId,
    pub(super) symbol: identity::NativeExternalSymbolKey,
    pub(super) library: identity::NativeLibraryBinding,
}

pub(crate) struct PreparedNativeAbi {
    pub(super) canonical_c_abi: lir::CanonicalCAbiMetadata,
    pub(super) contracts: Vec<identity::NativeExternalContractRecord>,
    pub(super) link_requirements: Vec<NativeLinkRequirementRecord>,
    pub(super) callback_signatures:
        HashMap<mir::FunctionTypeId, identity::CanonicalCAbiSignatureFingerprint>,
    pub(super) scoop_externs: Vec<PreparedScoopExtern>,
}

impl PreparedNativeAbi {
    pub(crate) fn complete(
        mut self,
        module: &mir::Module,
        enums: &lir::EnumDefs,
        plans: &abi::plans::Signatures,
    ) -> LoweredNativeAbi {
        for external in self.scoop_externs {
            let function = &module.extern_functions[external.id];
            let physical = plans
                .plans
                .scoop(plans.requests.scoop_externs[&external.id]);
            let signature = canonical_scoop_signature(
                module,
                enums,
                identity::ExactCallableSignature::new(
                    identity::Effect::Ordinary,
                    None,
                    function
                        .params
                        .iter()
                        .map(|ty| exact_type_record(module, ty).id())
                        .collect(),
                    exact_type_record(module, function.result.native_type()).id(),
                ),
                &function.params,
                function.result.native_type(),
                function.gc_effect,
                physical,
            );
            let contract = identity::NativeExternalContract::scoop_function(
                external.library,
                signature,
                identity::TargetCallingConvention::Cdecl,
            );
            self.contracts.push(
                identity::NativeExternalContractRecord::new(
                    function.source_contract.id(),
                    external.symbol,
                    contract,
                )
                .expect("validated Scoop externs have encodable physical contracts"),
            );
        }
        LoweredNativeAbi {
            canonical_c_abi: self.canonical_c_abi,
            native_externals: lir::NativeExternalMetadata::checked(
                self.contracts,
                self.link_requirements,
            )
            .expect("validated source externs normalize to one target contract each"),
            callback_signatures: self.callback_signatures,
        }
    }
}
