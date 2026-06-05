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
#include "scoop_gc_root_map_internal.h"
#include "scoop_root_frame.h"
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

typedef struct ScoopTestExplicitRootFrameTwoSlots {
  ScoopRootFrameHeader hdr;
  void *slot0;
  void *slot1;
} ScoopTestExplicitRootFrameTwoSlots;

typedef struct ScoopTestExplicitRootFrameZeroSlots {
  ScoopRootFrameHeader hdr;
} ScoopTestExplicitRootFrameZeroSlots;

typedef struct ScoopTestExplicitRootFrameOneSlot {
  ScoopRootFrameHeader hdr;
  void *slot0;
} ScoopTestExplicitRootFrameOneSlot;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopTestExplicitRootFrameTwoSlots, hdr) == 0,
               "explicit test frame header must be the first field");
_Static_assert(offsetof(ScoopTestExplicitRootFrameZeroSlots, hdr) == 0,
               "explicit zero-slot test frame header must be the first field");
_Static_assert(offsetof(ScoopTestExplicitRootFrameOneSlot, hdr) == 0,
               "explicit one-slot test frame header must be the first field");
#endif

static const uint32_t scoop_test_explicit_root_frame_two_slots_offsets[] = {
    offsetof(ScoopTestExplicitRootFrameTwoSlots, slot0),
    offsetof(ScoopTestExplicitRootFrameTwoSlots, slot1),
};

static const uint32_t scoop_test_explicit_root_frame_one_slot_offsets[] = {
    offsetof(ScoopTestExplicitRootFrameOneSlot, slot0),
};

static const ScoopRootFrameDesc scoop_test_explicit_root_frame_two_slots_desc = {
    .slot_count = 2,
    .slot_offsets = scoop_test_explicit_root_frame_two_slots_offsets,
};

static const ScoopRootFrameDesc scoop_test_explicit_root_frame_zero_slots_desc = {
    .slot_count = 0,
    .slot_offsets = 0,
};

static const ScoopRootFrameDesc scoop_test_explicit_root_frame_one_slot_desc = {
    .slot_count = 1,
    .slot_offsets = scoop_test_explicit_root_frame_one_slot_offsets,
};

typedef struct ScoopTestExplicitRootFrameCapture {
  void **slots[3];
  void *values[3];
  uint32_t count;
} ScoopTestExplicitRootFrameCapture;

static void scoop_test_explicit_root_frame_capture_slot(void **slot, void *ctx) {
  if (slot == 0 || ctx == 0) {
    return;
  }

  ScoopTestExplicitRootFrameCapture *capture = (ScoopTestExplicitRootFrameCapture *)ctx;
  if (capture->count >= 3) {
    return;
  }

  capture->slots[capture->count] = slot;
  capture->values[capture->count] = *slot;
  capture->count += 1;
}

uintptr_t scoop_test_explicit_root_frame_top(void) {
  return (uintptr_t)__scoop_explicit_root_frame_top;
}

intptr_t scoop_test_explicit_root_frame_root_map_smoke(void) {
  ScoopGcManagedRootMap empty_map = scoop_gc_managed_root_map_from_explicit_frame_top(0);
  ScoopGcRootMapVisitResult empty_result = {0};
  const uint64_t empty_visited = scoop_gc_root_map_visit_slots(
      &empty_map, scoop_test_explicit_root_frame_capture_slot, 0, &empty_result);
  if (empty_visited != 0 || empty_result.slots_visited != 0 || empty_result.units_hit != 0 ||
      empty_result.visit_error != SCOOP_GC_ROOT_MAP_VISIT_OK) {
    return -1;
  }

  ScoopTestExplicitRootFrameTwoSlots bottom = {0};
  ScoopTestExplicitRootFrameZeroSlots middle = {0};
  ScoopTestExplicitRootFrameOneSlot top = {0};

  bottom.hdr.prev = 0;
  bottom.hdr.desc = &scoop_test_explicit_root_frame_two_slots_desc;
  bottom.slot0 = (void *)(uintptr_t)0x1111u;
  bottom.slot1 = (void *)(uintptr_t)0x2222u;

  middle.hdr.prev = &bottom.hdr;
  middle.hdr.desc = &scoop_test_explicit_root_frame_zero_slots_desc;

  top.hdr.prev = &middle.hdr;
  top.hdr.desc = &scoop_test_explicit_root_frame_one_slot_desc;
  top.slot0 = (void *)(uintptr_t)0x3333u;

  ScoopRootFrameHeader *saved_top = __scoop_explicit_root_frame_top;
  __scoop_explicit_root_frame_top = &top.hdr;

  ScoopTestExplicitRootFrameCapture capture = {0};
  ScoopGcManagedRootMap map =
      scoop_gc_managed_root_map_from_explicit_frame_top(__scoop_explicit_root_frame_top);
  ScoopGcRootMapVisitResult result = {0};
  const uint64_t visited = scoop_gc_root_map_visit_slots(
      &map, scoop_test_explicit_root_frame_capture_slot, (void *)&capture, &result);

  __scoop_explicit_root_frame_top = saved_top;

  if (visited != 3 || result.slots_visited != 3) {
    return -2;
  }
  if (result.units_hit != 3) {
    return -3;
  }
  if (result.visit_error != SCOOP_GC_ROOT_MAP_VISIT_OK) {
    return -4;
  }
  if (capture.count != 3) {
    return -5;
  }
  if (capture.slots[0] != &top.slot0 || capture.slots[1] != &bottom.slot0 ||
      capture.slots[2] != &bottom.slot1) {
    return -6;
  }
  if (capture.values[0] != top.slot0 || capture.values[1] != bottom.slot0 ||
      capture.values[2] != bottom.slot1) {
    return -7;
  }
  return 1;
}

