// Scoop C runtime: test-only exports.
//
// 说明：
// - 这些符号仅用于 `tests/fixtures/run-pass/*` 的可执行回归；
// - 不承诺稳定 ABI（生产环境不应依赖）。

#include <stdint.h>

#include "platform/unwind.h"

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
