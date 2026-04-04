// Scoop C runtime: test-only exports.
//
// 说明：
// - 这些符号仅用于 `tests/fixtures/run-pass/*` 的可执行回归；
// - 不承诺稳定 ABI（生产环境不应依赖）。

#include <stdint.h>

#include "platform/unwind.h"
#include "scoop_stackmap.h"

// 运行时 GC helper（由具体 backend 提供实现）。
void scoop_gc_collect(void);

// 一个最小可调用的 C 函数：`Int + Int -> Int`（按 host word-size）。
//
// 约定：
// - Scoop `Int` 在 early stage 的 ABI 采用 `intptr_t`（见 codegen 的 `word_bit_width` 映射）。
intptr_t scoop_test_add_int(intptr_t a, intptr_t b) {
  return a + b;
}

// 返回 `scoop_test_add_int` 的函数地址，作为 `FunPtr<(Int, Int) -> Int>` 的 runtime 落点。
//
// 说明：
// - 该转换在 C 标准中属于实现定义行为，但在我们支持的 host 平台上是可行的；
// - v0 阶段 `FunPtr<F>` 在 LLVM codegen 中被视为 `word-sized address`（unsigned int）。
uintptr_t scoop_test_get_add_int_funptr(void) {
  return (uintptr_t)&scoop_test_add_int;
}

// 捕获当前线程的 backtrace（instruction pointers）。
//
// 说明：
// - 该符号用于回归验证 platform/unwind 的基本可用性；
// - 不承诺稳定 ABI；只保证在本仓库的测试/fixtures 中可用。
uint32_t scoop_test_unwind_capture_ips(uintptr_t *out_ips, uint32_t out_cap, uint32_t skip_frames) {
  return scoop_platform_unwind_capture_ips(out_ips, out_cap, skip_frames);
}

// statepoint/stackmap smoke：验证 runtime registry 非空，且“调用点的 return address”可查到 record。
//
// 设计意图（T1504b）：
// - 该函数将被 Scoop 代码通过 `@Extern("scoop_test_stackmap_statepoint_smoke")` 调用；
// - 由于 entry `main` 带 `gc "statepoint-example"`，`rewrite-statepoints-for-gc` 会把对本函数的调用点
//   重写为 statepoint，从而在 `.llvm_stackmaps` 中产生对应 record；
// - 在本函数内部读取 `__builtin_return_address(0)` 即得到该调用点的 return address；
// - 用该地址查询 registry，验证 return address ↔ record 的映射规则在真实产物上成立。
//
// 返回：
// - 1：通过（registry 非空且 lookup 成功）
// - 0：失败（未发现 stackmaps 或 lookup 失败）
__attribute__((noinline)) intptr_t scoop_test_stackmap_statepoint_smoke(void) {
  // 说明：
  // - 该符号仅用于 fixtures/run-pass 的 smoke；这里允许“强制重扫一次”，便于在不同平台/链接策略下
  //   排除初始化时机导致的误差。
  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();

  const uint32_t n = scoop_stackmap_registry_record_count();
  if (n == 0) {
    return -1;
  }

#if defined(__clang__) || defined(__GNUC__)
  const uintptr_t ra = (uintptr_t)__builtin_return_address(0);
  if (ra == 0) {
    return -2;
  }
  ScoopStackmapRecordRef rec = {0};
  if (!scoop_stackmap_registry_lookup(ra, &rec)) {
    return -3;
  }
  if (rec.patchpoint_id == 0) {
    return -4;
  }
  return 1;
#else
  return -5;
#endif
}

// `@Extern` + enter_native/leave_native 回归：在 native 内部触发一次 GC。
//
// 说明：
// - 该符号由 fixtures 通过 `@Extern("scoop_test_gc_collect_in_native")` 调用；
// - 调用点应当由编译器自动插入 `scoop_enter_native(root_slots, len)` / `scoop_leave_native()`；
// - 这样当 `scoop_gc_collect()` 运行时，即使当前线程处于 InNative，GC 仍可通过 native_roots
//   扫描/保活 call-site 上的对象引用（避免误回收）。
void scoop_test_gc_collect_in_native(void) { scoop_gc_collect(); }