// 一个最小可调用的 C 函数：`Int + Int -> Int`（按 host word-size）。
//
// 约定：
// - Scoop `Int` 在 early stage 的 ABI 采用 `intptr_t`（见 codegen 的 `word_bit_width` 映射）。
intptr_t scoop_test_add_int(intptr_t a, intptr_t b) {
  return a + b;
}

// 外部全局变量回归：供 `@Extern var` load/store codegen 链接到真实 C storage。
intptr_t scoop_test_extern_global_counter = 0;

typedef struct ScoopTestIntPair {
  intptr_t first;
  intptr_t second;
} ScoopTestIntPair;

// 一个最小的 aggregate 返回 helper。
//
// 约定：
// - direct `@Extern` 与 `FunPtr<(Int) -> (Int, Int)>` 都应走同一套目标机 native ABI；
// - 这里直接返回 C struct by value，供 direct/indirect parity fixture 验证 native aggregate return。
ScoopTestIntPair scoop_test_make_int_pair(intptr_t seed) {
  ScoopTestIntPair out;
  out.first = seed + 1;
  out.second = seed + 2;
  return out;
}

// test-only named intrinsic runtime entry：返回并递增一个 C 侧计数器。
//
// 说明：
// - P4-T01d 需要一条可审计的 RuntimeCall dummy entry；
// - 这里故意让行为依赖外部可变状态，避免编译器把它伪装成纯 IR 常量。
static intptr_t scoop_test_named_intrinsic_dummy_runtime_counter = 700;

intptr_t scoop_test_named_intrinsic_dummy_runtime(void) {
  scoop_test_named_intrinsic_dummy_runtime_counter += 1;
  return scoop_test_named_intrinsic_dummy_runtime_counter;
}

static uint64_t scoop_test_sync_mutex_destroy_calls = 0;
static uint64_t scoop_test_sync_condvar_destroy_calls = 0;
static uint64_t scoop_test_sync_once_destroy_calls = 0;

