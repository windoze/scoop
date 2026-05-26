//! LLVM backend crate.
//!
//! This crate owns LLVM lowering and consumes the codegen handoff as direct
//! `scoopc_lir` / `scoopc_lir_facts` input. Frontend orchestration remains in
//! the `scoopc` pipeline wrapper.

#[cfg(feature = "llvm")]
pub mod llvm;

pub mod stackmap;

pub mod ast {
    pub use scoopc_lir::effect_lowered::source::{
        BinaryOp, CastOp, TopLevelFunCallBinding, TypeCheckOp, TypeKind,
    };
}

pub mod cone {
    pub use scoop_project_model::*;
}

pub mod effect_facts {
    pub use scoopc_lir::effect_facts::*;
}

pub mod effect_lowered {
    pub use scoopc_lir::effect_lowered::*;
}

pub mod intrinsics {
    pub use scoopc_lir::effect_lowered::source::intrinsics::*;
}

pub mod itable {
    pub use scoopc_lir::effect_lowered::source::{
        ClassItableEntry, ClassItableIndex, ITABLE_RECEIVER_REF_TYPE_ID, InterfaceIndex,
    };
}

pub mod mir {
    pub use scoopc_lir::mir::*;
}

pub mod opt {
    pub use scoopc_lir::opt::*;
}

pub mod source {
    pub use scoopc_lir::source::*;
}

pub mod span {
    pub use scoopc_lir::span::*;
}

pub mod stable_id {
    pub use scoopc_lir::stable_id::*;
}

pub mod syntax {
    pub mod char_literal {
        pub use scoopc_lir::effect_lowered::source::char_literal::*;
    }

    pub mod float_literal {
        pub use scoopc_lir::effect_lowered::source::float_literal::*;
    }

    pub mod int_literal {
        pub use scoopc_lir::effect_lowered::source::int_literal::*;
    }

    pub mod string_literal {
        pub use scoopc_lir::effect_lowered::source::string_literal::*;
    }
}

pub mod ty {
    pub use scoopc_lir::ty::*;
}

pub mod vtable {
    pub use scoopc_lir::effect_lowered::source::ClassVtableIndex;
}
