// Scoop C runtime: test-only exports.
//
// 说明：
// - 这些符号仅用于 `tests/fixtures/run-pass/*` 的可执行回归；
// - 不承诺稳定 ABI（生产环境不应依赖）。

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "platform/platform.h"
#include "platform/unwind.h"
#include "scoop_gc.h"
#include "scoop_stackmap.h"

// 运行时 GC helper（由具体 backend 提供实现）。
void scoop_gc_collect(void);

typedef struct ScoopTestUnwindDumpFramesState {
  uint32_t frame_index;
  uint32_t visited;
  uint32_t should_dump;
} ScoopTestUnwindDumpFramesState;

static uint32_t scoop_test_unwind_dump_frames_visitor(uintptr_t sp,
                                                      uintptr_t ra,
                                                      uintptr_t fp,
                                                      void *user_data) {
  if (user_data == 0) {
    return 0;
  }

  ScoopTestUnwindDumpFramesState *st = (ScoopTestUnwindDumpFramesState *)user_data;
  st->visited += 1;

  ScoopStackmapRecordRef rec = {0};
  const uint32_t hit = scoop_stackmap_registry_lookup(ra, &rec);

  if (st->should_dump) {
    if (hit) {
      (void)fprintf(stderr,
                    "[scooprt][unwind] frame=%u sp=0x%lx ra=0x%lx fp=0x%lx stackmap=hit "
                    "patchpoint_id=%llu\n",
                    (unsigned)st->frame_index,
                    (unsigned long)sp,
                    (unsigned long)ra,
                    (unsigned long)fp,
                    (unsigned long long)rec.patchpoint_id);
    } else {
      (void)fprintf(stderr,
                    "[scooprt][unwind] frame=%u sp=0x%lx ra=0x%lx fp=0x%lx stackmap=miss\n",
                    (unsigned)st->frame_index,
                    (unsigned long)sp,
                    (unsigned long)ra,
                    (unsigned long)fp);
    }
  }

  st->frame_index += 1;
  return 1;
}

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

// 帧校验工具输出（GC-FIX-TODO A3）：
// - 捕获当前线程的 stack walking ctx；
// - 逐帧打印 `(sp, ra, fp)`，并标注 stackmap registry lookup 是否命中 record；
// - 默认不输出（避免污染普通测试输出）；设置 `SCOOP_UNWIND_DUMP_FRAMES=1` 启用。
//
// 返回：
// - >0：实际枚举的帧数
// - 0：当前平台不支持或捕获失败
intptr_t scoop_test_unwind_dump_frames_and_stackmap_hits(void) {
  const char *dump_env = getenv("SCOOP_UNWIND_DUMP_FRAMES");
  const uint32_t should_dump = (dump_env != 0 && dump_env[0] == '1');

  // best-effort：仅用于诊断；找不到 stackmaps 不视为失败。
  (void)scoop_stackmap_registry_register_current_process();
  const uint32_t record_count = scoop_stackmap_registry_record_count();

  void *ctx = scoop_platform_unwind_ctx_capture();
  if (ctx == 0) {
    if (should_dump) {
      (void)fprintf(stderr, "[scooprt][unwind] ctx capture unsupported or failed\n");
    }
    return 0;
  }

  ScoopTestUnwindDumpFramesState state = {
      .frame_index = 0,
      .visited = 0,
      .should_dump = should_dump,
  };

  if (should_dump) {
    (void)fprintf(stderr, "[scooprt][unwind] stackmap_records=%u\n", (unsigned)record_count);
  }

  (void)scoop_platform_unwind_ctx_walk_frames(
      ctx, /*skip_frames=*/0, scoop_test_unwind_dump_frames_visitor, (void *)&state);
  scoop_platform_unwind_ctx_destroy(ctx);
  return (intptr_t)state.visited;
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

// 返回 stable handle 当前指向对象的地址（用于 pin/unpin + moving GC 回归）。
//
// 说明：
// - `@Extern` ABI 禁止直接透传 GC ref，因此该函数以 `UIntPtr`（word-sized integer）作为输入/输出；
// - Scoop 侧可先通过 `GC.handleNew(obj)` 得到 `GcHandle.raw`，再用本函数观测 `handleGet` 的指针值；
// - 在 moving/compaction GC 下，handle 表的槽位会被更新到新地址；而 pinned 对象应保持地址稳定。
uintptr_t scoop_test_handle_get_object_addr(uintptr_t handle_raw) {
  void *obj = scoop_handle_get((uint64_t)handle_raw);
  return (uintptr_t)obj;
}

// `@Extern` + stop-the-world（跨线程）+ InNative smoke（TODO T1512c）。
//
// 设计意图：
// - worker 线程通过 `@Extern("scoop_test_gc_sleep_in_native_ms")` 进入 native，并在该函数内阻塞一段时间；
// - 该调用点会由编译器自动插入 `enter_native/leave_native`，使 worker 线程状态机切换到 InNative；
// - main 线程在观测到 “worker 已进入 native” 后触发一次 `__scoop_gc_collect()`；
// - 期望：GC 不会等待 InNative 线程 park（避免死锁），并能通过 `native_roots` 保活 call-site roots。
static uint32_t scoop_test_gc_native_sleep_entered_flag = 0;

// 重置 “已进入 native” 标记（方便 fixtures 重复执行、避免跨进程/多用例残留）。
void scoop_test_gc_native_sleep_reset(void) {
  __atomic_store_n(&scoop_test_gc_native_sleep_entered_flag, 0u, __ATOMIC_SEQ_CST);
}

// 返回 1 表示 worker 已在 native sleep 内部；否则返回 0。
intptr_t scoop_test_gc_native_sleep_entered(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_gc_native_sleep_entered_flag, __ATOMIC_SEQ_CST);
}

// 在 native 内阻塞指定毫秒数（<=0 则仅设置 entered 标记后直接返回）。
void scoop_test_gc_sleep_in_native_ms(intptr_t ms) {
  __atomic_store_n(&scoop_test_gc_native_sleep_entered_flag, 1u, __ATOMIC_SEQ_CST);

  if (ms <= 0) {
    return;
  }
  scoop_platform_thread_sleep_millis((int64_t)ms);
}
