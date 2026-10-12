//! LLVM-independent part of codegen: classify complete physical call signatures.
//! The production driver reaches this through scoop-codegen. Lowering's tests
//! use the same classifier without loading LLVM or another lowering stage.

use scoop_lir as lir;

mod aggregate;
mod c;
mod layout;
mod registers;
mod scoop;

pub use c::{c_indirect_passing, c_integer_extension};

#[derive(Clone, Copy)]
enum ProcedureAbi {
    DarwinAarch64,
    SysvAmd64,
}

impl ProcedureAbi {
    fn for_target(target: lir::LirTargetProfile) -> Self {
        match target.id() {
            lir::TargetProfileId::DarwinAarch64 => Self::DarwinAarch64,
            lir::TargetProfileId::LinuxX86_64Gnu | lir::TargetProfileId::LinuxX86_64Musl => {
                Self::SysvAmd64
            }
        }
    }
}

pub fn plan(input: &lir::AbiPlanningInput) -> Result<lir::AbiPlanSet, lir::AbiValueError> {
    Ok(lir::AbiPlanSet {
        scoop: input
            .scoop
            .iter()
            .map(|(_, request)| scoop::classify(input, request))
            .collect::<Result<_, _>>()?,
        c: input
            .c
            .iter()
            .map(|(_, request)| c::classify(input, request))
            .collect::<Result<_, _>>()?,
    })
}

#[cfg(test)]
mod tests;
