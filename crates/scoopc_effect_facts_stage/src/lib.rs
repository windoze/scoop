//! Effect-facts builder stage for the Scoop compiler.
//!
//! This crate owns the MIR-to-effect-facts construction and solver logic. The
//! data-only product remains in `scoopc_effect_facts`.

#![forbid(unsafe_code)]

pub use scoopc_ast as ast;
pub use scoopc_ast::parser;
pub use scoopc_hir::{hir, intrinsics, itable, resolve, session, typecheck, vtable};
pub use scoopc_mir::{mir, stable_id};

pub mod effect_facts;
pub mod source {
    pub use scoopc_source::*;
}
pub mod span {
    pub use scoopc_span::*;
}
pub mod ty {
    pub use scoopc_types::*;
}
pub mod opt {
    pub use scoop_project_model::OptLevel;
}

pub mod frontend {
    use miette::{Context as _, IntoDiagnostic as _, Result};
    use scoopc_hir::session::SessionOptions;
    use scoopc_hir::sysroot;
    use scoopc_source::SourceFile;

    /// Load default sysroot support sources needed by effect signature lowering.
    pub fn load_default_support_sources(
        session_options: &SessionOptions,
    ) -> Result<Vec<SourceFile>> {
        let mut support_paths = Vec::new();
        let sysroot_root = sysroot::Sysroot::default_path()
            .canonicalize()
            .into_diagnostic()
            .wrap_err("无法定位 sysroot 目录（T0143）")?;
        let sysroot_entries = sysroot::collect_auto_sysroot_source_entries(
            &sysroot_root,
            session_options.sysroot_overlay(),
            session_options.extra_sysroot_dependencies(),
        )?;

        support_paths.extend(
            sysroot_entries
                .into_iter()
                .map(|entry| (entry.path, entry.trusted_syslib)),
        );
        support_paths.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

        let mut out = Vec::with_capacity(support_paths.len());
        for (path, trusted_syslib) in support_paths {
            out.push(if trusted_syslib {
                SourceFile::load_trusted_syslib(&path)?
            } else {
                SourceFile::load_sysroot(&path)?
            });
        }
        Ok(out)
    }
}

pub use effect_facts::*;
