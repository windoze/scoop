//! 跨动态库（dylib/so）场景下的 once/guard 行为回归。
//!
//! 背景（TODO T0919）：
//! - `object` / `companion object` 的 init guard 若在多个动态库中各自生成一份，
//!   则单纯用 `&guard` 调用 `scoop_once_begin/end` 会导致“每个动态库都初始化一次”。
//! - runtime 提供 `scoop_once_guard_canonicalize`，通过 `dlsym(RTLD_DEFAULT, ...)`
//!   选出进程内 canonical guard 地址，使多个动态库最终共享同一 guard word。

// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use std::ffi::{CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use libc::c_void;

const GUARD_SYMBOL: &str = "__scoop_object_guard__fixtures.once_guard.Object";

struct Dylib {
    handle: *mut c_void,
}

impl Dylib {
    unsafe fn open(path: &Path) -> Self {
        let path_c =
            CString::new(path.to_string_lossy().as_bytes()).expect("dlopen path contains NUL");

        // 明确使用 RTLD_GLOBAL：确保 `dlsym(RTLD_DEFAULT, ...)` 能搜索到该 image。
        let flags = libc::RTLD_NOW | libc::RTLD_GLOBAL;

        // 清理旧错误，便于调试。
        let _ = unsafe { libc::dlerror() };
        let handle = unsafe { libc::dlopen(path_c.as_ptr(), flags) };
        if handle.is_null() {
            let err = dlerror_string().unwrap_or_else(|| "unknown dlopen error".to_string());
            panic!("dlopen failed: {err} (path={})", path.display());
        }

        Self { handle }
    }

    unsafe fn sym<T>(&self, name: &CStr) -> T {
        // 清理旧错误，避免把上一次 dlerror 误判为本次 dlsym 的错误。
        let _ = unsafe { libc::dlerror() };
        let sym = unsafe { libc::dlsym(self.handle, name.as_ptr()) };

        let err = dlerror_string();
        if let Some(err) = err {
            panic!("dlsym failed: {err} (name={:?})", name);
        }
        if sym.is_null() {
            panic!("dlsym returned NULL without dlerror (name={:?})", name);
        }

        // SAFETY: 由调用方保证符号签名匹配。
        unsafe { std::mem::transmute_copy(&sym) }
    }
}

impl Drop for Dylib {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe {
            let _ = libc::dlclose(self.handle);
        }
        self.handle = std::ptr::null_mut();
    }
}

