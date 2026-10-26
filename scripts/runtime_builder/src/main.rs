//! Script helper: use the existing runtime compiler and report its output files.
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use clap::Parser;
use scoop::{RuntimeBuildRequest, RuntimeOptimization, build_runtime};
use scoop_toolchain::{CToolchainOptions, ResolvedTargetProfile};

#[derive(Parser)]
struct Arguments {
    #[arg(long)]
    enable_gc_stress: bool,
    #[arg(long)]
    enable_startup_stats: bool,
    #[arg(long)]
    target: String,
    #[arg(long)]
    runtime_root: PathBuf,
    #[arg(long)]
    vendor_root: PathBuf,
    #[arg(long)]
    cache_dir: PathBuf,
    #[arg(long)]
    cc: Option<PathBuf>,
    #[arg(long)]
    native_sysroot: Option<PathBuf>,
    #[arg(long)]
    unwind_prefix: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let tools = CToolchainOptions {
        compiler: args.cc,
        native_sysroot: args.native_sysroot,
    };
    let target = ResolvedTargetProfile::resolve_with_cached(&args.target, &tools, &args.cache_dir)?;
    let output = build_runtime(RuntimeBuildRequest {
        target: &target,
        runtime_root: &args.runtime_root,
        vendor_root: &args.vendor_root,
        cache_root: &args.cache_dir,
        optimization: RuntimeOptimization::Optimized,
        enable_gc_stress: args.enable_gc_stress,
        enable_startup_stats: args.enable_startup_stats,
        unwind_prefix: args.unwind_prefix.as_deref(),
    })?;
    let root = output
        .index()
        .parent()
        .ok_or("runtime index has no parent")?;
    let files = std::iter::once(output.index())
        .chain(
            output
                .objects()
                .objects()
                .iter()
                .map(|object| object.path()),
        )
        .map(|path| {
            path.strip_prefix(root)
                .map(|relative| relative.as_os_str().as_bytes().to_vec())
        })
        .collect::<Result<Vec<_>, _>>()?;
    println!(
        "{}",
        serde_json::json!({
            "root": root.as_os_str().as_bytes(),
            "files": files,
            "cache_hit": output.cache_hit(),
        })
    );
    Ok(())
}
