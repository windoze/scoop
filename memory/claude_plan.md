# Current Task: T0117 — `@Extern(lib=...)` 参数传递到链接器

## Status: COMPLETED

## Summary
Audited and completed the `@Extern(lib=...)` parameter pipeline:

1. **Discovery**: The linker parameter passing was ALREADY fully implemented — `collect_extern_libs()` → `LoweredHir.extern_libs` → `link_objs()` → `clang -l<name>`. Unit test `clang_link_command_includes_extern_libs` already verified this.

2. **Changes made**:
   - `ExternFun` struct (`hir/mod.rs`): Added `lib: Option<String>` field for per-function lib traceability
   - `extern_fun_of_decl` (`hir/lower/util.rs`): Populates `ExternFun.lib` from `parse_extern_annotation_args().lib`
   - `toolchain.rs`: Updated existing test to verify `ExternFun.lib` field
   - New run-pass fixture: `extern_lib_link_basic.scoop` — calls `labs()` via `@Extern(lib = "c", name = "labs")`
   - New Cone fixture: `extern_lib_link_basic/` — same test in Cone project form

3. **Verification**: 139 unit tests + 791 fixtures pass