fn dlerror_string() -> Option<String> {
    let err = unsafe { libc::dlerror() };
    if err.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(err) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn plugin_code() -> String {
    // 说明：
    // - 这里用 `__asm("...")` 给 C 变量指定 “带 '.' 的符号名”，模拟 LLVM codegen 的命名约定；
    // - `plugin_access` 内部使用 `scoop_once_guard_canonicalize` 找到 canonical guard。
    format!(
        r#"
#include <stdint.h>

extern uint64_t* scoop_once_guard_canonicalize(const char* symbol_name, uint64_t* fallback_guard_word);
extern uint32_t scoop_once_begin(uint64_t* guard_word);
extern void scoop_once_end(uint64_t* guard_word);

// macOS：dlsym("NAME") 会按 C 语义查找，并自动补一个前导 '_' 变为 "_NAME"。
// 因此为了让 `dlsym(RTLD_DEFAULT, "{guard_symbol}")` 能命中，这里把实际导出符号名设为 "_{guard_symbol}"。
// Linux：dlsym 不会自动补 '_'，所以直接使用 "{guard_symbol}" 即可。
// Linux：使用 protected visibility 防止 RTLD_GLOBAL 下的符号互位 (interposition)，
// 确保每个 dylib 的 &local_guard 指向自己的副本。在 glibc >= 2.36 上，
// dlsym(RTLD_DEFAULT, ...) 能正确找到 protected 符号。
#if defined(__APPLE__)
uint64_t local_guard __asm("_{guard_symbol}") = 0;
#else
uint64_t __attribute__((visibility("protected"))) local_guard __asm("{guard_symbol}") = 0;
#endif

uintptr_t plugin_local_guard_addr(void) {{
  return (uintptr_t)&local_guard;
}}

uintptr_t plugin_canonical_guard_addr(void) {{
  uint64_t* guard = scoop_once_guard_canonicalize("{guard_symbol}", &local_guard);
  return (uintptr_t)guard;
}}

uint32_t plugin_access(uint64_t* init_count) {{
  uint64_t* guard = scoop_once_guard_canonicalize("{guard_symbol}", &local_guard);
  uint32_t should_init = scoop_once_begin(guard);
  if (should_init != 0) {{
    (*init_count)++;
    scoop_once_end(guard);
  }}
  return should_init;
}}
"#,
        guard_symbol = GUARD_SYMBOL
    )
}

fn compile_plugin(out_dir: &Path, name: &str) -> PathBuf {
    let src_path = out_dir.join(format!("{name}.c"));
    fs::write(&src_path, plugin_code()).expect("write plugin source");

    let scoop_once_c = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/c/scoop_once.c")
        .canonicalize()
        .expect("canonicalize runtime/c/scoop_once.c");

    let out_lib = {
        #[cfg(target_os = "macos")]
        {
            out_dir.join(format!("lib{name}.dylib"))
        }
        #[cfg(target_os = "linux")]
        {
            out_dir.join(format!("lib{name}.so"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (out_dir, name);
            unreachable!("unsupported platform for this test");
        }
    };

    let mut cmd = Command::new("clang");

    #[cfg(target_os = "macos")]
    {
        cmd.arg("-dynamiclib");
    }
    #[cfg(target_os = "linux")]
    {
        cmd.arg("-shared").arg("-fPIC");
    }

    // 优先可读的调试符号，便于定位 dlsym/链接问题。
    cmd.arg("-O0").arg("-g");

    cmd.arg("-o").arg(&out_lib);
    cmd.arg(&src_path);
    cmd.arg(&scoop_once_c);

    #[cfg(target_os = "linux")]
    {
        cmd.arg("-ldl").arg("-pthread");
    }

    let output = cmd.output().expect("spawn clang");
    if !output.status.success() {
        panic!(
            "clang failed (status={:?})\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    out_lib
}

#[test]
fn once_guard_is_canonical_across_dylibs() {
    // 用临时目录生成两个动态库：它们都定义同名 guard 符号，但地址不同。
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!(
        "scoop_once_guard_cross_dylib_{pid}_{stamp}",
        pid = std::process::id()
    ));
    fs::create_dir_all(&out_dir).expect("create temp dir");

    let lib_a_path = compile_plugin(&out_dir, "scoop_plugin_a");
    let lib_b_path = compile_plugin(&out_dir, "scoop_plugin_b");

    unsafe {
        type AddrFn = unsafe extern "C" fn() -> usize;
        type AccessFn = unsafe extern "C" fn(*mut u64) -> u32;

        let local_addr: &CStr = c"plugin_local_guard_addr";
        let canon_addr: &CStr = c"plugin_canonical_guard_addr";
        let access: &CStr = c"plugin_access";

        // 先加载 A 并触发一次访问，再加载 B：
        // - 覆盖“先访问后 dlopen”的真实插件场景；
        // - 也能验证 canonical guard 的选择不会在后续加载新 dylib 后发生漂移。
        let lib_a = Dylib::open(&lib_a_path);

        let a_local: AddrFn = lib_a.sym(local_addr);
        let a_canon: AddrFn = lib_a.sym(canon_addr);
        let a_access: AccessFn = lib_a.sym(access);

        let a_local_addr = a_local();
        let a_canon_before = a_canon();
        assert_eq!(a_canon_before, a_local_addr);

        // 观测副作用：第一次访问必须触发 init。
        let mut init_count: u64 = 0;
        assert_eq!(a_access(&mut init_count as *mut u64), 1);

        // 再加载 B，并验证它会复用 A 的 canonical guard（而不是各自初始化一次）。
        let lib_b = Dylib::open(&lib_b_path);

        let b_local: AddrFn = lib_b.sym(local_addr);
        let b_canon: AddrFn = lib_b.sym(canon_addr);
        let b_access: AccessFn = lib_b.sym(access);

        let b_local_addr = b_local();
        let a_canon_after = a_canon();
        let b_canon_after = b_canon();

        // 由于两个 dylib 都定义了同名 guard 符号，它们的本地 guard 地址必须不同。
        assert_ne!(a_local_addr, b_local_addr);

        // canonical guard 的选择必须稳定：加载新 dylib 后不能“漂移”到另一个地址。
        assert_eq!(a_canon_after, a_canon_before);
        assert_eq!(b_canon_after, a_canon_before);
        assert_ne!(b_local_addr, b_canon_after);

        // B 的访问不应再次触发 init；A/B 后续访问也不应触发。
        assert_eq!(b_access(&mut init_count as *mut u64), 0);
        assert_eq!(a_access(&mut init_count as *mut u64), 0);
        assert_eq!(b_access(&mut init_count as *mut u64), 0);
        assert_eq!(init_count, 1);
    }

    // 尽量清理临时目录；失败也不影响测试结果（主要用于本地调试）。
    let _ = fs::remove_dir_all(&out_dir);
}