void scoop_runtime_test_sync_mutex_destroyed(void) {
  (void)__atomic_fetch_add(&scoop_test_sync_mutex_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

void scoop_runtime_test_sync_condvar_destroyed(void) {
  (void)__atomic_fetch_add(&scoop_test_sync_condvar_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

void scoop_runtime_test_sync_once_destroyed(void) {
  (void)__atomic_fetch_add(&scoop_test_sync_once_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

void scoop_test_sync_destroy_counts_reset(void) {
  __atomic_store_n(&scoop_test_sync_mutex_destroy_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_sync_condvar_destroy_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_sync_once_destroy_calls, 0u, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_mutex_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_mutex_destroy_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_condvar_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_condvar_destroy_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_once_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_once_destroy_calls, __ATOMIC_SEQ_CST);
}

typedef struct ScoopTestReleaseHookProbe {
  uint64_t magic;
  intptr_t id;
  uint32_t released;
  struct ScoopTestReleaseHookProbe *next;
} ScoopTestReleaseHookProbe;

static const uint64_t SCOOP_TEST_RELEASE_HOOK_PROBE_MAGIC = 0x5c00b715e0000001ull;

static ScoopTestReleaseHookProbe *scoop_test_release_hook_probe_head = 0;
static uint64_t scoop_test_release_hook_probe_release_calls = 0;
static uint64_t scoop_test_release_hook_probe_duplicate_calls = 0;
static uint64_t scoop_test_release_hook_probe_invalid_calls = 0;
static uint64_t scoop_test_release_hook_probe_live_handles = 0;
static intptr_t scoop_test_release_hook_probe_last_released_id = 0;
static intptr_t scoop_test_release_hook_probe_expected_exit_releases = -1;
static intptr_t scoop_test_release_hook_probe_expected_exit_live = -1;
static uint32_t scoop_test_release_hook_probe_atexit_registered = 0;

static ScoopTestReleaseHookProbe *scoop_test_release_hook_probe_find(uintptr_t raw) {
  for (ScoopTestReleaseHookProbe *it = scoop_test_release_hook_probe_head; it != 0; it = it->next) {
    if ((uintptr_t)it == raw) {
      return it;
    }
  }
  return 0;
}

static void scoop_test_release_hook_probe_at_exit(void) {
  const intptr_t expected_releases = __atomic_load_n(&scoop_test_release_hook_probe_expected_exit_releases,
                                                     __ATOMIC_SEQ_CST);
  const intptr_t expected_live = __atomic_load_n(&scoop_test_release_hook_probe_expected_exit_live,
                                                __ATOMIC_SEQ_CST);
  if (expected_releases < 0 || expected_live < 0) {
    return;
  }

  const intptr_t actual_releases =
      (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_release_calls, __ATOMIC_SEQ_CST);
  const intptr_t actual_live =
      (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_live_handles, __ATOMIC_SEQ_CST);
  const intptr_t duplicate_calls =
      (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_duplicate_calls, __ATOMIC_SEQ_CST);
  const intptr_t invalid_calls =
      (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_invalid_calls, __ATOMIC_SEQ_CST);

  if (actual_releases != expected_releases || actual_live != expected_live || duplicate_calls != 0 ||
      invalid_calls != 0) {
    (void)fprintf(stderr,
                  "release hook atexit mismatch: releases=%ld expected=%ld live=%ld "
                  "expected_live=%ld duplicate=%ld invalid=%ld\n",
                  (long)actual_releases,
                  (long)expected_releases,
                  (long)actual_live,
                  (long)expected_live,
                  (long)duplicate_calls,
                  (long)invalid_calls);
    abort();
  }
}

void scoop_test_release_hook_probe_reset(void) {
  ScoopTestReleaseHookProbe *it = scoop_test_release_hook_probe_head;
  while (it != 0) {
    ScoopTestReleaseHookProbe *next = it->next;
    free(it);
    it = next;
  }
  scoop_test_release_hook_probe_head = 0;
  __atomic_store_n(&scoop_test_release_hook_probe_release_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_duplicate_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_invalid_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_live_handles, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_last_released_id, 0, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_expected_exit_releases, -1, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_expected_exit_live, -1, __ATOMIC_SEQ_CST);
}

uintptr_t scoop_test_release_hook_probe_create(intptr_t id) {
  ScoopTestReleaseHookProbe *probe = (ScoopTestReleaseHookProbe *)malloc(sizeof(ScoopTestReleaseHookProbe));
  if (probe == 0) {
    abort();
  }
  probe->magic = SCOOP_TEST_RELEASE_HOOK_PROBE_MAGIC;
  probe->id = id;
  probe->released = 0;
  probe->next = scoop_test_release_hook_probe_head;
  scoop_test_release_hook_probe_head = probe;
  (void)__atomic_fetch_add(&scoop_test_release_hook_probe_live_handles, 1u, __ATOMIC_SEQ_CST);
  return (uintptr_t)probe;
}

void scoop_test_release_hook_probe_release(uintptr_t raw) {
  ScoopTestReleaseHookProbe *probe = scoop_test_release_hook_probe_find(raw);
  if (probe == 0 || probe->magic != SCOOP_TEST_RELEASE_HOOK_PROBE_MAGIC) {
    (void)__atomic_fetch_add(&scoop_test_release_hook_probe_invalid_calls, 1u, __ATOMIC_SEQ_CST);
    return;
  }
  if (__atomic_exchange_n(&probe->released, 1u, __ATOMIC_SEQ_CST) != 0) {
    (void)__atomic_fetch_add(&scoop_test_release_hook_probe_duplicate_calls, 1u, __ATOMIC_SEQ_CST);
    return;
  }
  __atomic_store_n(&scoop_test_release_hook_probe_last_released_id, probe->id, __ATOMIC_SEQ_CST);
  (void)__atomic_fetch_add(&scoop_test_release_hook_probe_release_calls, 1u, __ATOMIC_SEQ_CST);
  (void)__atomic_fetch_sub(&scoop_test_release_hook_probe_live_handles, 1u, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_release_hook_probe_id(uintptr_t raw) {
  ScoopTestReleaseHookProbe *probe = scoop_test_release_hook_probe_find(raw);
  if (probe == 0 || probe->magic != SCOOP_TEST_RELEASE_HOOK_PROBE_MAGIC) {
    return -1;
  }
  return probe->id;
}

intptr_t scoop_test_release_hook_probe_release_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_release_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_release_hook_probe_duplicate_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_duplicate_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_release_hook_probe_invalid_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_invalid_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_release_hook_probe_live_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_release_hook_probe_live_handles, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_release_hook_probe_last_id(void) {
  return __atomic_load_n(&scoop_test_release_hook_probe_last_released_id, __ATOMIC_SEQ_CST);
}

void scoop_test_release_hook_probe_expect_at_exit(intptr_t release_count, intptr_t live_count) {
  __atomic_store_n(&scoop_test_release_hook_probe_expected_exit_releases, release_count,
                   __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_release_hook_probe_expected_exit_live, live_count, __ATOMIC_SEQ_CST);
  if (__atomic_exchange_n(&scoop_test_release_hook_probe_atexit_registered, 1u, __ATOMIC_SEQ_CST) ==
      0) {
    if (atexit(scoop_test_release_hook_probe_at_exit) != 0) {
      abort();
    }
  }
}

// 返回 `scoop_test_add_int` 的函数地址，作为 `FunPtr<(Int, Int) -> Int>` 的 runtime 落点。
//
// 说明：
// - 该转换在 C 标准中属于实现定义行为，但在我们支持的 host 平台上是可行的；
// - v0 阶段 `FunPtr<F>` 在 LLVM codegen 中被视为 `word-sized address`（unsigned int）。
uintptr_t scoop_test_get_add_int_funptr(void) {
  return (uintptr_t)&scoop_test_add_int;
}

// 返回 `scoop_test_make_int_pair` 的函数地址，作为
// `FunPtr<(Int) -> (Int, Int)>` 的 runtime 落点。
uintptr_t scoop_test_get_make_int_pair_funptr(void) {
  return (uintptr_t)&scoop_test_make_int_pair;
}

void scoop_test_gc_collect_in_native(void);

// 返回 `scoop_test_gc_collect_in_native` 的函数地址，供 native `FunPtr<() -> Unit>`
// 通过同一 boundary scaffold 验证 `enter_native/leave_native` + roots 暴露。
uintptr_t scoop_test_get_gc_collect_in_native_funptr(void) {
  return (uintptr_t)&scoop_test_gc_collect_in_native;
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
// - 该函数由 sysroot 内部测试 helper `__scoop_stackmap_statepoint_smoke()` 调用；
// - 该 helper 属于显式 opt-in 的 stackmap smoke：编译器只会对包含该调用点的函数恢复
//   `gc "statepoint-example"`，从而让该调用点重新进入 statepoint/stackmap pipeline；
// - 同时仍必须保持为 ordinary managed runtime 调用，不能走 `@Extern` +
//   `enter_native/leave_native` leaf lowering；T1510c1 已明确这类调用点不再生成 statepoint；
// - 当调用点按上述 contract 保持为 ordinary managed call 时，`rewrite-statepoints-for-gc` 会把它
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

// 最小 stable-handle token slot：模拟 reactor / completion callback / future executor
// 在 native 状态里长期保存 `GcHandle.raw`，并在稍后把该 token 回传给 Scoop。
//
// 约定：
// - slot 里只保存 word-sized token，不保存对象地址；
// - `take` 会消费当前 token 并清空 slot，模拟“一次回调取走 wake token”；
// - 这些 helper 不会替调用方调用 `handleDrop`；ownership 仍由测试侧明确验证。
static uintptr_t scoop_test_handle_token_slot = 0;

void scoop_test_handle_token_slot_reset(void) {
  __atomic_store_n(&scoop_test_handle_token_slot, (uintptr_t)0, __ATOMIC_SEQ_CST);
}

void scoop_test_handle_token_slot_store(uintptr_t handle_raw) {
  __atomic_store_n(&scoop_test_handle_token_slot, handle_raw, __ATOMIC_SEQ_CST);
}

uintptr_t scoop_test_handle_token_slot_take(void) {
  return __atomic_exchange_n(&scoop_test_handle_token_slot, (uintptr_t)0, __ATOMIC_SEQ_CST);
}

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
