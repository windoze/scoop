// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
#include <stdatomic.h>
#include <inttypes.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc.h"
#include "scoop_gc_backend.h"
#include "scoop_gc_immix_internal.h"
#include "scoop_stackmap.h"
#include "scoop_tls_internal.h"
#include "platform/platform.h"
#include "platform/unwind.h"

// TLS（thread-local storage）抽象层。
//
// 说明：
// - 运行时的 GC/effect 状态必须是线程本地的（见 PLAN.md §9 / TODO T0903/T0905/T0906）。
// - 早期阶段我们只需要“有 TLS 骨架且可链接”，具体字段会在后续任务中逐步补齐。
// - 优先使用 C11 `_Thread_local`；否则降级到常见编译器扩展。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
#define SCOOP_THREAD_LOCAL _Thread_local
#elif defined(_MSC_VER)
#define SCOOP_THREAD_LOCAL __declspec(thread)
#elif defined(__GNUC__) || defined(__clang__)
#define SCOOP_THREAD_LOCAL __thread
#else
#define SCOOP_THREAD_LOCAL
#endif

// 运行时 debug 日志（编译期开关）。
//
// 说明：
// - 该宏用于让早期 runtime 具备“可观察性”，便于手动调试初始化/ABI；
// - 默认关闭，避免污染用户程序输出；
// - 可在编译 C runtime 时通过 `-DSCOOP_RT_DEBUG=1` 打开（TODO T0901）。
#ifndef SCOOP_RT_DEBUG
#define SCOOP_RT_DEBUG 0
#endif

#if SCOOP_RT_DEBUG
#define SCOOP_RT_LOG(...) \
  do { \
    (void)fprintf(stderr, "[scooprt] " __VA_ARGS__); \
    (void)fputc('\n', stderr); \
    (void)fflush(stderr); \
  } while (0)
#else
#define SCOOP_RT_LOG(...) \
  do { \
    (void)0; \
  } while (0)
#endif

// 运行时字符串对象（early stage）。
//
// 说明：
// - `ScoopString` 是 GC-managed heap 对象：以 `ScoopGcObjectHeader` 开头（与 `scoop_alloc` 对齐）；
// - 字符串数据视为 UTF-8 字节序列；
// - `data` 当前仍是 native 指针（addrspace(0)），可指向：
//   - 只读静态数据（例如字符串字面量的全局常量）；或
//   - `malloc` 分配的 owned buffer（当前阶段未接入 type descriptor/release，后续任务会补齐）。
typedef struct ScoopString {
  // 作为 GC-managed 对象，必须以对象头开头（与 `scoop_alloc` 约定一致）。
  ScoopGcObjectHeader hdr;
  uint64_t len;
  const uint8_t *data;
} ScoopString;

// ABI 断言：保证 codegen 侧对 `ScoopString` 的布局假设稳定。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(uint64_t) == 8, "uint64_t must be 8 bytes");
_Static_assert(offsetof(ScoopString, hdr) == 0, "ScoopString.hdr offset must be 0");
_Static_assert(offsetof(ScoopString, len) == sizeof(ScoopGcObjectHeader),
               "ScoopString.len offset must be sizeof(ScoopGcObjectHeader)");
_Static_assert(offsetof(ScoopString, data) == (sizeof(ScoopGcObjectHeader) + 8u),
               "ScoopString.data offset must be header + 8");
_Static_assert((sizeof(ScoopString) % sizeof(void *)) == 0,
               "ScoopString size must be pointer-aligned");
#endif

static const uint8_t SCOOP_DOT_BYTES[1] = {'.'};

static const uint8_t SCOOP_SLASH_BYTES[1] = {'/'};
#ifdef _WIN32
static const uint8_t SCOOP_BACKSLASH_BYTES[1] = {'\\'};
#endif

// 前置声明：String helper（定义在文件后部）。
static const ScoopString *scoop_string_empty(void);
static const ScoopString *scoop_string_from_static_bytes(const uint8_t *value, uint64_t len);

// 运行时全局状态（early stage）。
//
// 说明：
// - 当前阶段只需要“可被初始化且可观察”，不引入 GC/TLS/线程；
// - 未来会扩展为：线程注册、TLS、effect slots、GC heap 等（TODO T0903/T0904/...）。
static uint32_t scoop_rt_initialized = 0;
static uint32_t scoop_rt_init_calls = 0;
static pthread_mutex_t scoop_rt_init_lock = PTHREAD_MUTEX_INITIALIZER;

// GC stress（测试/回归用）：通过 env `SCOOP_GC_STRESS` 触发额外 GC。
//
// 约定：
// - 未设置：关闭（0）；
// - 设为数字 N（N>=1）：每 N 次分配前触发一次 `scoop_gc_collect()`；
// - 其它非空字符串：视为开启（等价于 1）；
// - 特判：`0`/`false`/`no` 视为关闭。
//
// 说明：
// - 该开关只影响 `scoop_alloc` 分配路径；默认关闭，避免影响正常性能。
// - GC 触发点选择为“分配前”：避免在对象尚未被 caller 放入 roots 之前被 GC 误回收。
static uint64_t scoop_rt_gc_stress_interval = 0;
static _Atomic(uint64_t) scoop_rt_gc_stress_alloc_counter = 0;

static uint64_t scoop_rt_parse_gc_stress_interval(void) {
  const char *raw = getenv("SCOOP_GC_STRESS");
  if (raw == 0) {
    return 0;
  }

  // 跳过前导空白（允许 `SCOOP_GC_STRESS=" 1"`）。
  while (raw[0] == ' ' || raw[0] == '\t' || raw[0] == '\n' || raw[0] == '\r') {
    raw++;
  }

  if (raw[0] == 0) {
    return 1;
  }
  if (strcmp(raw, "0") == 0 || strcmp(raw, "false") == 0 || strcmp(raw, "no") == 0) {
    return 0;
  }

  char *end = 0;
  unsigned long long v = strtoull(raw, &end, 10);
  if (end != 0 && end != raw && end[0] == 0) {
    if (v == 0) {
      return 0;
    }
    return (uint64_t)v;
  }

  // 非数字：只要不是显式 false/0/no，就视为“开启（每次分配触发）”。
  return 1;
}

// GC heap（v0：数据结构骨架）。
//
// 说明：
// - heap 的定义位于 `runtime/c/scoop_gc.c`（保持 GC 逻辑集中）；本文件仅引用它；
// - `scoop_runtime_init()` 会初始化 heap；`scoop_alloc` 会把对象登记到 heap 链表；
// - 手动触发 GC：`scoop_gc_collect()`（TODO T0910）。
extern ScoopGcHeap scoop_gc_heap;

uint32_t scoop_runtime_is_initialized(void) {
  return scoop_rt_initialized;
}

uint32_t scoop_runtime_init_count(void) {
  return scoop_rt_init_calls;
}

// 每线程 TLS 状态：
// - 定义位于 `runtime/c/scoop_tls_internal.h`（internal，便于 GC 后端共享布局）。
// - 本文件负责声明 thread-local 实例与读写 API。

static SCOOP_THREAD_LOCAL ScoopThreadTls scoop_tls = {0};

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
// T1409b：thread-local block cache（批量取还 / per-thread cache）。
//
// 说明：
// - cache 以 `ScoopGcImmixBlock.next_free` 串成单链表；
// - 所有读写仅发生在“当前线程”，因此不需要额外同步原语；
// - refill 时会短暂持有全局 GC 锁，并批量从全局 pool 取 blocks 放入 cache，以减少锁进入频率。
#ifndef SCOOP_GC_IMMIX_BLOCK_CACHE_BATCH
#define SCOOP_GC_IMMIX_BLOCK_CACHE_BATCH 8u
#endif

static inline ScoopGcImmixBlock *scoop_gc_immix_tls_cache_pop(void) {
  ScoopGcImmixBlock *head = (ScoopGcImmixBlock *)scoop_tls.gc_immix_block_cache;
  if (head == 0) {
    return 0;
  }
  ScoopGcImmixBlock *next = head->next_free;
  head->next_free = 0;
  scoop_tls.gc_immix_block_cache = (void *)next;
  if (scoop_tls.gc_immix_block_cache_len > 0) {
    scoop_tls.gc_immix_block_cache_len -= 1;
  }
  return head;
}

static inline void scoop_gc_immix_tls_cache_push(ScoopGcImmixBlock *block) {
  if (block == 0) {
    return;
  }
  block->next_free = (ScoopGcImmixBlock *)scoop_tls.gc_immix_block_cache;
  scoop_tls.gc_immix_block_cache = (void *)block;
  if (scoop_tls.gc_immix_block_cache_len != UINT32_MAX) {
    scoop_tls.gc_immix_block_cache_len += 1;
  }
}

// 注意：调用方必须持有 `state->lock`。
static inline void scoop_gc_immix_tls_cache_refill_locked(ScoopGcImmixState *state) {
  if (state == 0) {
    return;
  }
  if (scoop_tls.gc_immix_block_cache_len != 0) {
    return;
  }

  for (uint32_t i = 0; i < (uint32_t)SCOOP_GC_IMMIX_BLOCK_CACHE_BATCH; i++) {
    ScoopGcImmixBlock *b = scoop_gc_immix_state_take_block(state);
    if (b == 0) {
      break;
    }
    scoop_gc_immix_tls_cache_push(b);
  }
}

// T1412b：Immix nursery（bump-only）。
//
// 设计目标（v0）：
// - nursery 的分配必须“成本可上界”：只做 bump，不做 holes 搜索/复用；
// - nursery 的工作量边界由 `nursery_max_blocks` 控制（通过 env 配置；见 immix heap init）；
// - 当 nursery 用尽时，分配路径回退到 old allocator（现有 hole-bump + reusable blocks 复用）。
//
// 注意：
// - 该实现仅提供分配区与上限；minor evacuation 语义在 TODO T1412c 落地。
// - 调用方必须持有 `state->lock`（便于与 GC 周期/blocks 列表维护保持一致）。
static inline ScoopGcImmixBlock *scoop_gc_immix_nursery_take_block_locked(ScoopGcImmixState *state) {
  if (state == 0) {
    return 0;
  }
  if (state->nursery_max_blocks == 0) {
    return 0;
  }

  ScoopGcImmixBlock *block = 0;
  if (state->nursery_free_blocks != 0) {
    block = state->nursery_free_blocks;
    state->nursery_free_blocks = block->next_free;
    block->next_free = 0;
  } else {
    if (state->nursery_blocks >= state->nursery_max_blocks) {
      return 0;
    }

    block = scoop_gc_immix_block_alloc_new();
    if (block == 0) {
      return 0;
    }
    block->generation = (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY;

    // nursery blocks 仍是“真实 Immix blocks”：挂入 all_blocks，便于 major GC 遍历与统计。
    block->next_all = state->all_blocks;
    state->all_blocks = block;

    if (state->nursery_blocks != UINT32_MAX) {
      state->nursery_blocks += 1;
    }
  }

  state->nursery_current_block = block;
  return block;
}

static inline void *scoop_gc_immix_nursery_alloc_locked(ScoopGcImmixState *state,
                                                        size_t size,
                                                        size_t alignment) {
  if (state == 0 || size == 0) {
    return 0;
  }
  if (state->nursery_max_blocks == 0) {
    return 0;
  }
  if (alignment == 0) {
    alignment = 1;
  }

  ScoopGcImmixBlock *block = state->nursery_current_block;
  for (uint32_t tries = 0; tries < 128; tries++) {
    if (block == 0) {
      block = scoop_gc_immix_nursery_take_block_locked(state);
    }
    if (block == 0) {
      return 0;
    }

    void *p = scoop_gc_immix_block_alloc_bump(block, size, alignment);
    if (p != 0) {
      return p;
    }

    // 当前 nursery block 放不下：切换到下一个 block（bump-only，不回退到 holes）。
    state->nursery_current_block = 0;
    block = 0;
  }

  return 0;
}
#endif

// --- effect runtime v0（TODO T0906） ---
//
// 说明：
// - 本阶段只提供 flag + 单个 perform slot 的 TLS 骨架；不实现 dispatch；
// - codegen/lowering 会在后续任务（T0613+）接入对这些 TLS 符号的读写；
// - 这些符号名用于仓库内部实现/测试，并不承诺稳定 ABI（见 spec 备注）。
// perform slot：flag-based unwinding 的“effect 载荷寄存器”（每线程）。
//
// 设计目标（TODO T0630）：
// - 能承载多 word payload（结构体风格：按字段顺序写入 words）
// - 允许 union/variant 风格：由 lowering 在 words[0] 写入判别信息（例如 enum tag / kind）
// - ABI 在同一 target 上稳定（offset/size 固化，便于 LLVM codegen 假设与测试）
//
// 说明：
// - 当前阶段把 payload 统一表示为若干个 `u64` word；更复杂的布局/对齐规则留给后续任务扩展。
// - `payload_len_words` 表示有效 word 数量；当 slot 被 clear 时，它必须为 0。
#define SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS 8u

typedef struct ScoopEffectPerformSlot {
  // operation tag（由 lowering 写入；当前阶段用于区分不同 effect op）。
  uint32_t op_tag;

  // payload 的有效 word 数（0..=SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS）。
  uint32_t payload_len_words;

  // payload words：低层 ABI 以 “word 序列” 形式传递复合数据。
  uint64_t payload_words[SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS];
} ScoopEffectPerformSlot;

// non-resuming effect（flag-based unwinding）的“最小诊断信息”TLS。
//
// 说明（TODO T1411c）：
// - 该结构用于在 `perform/raise` 发生时记录一个稳定的 call-site（line/col），并可选采样 backtrace；
// - 该数据结构是“诊断/调试辅助”，不参与语义判定：effect 的语义仍由 active flag + perform slot 决定；
// - 为保证 fixtures 的可断言性：
//   - 默认不采样 backtrace（避免地址不稳定）；仅记录 line/col；
//   - 若设置环境变量 `SCOOP_EFFECT_CAPTURE_UNWIND=1`，则额外采样当前线程的 instruction pointers。
// - 该 TLS 不通过 `scoop_effect_clear()` 清空，便于 handler/catch body 在清 flag/slot 后仍可读取。
#define SCOOP_EFFECT_TRACE_MAX_IPS 8u

typedef struct ScoopEffectTraceV0 {
  uint32_t version;
  uint32_t src_line;
  uint32_t src_col;
  uint32_t unwind_len;
  uintptr_t unwind_ips[SCOOP_EFFECT_TRACE_MAX_IPS];
} ScoopEffectTraceV0;

static SCOOP_THREAD_LOCAL ScoopEffectTraceV0 scoop_effect_trace = {0};

// ABI 断言：固定 perform slot 的布局，以便 codegen 与跨 crate 测试可以依赖。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(
    offsetof(ScoopEffectPerformSlot, op_tag) == 0,
    "ScoopEffectPerformSlot.op_tag offset must be 0");
_Static_assert(
    offsetof(ScoopEffectPerformSlot, payload_len_words) == 4,
    "ScoopEffectPerformSlot.payload_len_words offset must be 4");
_Static_assert(
    offsetof(ScoopEffectPerformSlot, payload_words) == 8,
    "ScoopEffectPerformSlot.payload_words offset must be 8");
_Static_assert(
    sizeof(ScoopEffectPerformSlot) ==
        (8u + 8u * SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS),
    "ScoopEffectPerformSlot size must be 8 + 8*MAX_WORDS bytes");
#endif

// --- effect runtime：handler stack（TODO T0913 / Appendix A） ---
//
// 说明：
// - handler stack 用于表达“当前计算”的动态 effect 上下文（Appendix A：最近匹配 handler 分发）；
// - v0 只实现：
//   - TLS 栈：push/pop；
//   - 最近匹配查询（按 op_tag 精确匹配）；
//   - active 开关：用于实现“arm body 在自身 handler 的 dispatch scope 之外执行”（Appendix A.4）。
// - handler arm 的实际执行仍由编译器 lowering/codegen 生成；runtime 只维护动态上下文。
//
// 关键语义（Appendix A.4）：
// - 进入 arm body 期间，触发该 arm 的 handler instance 必须被视为 inactive；
// - arm body 内再次 perform 同一 op，应命中外层 handler（若存在），而不是自捕获。
typedef struct ScoopEffectHandlerFrame {
  struct ScoopEffectHandlerFrame *prev;
  uint32_t op_tag;
  uint32_t active;
} ScoopEffectHandlerFrame;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(
    offsetof(ScoopEffectHandlerFrame, prev) == 0,
    "ScoopEffectHandlerFrame.prev offset must be 0");
_Static_assert(
    offsetof(ScoopEffectHandlerFrame, op_tag) == 8,
    "ScoopEffectHandlerFrame.op_tag offset must be 8");
_Static_assert(
    offsetof(ScoopEffectHandlerFrame, active) == 12,
    "ScoopEffectHandlerFrame.active offset must be 12");
_Static_assert(sizeof(ScoopEffectHandlerFrame) == 16, "ScoopEffectHandlerFrame size must be 16");
#endif

// handler stack：每线程栈顶指针。
SCOOP_THREAD_LOCAL ScoopEffectHandlerFrame *__scoop_effect_handler_stack_top = 0;

// flag-based unwinding：每线程 active flag（0=inactive，1=active）。
SCOOP_THREAD_LOCAL uint32_t __scoop_effect_active = 0;

// flag-based unwinding：每线程 perform slot（后续由 `perform` 写入）。
SCOOP_THREAD_LOCAL ScoopEffectPerformSlot __scoop_effect_perform_slot = {0};

static inline void scoop_effect_trace_reset(uint32_t src_line, uint32_t src_col) {
  scoop_effect_trace.version = 0;
  scoop_effect_trace.src_line = src_line;
  scoop_effect_trace.src_col = src_col;
  scoop_effect_trace.unwind_len = 0;
  for (uint32_t i = 0; i < SCOOP_EFFECT_TRACE_MAX_IPS; i++) {
    scoop_effect_trace.unwind_ips[i] = 0;
  }
}

static inline uint32_t scoop_effect_trace_should_capture_unwind(void) {
  const char *value = getenv("SCOOP_EFFECT_CAPTURE_UNWIND");
  if (value == 0) {
    return 0;
  }
  if (value[0] == 0) {
    return 0;
  }
  if (value[0] == '0' && value[1] == 0) {
    return 0;
  }
  return 1;
}

static inline void scoop_effect_trace_on_set_active(uint32_t src_line, uint32_t src_col) {
  scoop_effect_trace_reset(src_line, src_col);
  if (!scoop_effect_trace_should_capture_unwind()) {
    return;
  }

  // 跳过最顶端的一帧（通常是 set_active wrapper 自身），避免把 runtime wrapper 暴露到诊断里。
  const uint32_t skip_frames = 1;
  uint32_t n = scoop_platform_unwind_capture_ips(
      scoop_effect_trace.unwind_ips, SCOOP_EFFECT_TRACE_MAX_IPS, skip_frames);
  scoop_effect_trace.unwind_len = n;
}

uint32_t scoop_thread_is_registered(void) {
  return scoop_tls.registered;
}

// `scoop_runtime_init` 定义在文件后部；这里给出前置声明以避免隐式声明警告。
void scoop_runtime_init(void);

// 线程注册接口（占位）。
//
// 说明：
// - 未来在引入 stop-the-world GC 后，新线程必须注册/注销，以便被枚举并扫描 roots；
// - 当前阶段只做“TLS 标记 + 可重复调用”，不维护全局线程列表。
void scoop_thread_register(void) {
  // 若 runtime 尚未 init，则允许先 init（保持接口易用性；init 目前是幂等的）。
  if (!scoop_rt_initialized) {
    scoop_runtime_init();
  }

  if (scoop_tls.registered) {
    return;
  }

  scoop_tls.registered = 1;

  // 把当前线程纳入 GC stop-the-world 线程表（TODO T0911）。
  void scoop_gc_thread_register(ScoopThreadTls *tls);
  scoop_gc_thread_register(&scoop_tls);
}

void scoop_thread_unregister(void) {
  if (!scoop_tls.registered) {
    return;
  }

  // 从 GC stop-the-world 线程表注销（TODO T0911）。
  void scoop_gc_thread_unregister(ScoopThreadTls *tls);
  scoop_gc_thread_unregister(&scoop_tls);

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
  // T1409a/T1409b：线程退出前归还 thread-local blocks（current block + cache），避免：
  // - thread-local cache 长期“藏住 blocks”，导致其它线程不得不分配新 blocks；
  // - 线程频繁创建/销毁时 reserved bytes 出现不必要增长。
  ScoopGcImmixState *state = scoop_gc_immix_state_from_heap(&scoop_gc_heap);
  if (state != 0 && state->lock_inited) {
    (void)pthread_mutex_lock(&state->lock);

    // 1) current block
    if (scoop_tls.gc_immix_current_block != 0) {
      ScoopGcImmixBlock *block = (ScoopGcImmixBlock *)scoop_tls.gc_immix_current_block;
      scoop_tls.gc_immix_current_block = 0;
      if (block != 0) {
        if (block->live_objects == 0) {
          block->next_free = state->free_blocks;
          state->free_blocks = block;
        } else {
          block->next_free = state->reusable_blocks;
          state->reusable_blocks = block;
        }
      }
    }

    // 2) cached blocks（以 next_free 串起来）
    ScoopGcImmixBlock *b = (ScoopGcImmixBlock *)scoop_tls.gc_immix_block_cache;
    scoop_tls.gc_immix_block_cache = 0;
    scoop_tls.gc_immix_block_cache_len = 0;

    while (b != 0) {
      ScoopGcImmixBlock *next = b->next_free;
      b->next_free = 0;

      if (b->live_objects == 0) {
        b->next_free = state->free_blocks;
        state->free_blocks = b;
      } else {
        b->next_free = state->reusable_blocks;
        state->reusable_blocks = b;
      }

      b = next;
    }

    (void)pthread_mutex_unlock(&state->lock);
  } else {
    // 保守：无法拿到 state lock 时仍应清空 TLS，避免泄漏悬挂指针。
    scoop_tls.gc_immix_current_block = 0;
    scoop_tls.gc_immix_block_cache = 0;
    scoop_tls.gc_immix_block_cache_len = 0;
  }
#endif

  // 早期阶段：注销时清空 TLS，避免后续测试/手动调试场景出现悬挂状态。
  scoop_tls.registered = 0;
  scoop_tls.gc_immix_current_block = 0;
  scoop_tls.gc_immix_block_cache = 0;
  scoop_tls.gc_immix_block_cache_len = 0;
  scoop_tls._reserved_u32_1 = 0;
  scoop_tls.gc_native_roots = 0;
  scoop_tls.gc_native_roots_len = 0;
  scoop_tls._reserved_u32_2 = 0;
  scoop_tls._reserved0 = 0;
  scoop_tls._reserved1 = 0;
  scoop_tls._reserved2 = 0;

  // effect runtime：清空 flag/slot（TODO T0906）。
  __scoop_effect_active = 0;
  __scoop_effect_handler_stack_top = 0;
  (void)memset(&__scoop_effect_perform_slot, 0, sizeof(__scoop_effect_perform_slot));
  scoop_effect_trace_reset(0, 0);
}

// effect runtime（TODO T0906）：set/clear API（仅用于最小回归与后续 lowering 接入）。
uint32_t scoop_effect_is_active(void) {
  return __scoop_effect_active;
}

void scoop_effect_set_active(void) {
  __scoop_effect_active = 1;
  scoop_effect_trace_on_set_active(0, 0);
}

// effect runtime（TODO T1411c）：set active + 记录最小诊断信息（call-site line/col）。
void scoop_effect_set_active_with_trace(uint32_t src_line, uint32_t src_col) {
  __scoop_effect_active = 1;
  scoop_effect_trace_on_set_active(src_line, src_col);
}

void scoop_effect_clear(void) {
  __scoop_effect_active = 0;
  (void)memset(&__scoop_effect_perform_slot, 0, sizeof(__scoop_effect_perform_slot));
}

// effect runtime（TODO T1411c）：读取最近一次 non-resuming effect 的诊断信息。
//
// 说明：
// - 这些 getter 只返回稳定字段（line/col/unwind_len）；
// - `unwind_ips` 目前不导出为稳定 ABI（避免把地址暴露到 fixtures 输出中）。
uintptr_t scoop_effect_trace_src_line(void) {
  return (uintptr_t)scoop_effect_trace.src_line;
}

uintptr_t scoop_effect_trace_src_col(void) {
  return (uintptr_t)scoop_effect_trace.src_col;
}

uintptr_t scoop_effect_trace_unwind_len(void) {
  return (uintptr_t)scoop_effect_trace.unwind_len;
}

// effect runtime（TODO T0613/T0630）：perform slot 读写 API（稳定 ABI）。
//
// 说明：
// - `op_tag` 用于区分 operation（后续任务会定义稳定的 tag 分配规则）。
// - 当前阶段不做任何 dispatch/unwind；仅提供 TLS slot 的读写，以便后续 lowering 接入并可回归验证。
void scoop_effect_perform_slot_write_u64(uint32_t op_tag, uint64_t value) {
  __scoop_effect_perform_slot.op_tag = op_tag;
  __scoop_effect_perform_slot.payload_len_words = 1;
  __scoop_effect_perform_slot.payload_words[0] = value;
  // 清理剩余 words，避免测试/调试读取到“上一次”的脏数据。
  for (uint32_t i = 1; i < SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS; i++) {
    __scoop_effect_perform_slot.payload_words[i] = 0;
  }
}

void scoop_effect_perform_slot_write_u64_2(uint32_t op_tag,
                                          uint64_t word0,
                                          uint64_t word1) {
  __scoop_effect_perform_slot.op_tag = op_tag;
  __scoop_effect_perform_slot.payload_len_words = 2;
  __scoop_effect_perform_slot.payload_words[0] = word0;
  __scoop_effect_perform_slot.payload_words[1] = word1;
  for (uint32_t i = 2; i < SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS; i++) {
    __scoop_effect_perform_slot.payload_words[i] = 0;
  }
}

uint32_t scoop_effect_perform_slot_read_op_tag(void) {
  return __scoop_effect_perform_slot.op_tag;
}

uint32_t scoop_effect_perform_slot_read_len_words(void) {
  return __scoop_effect_perform_slot.payload_len_words;
}

uint64_t scoop_effect_perform_slot_read_u64(void) {
  // 兼容 API：读第 0 个 word（单 word payload 的最常见场景）。
  if (__scoop_effect_perform_slot.payload_len_words == 0) {
    return 0;
  }
  return __scoop_effect_perform_slot.payload_words[0];
}

uint64_t scoop_effect_perform_slot_read_u64_at(uint32_t index) {
  // 早期阶段选择“越界返回 0”而不是崩溃，避免错误传播路径里引入额外不确定性。
  if (index >= SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS) {
    return 0;
  }
  if (index >= __scoop_effect_perform_slot.payload_len_words) {
    return 0;
  }
  return __scoop_effect_perform_slot.payload_words[index];
}

// --- effect runtime：handler stack API（TODO T0913） ---
//
// 说明：
// - `frame` 预期由编译器在栈上分配，并保证 push/pop 成对；
// - 若 push/pop 不匹配，说明 lowering/codegen 出现 bug：按运行期错误处理（exit(3)）。
void scoop_effect_handler_stack_push(ScoopEffectHandlerFrame *frame, uint32_t op_tag) {
  if (frame == 0) {
    return;
  }

  // 保持与 GC/effect 其它 API 一致：允许在未显式 init/register 的情况下被调用。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  frame->prev = __scoop_effect_handler_stack_top;
  frame->op_tag = op_tag;
  frame->active = 1;
  __scoop_effect_handler_stack_top = frame;
}

void scoop_effect_handler_stack_pop(ScoopEffectHandlerFrame *frame) {
  if (frame == 0) {
    return;
  }

  if (__scoop_effect_handler_stack_top != frame) {
    exit(3);
  }

  __scoop_effect_handler_stack_top = frame->prev;
  frame->prev = 0;
  frame->active = 0;
}

void scoop_effect_handler_stack_set_active(ScoopEffectHandlerFrame *frame, uint32_t active) {
  if (frame == 0) {
    return;
  }
  frame->active = active ? 1u : 0u;
}

ScoopEffectHandlerFrame *scoop_effect_handler_stack_top(void) {
  return __scoop_effect_handler_stack_top;
}

// 切换当前线程的 handler stack 栈顶指针，并返回旧值。
//
// 说明：
// - 该 API 主要用于 continuation 跨线程 `resume`：在进入 continuation 时安装 captured handler stack，
//   在返回后恢复调用方原有的动态上下文（spec §5.5）。
ScoopEffectHandlerFrame *scoop_effect_handler_stack_swap_top(ScoopEffectHandlerFrame *new_top) {
  // 保持与其它 runtime API 一致：允许在未显式 init/register 的情况下被调用。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  ScoopEffectHandlerFrame *old_top = __scoop_effect_handler_stack_top;
  __scoop_effect_handler_stack_top = new_top;
  return old_top;
}

ScoopEffectHandlerFrame *scoop_effect_handler_stack_find_nearest(uint32_t op_tag) {
  ScoopEffectHandlerFrame *it = __scoop_effect_handler_stack_top;
  while (it != 0) {
    if (it->active && it->op_tag == op_tag) {
      return it;
    }
    it = it->prev;
  }
  return 0;
}

// T1608: Pop non-matching intermediate handler frames until the matching op_tag
// is on top (or the stack is empty).
//
// Used when an effect propagates through nested handlers of different effects:
// the compile-time dispatch can skip non-matching handlers, but their runtime
// frames need to be cleaned up before the matching handler's catch block runs.
// The matching frame is NOT popped — the catch block will do that.
void scoop_effect_handler_stack_unwind_to_tag(uint32_t op_tag) {
  while (__scoop_effect_handler_stack_top != 0) {
    ScoopEffectHandlerFrame *top = __scoop_effect_handler_stack_top;
    if (top->op_tag == op_tag) {
      break;
    }
    __scoop_effect_handler_stack_top = top->prev;
    top->prev = 0;
    top->active = 0;
  }
}

// --- Continuation（spec §5.5 / TODO T0914） ---
//
// 说明：
// - `Continuation<T>` 是 escape continuation（`, k ->`）在运行期的堆对象表示；
// - 该对象需要捕获：
//   - suspension 点的 handler stack（Appendix A：fiber-local 语义）
//   - heap state machine 的状态指针（由编译器生成并用 GC 管理）
// - one-shot 约束：同一个 continuation 只能成功 resume 一次（第二次是运行期错误）。
//
// 当前阶段（T0914）先只固定 ABI 布局并提供原子 one-shot 检查 API；
// handler stack 的跨线程安装与恢复由 `scoop_continuation_resume_u64`（TODO T0915a）提供。
typedef void (*ScoopContinuationStepFn)(void *state, uint64_t resume_value);

typedef struct ScoopContinuation {
  // 作为 GC-managed 对象，必须以对象头开头（与 `scoop_alloc` 约定一致）。
  ScoopGcObjectHeader hdr;

  // 0=未 resume；1=已 resume（one-shot）。使用原子状态位为未来并发 resume 做准备。
  _Atomic uint32_t resumed;

  // 保留字段：用于对齐/未来扩展（例如更细的状态机标志）。
  uint32_t _reserved_u32;

  // 捕获的 handler stack（suspension 点的 TLS 栈顶指针；Appendix A）。
  ScoopEffectHandlerFrame *captured_handler_stack_top;

  // heap state machine 指针（由编译器生成；应当是 GC-managed heap 对象）。
  void *state;

  // step 函数（由编译器生成的 trampoline），用于推进 state machine。
  ScoopContinuationStepFn step_fn;
} ScoopContinuation;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopContinuation, hdr) == 0,
               "ScoopContinuation.hdr offset must be 0");
_Static_assert(offsetof(ScoopContinuation, resumed) == sizeof(ScoopGcObjectHeader),
               "ScoopContinuation.resumed offset must be sizeof(ScoopGcObjectHeader)");
_Static_assert(offsetof(ScoopContinuation, captured_handler_stack_top) ==
                   (sizeof(ScoopGcObjectHeader) + 8u),
               "ScoopContinuation.captured_handler_stack_top offset must be header + 8");
_Static_assert((sizeof(ScoopContinuation) % sizeof(void *)) == 0,
               "ScoopContinuation size must be pointer-aligned");
#endif

static uint64_t scoop_continuation_trace(void *object, ScoopGcTraceVisitor visitor, void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopContinuation *k = (ScoopContinuation *)object;
  if (k->state == 0) {
    return 0;
  }

  // `state` 预期指向一个 GC-managed heap 对象；把该槽位暴露给 visitor 以便 mark 更新/追踪。
  void **slot = (void **)&k->state;
  visitor(slot, ctx);
  return 1;
}

static const ScoopTypeDescriptor SCOOP_CONTINUATION_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopContinuation),
    .align_bytes = (uint64_t)_Alignof(ScoopContinuation),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_continuation_trace,
    .release_fn = 0,
};

// `scoop_alloc` 在文件后部定义；这里提供前置声明以避免隐式声明警告/错误。
void *scoop_alloc(uint64_t size);

void *scoop_continuation_alloc(void *state, ScoopContinuationStepFn step_fn) {
  // 约定：为保持 API 易用，允许在未显式 init/register 的情况下被调用。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  // GC 安全性（T1606c）：
  // `state` 是 GC heap 上的对象（ContState），但本函数接收的是 raw void*——
  // 如果 scoop_alloc 触发 GC 并搬迁了 state，本函数持有的局部变量 `state` 不会被更新
  // （C 函数没有 statepoint 信息），导致下面 `k->state = state` 存入悬空指针。
  // 通过在分配前 pin 住 state，阻止 GC 搬迁它。
  if (state != 0) {
    scoop_pin(state);
  }

  ScoopContinuation *k = (ScoopContinuation *)scoop_alloc((uint64_t)sizeof(ScoopContinuation));
  if (k == 0) {
    if (state != 0) {
      scoop_unpin(state);
    }
    return 0;
  }

  // `scoop_alloc` 已初始化对象头（size/mark 等）；这里补齐 continuation 专属字段。
  k->hdr.type_desc = &SCOOP_CONTINUATION_TYPE_DESC;

  atomic_init(&k->resumed, 0);
  k->_reserved_u32 = 0;
  k->captured_handler_stack_top = __scoop_effect_handler_stack_top;
  k->state = state;
  k->step_fn = step_fn;

  if (state != 0) {
    scoop_unpin(state);
  }

  return (void *)k;
}

uint32_t scoop_continuation_try_resume(void *continuation) {
  if (continuation == 0) {
    return 0;
  }

  ScoopContinuation *k = (ScoopContinuation *)continuation;
  uint32_t expected = 0;
  if (atomic_compare_exchange_strong_explicit(
          &k->resumed,
          &expected,
          1u,
          memory_order_acq_rel,
          memory_order_acquire)) {
    return 1;
  }
  return 0;
}

// 执行 continuation 的一步推进（由编译器生成的 step_fn 实现状态机推进）。
//
// 语义（spec §5.5）：
// - one-shot：同一个 continuation 只能成功 resume 一次；第二次为运行期错误（exit(3)）。
// - fiber-local：resume 时需要恢复其捕获的 handler stack（Appendix A），允许在另一线程执行；
//   并在 step_fn 返回后恢复调用方原 TLS handler stack。
//
// 当前阶段（T0915a）只切换 handler stack；perform slot/flag 等其它 TLS 状态仍由 lowering 约束其使用。
void scoop_continuation_resume_u64(void *continuation, uint64_t resume_value) {
  if (continuation == 0) {
    return;
  }

  // 允许在未显式 init/register 的情况下被调用（与其它 API 保持一致）。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  if (!scoop_continuation_try_resume(continuation)) {
    // spec §5.5 / §5.7：one-shot 违规应当表现为可捕获的运行时错误：
    // `Raise.raise(RuntimeError.ContinuationAlreadyResumed)`（而不是进程级 exit/abort）。
    //
    // 说明：
    // - runtime 侧复用既有的 Raise flag-based unwinding 机制：写入 perform slot + set flag；
    // - 由 codegen 在 call-site 检查 flag 并跳转到最近的 try/catch/handle 边界；
    // - 这里需要写入 `RuntimeError` 的 tag 值：当前按 sysroot `RuntimeError` 的声明顺序编码：
    //   - 0: NullAssertionFailed
    //   - 1: ClassCastFailed
    //   - 2: ContinuationAlreadyResumed
    const uint32_t OP_TAG_RAISE = 1u;
    const uint64_t PAYLOAD_KIND_RUNTIME_ERROR = 2u;
    const uint64_t RUNTIME_ERROR_TAG_CONTINUATION_ALREADY_RESUMED = 2u;
    scoop_effect_perform_slot_write_u64_2(
        OP_TAG_RAISE,
        PAYLOAD_KIND_RUNTIME_ERROR,
        RUNTIME_ERROR_TAG_CONTINUATION_ALREADY_RESUMED);
    scoop_effect_set_active();
    return;
  }

  ScoopContinuation *k = (ScoopContinuation *)continuation;
  ScoopEffectHandlerFrame *saved =
      scoop_effect_handler_stack_swap_top(k->captured_handler_stack_top);

  if (k->step_fn != 0) {
    k->step_fn(k->state, resume_value);
  }

  (void)scoop_effect_handler_stack_swap_top(saved);
}

// --- Continuation 跨线程 resume（spec §5.5 / TODO T0618） ---
//
// 说明：
// - 该 helper 用于把“跨线程 resume”能力暴露给 end-to-end fixtures（不引入调度器）。
// - 语义：在一个新线程中调用 `scoop_continuation_resume_u64`，并 join 等待其完成。
// - 线程会在执行结束后调用 `scoop_thread_unregister`，避免 STW 线程表残留已退出线程的 TLS 槽位。
typedef struct ScoopThreadResumeU64Args {
  void *continuation;
  uint64_t resume_value;
} ScoopThreadResumeU64Args;

static void *scoop_thread_entry_resume_u64(void *arg) {
  if (arg == 0) {
    return 0;
  }

  ScoopThreadResumeU64Args *args = (ScoopThreadResumeU64Args *)arg;
  scoop_continuation_resume_u64(args->continuation, args->resume_value);

  // 清理线程注册信息：避免 GC 的线程枚举残留无效条目。
  scoop_thread_unregister();
  free(args);

  return 0;
}

void scoop_thread_spawn_join_resume_u64(void *continuation, uint64_t resume_value) {
  // 保持与其它 runtime API 一致：允许在未显式 init/register 的情况下被调用。
  if (!scoop_rt_initialized) {
    scoop_runtime_init();
  }

  ScoopThreadResumeU64Args *args =
      (ScoopThreadResumeU64Args *)malloc(sizeof(ScoopThreadResumeU64Args));
  if (args == 0) {
    exit(3);
  }
  args->continuation = continuation;
  args->resume_value = resume_value;

  pthread_t t;
  int rc = pthread_create(&t, 0, scoop_thread_entry_resume_u64, (void *)args);
  if (rc != 0) {
    free(args);
    exit(3);
  }

  rc = pthread_join(t, 0);
  if (rc != 0) {
    exit(3);
  }
}

// --- Tasks / spawn/join（early stage, TODO T0620） ---
//
// 说明：
// - 当前阶段只提供最小可执行语义：`spawn { body }` 会先在当前线程计算出 `Int` 值，
//   再由 runtime 分配一个句柄对象保存该值；`join handle` 取回该值；
// - 该实现不提供真实并行/取消；更完整的 `Task<T>`/executor 原语见 `runtime/c/scoop_task_executor.c`（T0917）。
// - `join` 当前为 one-shot：对同一个 handle 重复 join 会直接 `exit(3)`（与 `resume` 的运行期断言保持一致）。

typedef struct ScoopTaskInt {
  uint32_t joined;
  uint32_t _reserved_u32;
  int64_t value;
} ScoopTaskInt;

uint64_t scoop_task_spawn_int(int64_t value) {
  if (!scoop_rt_initialized) {
    scoop_runtime_init();
  }

  ScoopTaskInt *task = (ScoopTaskInt *)malloc(sizeof(ScoopTaskInt));
  if (task == 0) {
    // OOM：返回 0 句柄（join 时会返回 0）。
    return 0;
  }

  task->joined = 0;
  task->_reserved_u32 = 0;
  task->value = value;
  return (uint64_t)(uintptr_t)task;
}

int64_t scoop_task_join_int(uint64_t handle) {
  if (handle == 0) {
    return 0;
  }

  ScoopTaskInt *task = (ScoopTaskInt *)(uintptr_t)handle;
  if (task->joined) {
    exit(3);
  }

  task->joined = 1;
  return task->value;
}

static int scoop_is_indent_ws(uint8_t c) {
  // Kotlin 风格：缩进只考虑空格/Tab（raw string 的常见场景）。
  return c == (uint8_t)' ' || c == (uint8_t)'\t';
}

static int scoop_is_blank_ws(uint8_t c) {
  // “空白行”判断：把 CR 也视为可忽略空白，以兼容 CRLF 输入。
  return c == (uint8_t)' ' || c == (uint8_t)'\t' || c == (uint8_t)'\r';
}

// `trimIndent()`：去掉所有行的公共缩进，并剥离首尾空白行（spec §8.4）。
//
// 约定（early stage）：
// - 输入/输出字符串都以 `ScoopString { len, data }` 表示（UTF-8 bytes）；
// - 输出的 `ScoopString` 对象通过 `scoop_alloc` 分配（GC-managed）；`data` buffer 仍由 `malloc` 分配；
// - 当前实现仅识别 ASCII 空格/Tab 作为缩进；其它 Unicode 空白暂不处理。
const ScoopString *scoop_string_trim_indent(const ScoopString *value) {
  if (value == 0) {
    return 0;
  }
  if (value->len == 0 || value->data == 0) {
    return value;
  }

  const uint8_t *data = value->data;
  uint64_t len = value->len;

  // 1) 先统计行数（按 '\n' 分割）。
  uint64_t line_count = 1;
  for (uint64_t i = 0; i < len; i++) {
    if (data[i] == (uint8_t)'\n') {
      line_count++;
    }
  }

  // 2) 记录每一行的 [start, end)（end 不含 '\n'；若行尾是 '\r' 则剥离）。
  uint64_t *starts = (uint64_t *)malloc(sizeof(uint64_t) * (size_t)line_count);
  uint64_t *ends = (uint64_t *)malloc(sizeof(uint64_t) * (size_t)line_count);
  if (starts == 0 || ends == 0) {
    // OOM：保守回退为原串（避免崩溃）。
    if (starts != 0) {
      free(starts);
    }
    if (ends != 0) {
      free(ends);
    }
    return value;
  }

  uint64_t line_idx = 0;
  uint64_t cur_start = 0;
  for (uint64_t i = 0; i <= len; i++) {
    if (i == len || data[i] == (uint8_t)'\n') {
      uint64_t end = i;
      if (end > cur_start && data[end - 1] == (uint8_t)'\r') {
        end--;
      }
      starts[line_idx] = cur_start;
      ends[line_idx] = end;
      line_idx++;
      cur_start = i + 1;
    }
  }

  // 健壮性：理论上 line_idx == line_count。
  if (line_idx != line_count) {
    free(starts);
    free(ends);
    return value;
  }

  // 3) 剥离首尾空白行。
  uint64_t first = 0;
  while (first < line_count) {
    uint64_t s = starts[first];
    uint64_t e = ends[first];
    int blank = 1;
    for (uint64_t i = s; i < e; i++) {
      if (!scoop_is_blank_ws(data[i])) {
        blank = 0;
        break;
      }
    }
    if (!blank) {
      break;
    }
    first++;
  }

  if (first == line_count) {
    // 全部是空白行：返回空串。
    free(starts);
    free(ends);
    return scoop_string_empty();
  }

  uint64_t last = line_count - 1;
  while (last > first) {
    uint64_t s = starts[last];
    uint64_t e = ends[last];
    int blank = 1;
    for (uint64_t i = s; i < e; i++) {
      if (!scoop_is_blank_ws(data[i])) {
        blank = 0;
        break;
      }
    }
    if (!blank) {
      break;
    }
    last--;
  }

  // 4) 计算最小公共缩进（仅在非空白行上统计）。
  uint64_t min_indent = UINT64_MAX;
  for (uint64_t li = first; li <= last; li++) {
    uint64_t s = starts[li];
    uint64_t e = ends[li];

    // 跳过空白行（不参与 indent 计算）。
    int blank = 1;
    for (uint64_t i = s; i < e; i++) {
      if (!scoop_is_blank_ws(data[i])) {
        blank = 0;
        break;
      }
    }
    if (blank) {
      continue;
    }

    uint64_t indent = 0;
    while (s + indent < e && scoop_is_indent_ws(data[s + indent])) {
      indent++;
    }

    if (indent < min_indent) {
      min_indent = indent;
    }
  }

  if (min_indent == UINT64_MAX) {
    min_indent = 0;
  }

  // 5) 分配输出（上界为输入长度；trimIndent 只会变短）。
  uint8_t *out = (uint8_t *)malloc((size_t)len);
  if (out == 0) {
    free(starts);
    free(ends);
    return value;
  }

  uint64_t out_len = 0;
  for (uint64_t li = first; li <= last; li++) {
    uint64_t s = starts[li];
    uint64_t e = ends[li];

    // drop min_indent（不足则 drop 到行尾）。
    uint64_t drop = 0;
    while (drop < min_indent && s + drop < e && scoop_is_indent_ws(data[s + drop])) {
      drop++;
    }
    uint64_t ts = s + drop;

    // 若剩余全是空白，把该行规范化为真正的空行（不保留空格）。
    int blank = 1;
    for (uint64_t i = ts; i < e; i++) {
      if (!scoop_is_blank_ws(data[i])) {
        blank = 0;
        break;
      }
    }
    if (!blank) {
      uint64_t n = e - ts;
      for (uint64_t i = 0; i < n; i++) {
        out[out_len + i] = data[ts + i];
      }
      out_len += n;
    }

    if (li != last) {
      out[out_len] = (uint8_t)'\n';
      out_len++;
    }
  }

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    // OOM：尽力回收已分配的 buffer。
    free(out);
    free(starts);
    free(ends);
    return value;
  }

  out_str->len = out_len;
  out_str->data = out;

  free(starts);
  free(ends);
  return out_str;
}

// 运行时初始化（后续可由编译器生成的 main 调用）
void scoop_runtime_init(void) {
  (void)pthread_mutex_lock(&scoop_rt_init_lock);

  // 说明：当前阶段允许被重复调用（避免在多入口/测试场景下因重复 init 直接崩溃）。
  // 在引入线程注册后（TODO T0903/T0911），这里升级为“线程安全的幂等初始化”。
  if (scoop_rt_initialized) {
    scoop_rt_init_calls++;
    // 运行期 stackmap registry：允许重复调用 init（但 registry 自身幂等），便于测试复用。
    (void)scoop_stackmap_registry_register_current_process();
    SCOOP_RT_LOG("scoop_runtime_init: already initialized (calls=%" PRIu32 ")",
                 scoop_rt_init_calls);
    (void)pthread_mutex_unlock(&scoop_rt_init_lock);
    return;
  }

  scoop_rt_initialized = 1;
  scoop_rt_init_calls = 1;

  // 解析一次 GC stress 开关（仅首次 init 生效）。
  scoop_rt_gc_stress_interval = scoop_rt_parse_gc_stress_interval();

  scoop_gc_heap_init(&scoop_gc_heap);

  const uint32_t stackmaps_added = scoop_stackmap_registry_register_current_process();
  (void)stackmaps_added;

  SCOOP_RT_LOG("scoop_runtime_init: ok (ScoopString size=%zu, data_off=%zu)",
               sizeof(ScoopString),
               offsetof(ScoopString, data));
  SCOOP_RT_LOG("scoop_runtime_init: stackmaps registered=%" PRIu32, stackmaps_added);
  if (scoop_rt_gc_stress_interval != 0) {
    SCOOP_RT_LOG("scoop_runtime_init: GC stress enabled (interval=%" PRIu64 ")",
                 scoop_rt_gc_stress_interval);
  }

  (void)pthread_mutex_unlock(&scoop_rt_init_lock);
}

// 最小占位分配 API（后续替换为真正 GC 分配）。
//
// 约定（TODO T0908）：
// - `scoop_alloc(size)` 的 `size` 表示“对象总大小（字节）”，包含对象头（header）与 payload；
// - 返回指针指向对象头起始地址（`ScoopGcObjectHeader*`）；
// - v0：仍以 libc `malloc` 为底层分配器，但会登记到 heap 链表，供 `scoop_gc_collect()` sweep。
void *scoop_alloc(uint64_t size) {
  // 说明：保持与其它 runtime API 一致：允许在未显式 init 的情况下被调用。
  if (!scoop_rt_initialized) {
    scoop_runtime_init();
  }

  // T1409a：为保证协作式 STW 的并发边界清晰，分配前确保当前线程已注册到 runtime/GC。
  // （幂等：重复调用无副作用）
  scoop_thread_register();

  // safepoint（poll）：允许 GC stop-the-world 在分配前暂停当前线程（TODO T0911）。
  //
  // 说明：
  // - moving GC 需要在 stop-the-world 时定位并更新该线程 stackmap spill slots；
  // - 该 ctx 由 `scoop_gc_safepoint_poll` 在 park 前捕获（T1505b/T1506）。
  void scoop_gc_safepoint_poll(void);
  scoop_gc_safepoint_poll();

  // GC stress：在分配前触发额外 GC（避免返回后对象尚未入 roots 时被误回收）。
  if (scoop_rt_gc_stress_interval != 0) {
    const uint64_t interval = scoop_rt_gc_stress_interval;
    const uint64_t count = atomic_fetch_add(&scoop_rt_gc_stress_alloc_counter, 1u) + 1u;
    if (interval == 1 || (count % interval) == 0) {
      scoop_gc_collect();
    }
  }

  // 说明（early stage）：
  // - 当前以 libc `malloc` 作为最小可用实现，保证 codegen 侧能稳定拿到非空指针；
  // - 会初始化对象头（type_desc/size/flags/mark 等），并登记到 heap 链表供 `scoop_gc_collect()` sweep；
  // - 对象字段扫描依赖 type descriptor（`hdr->type_desc`），后续会由 typed alloc/codegen 写入（TODO T0907+）。
  // - OOM 时返回 NULL，由上层决定如何处理（未来可映射到 Raise<RuntimeError>）。
  uint64_t object_size = size;
  if (object_size == 0) {
    // `malloc(0)` 的返回值在不同实现上可能为 NULL 或唯一指针；为保持可预期，这里至少分配对象头大小。
    object_size = (uint64_t)sizeof(ScoopGcObjectHeader);
  }
  if (object_size < (uint64_t)sizeof(ScoopGcObjectHeader)) {
    // 保守策略：若调用方传入的 size 小于对象头，则强制提升到对象头大小，避免后续写 header 越界。
    object_size = (uint64_t)sizeof(ScoopGcObjectHeader);
  }
  if (object_size > (uint64_t)SIZE_MAX) {
    return 0;
  }

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
  // Immix v0：block/line allocator（协作式 STW，多线程可用）。
  //
  // 注意：
  // - 为避免改变对外 ABI，这里通过 `scoop_gc_heap.free_list` 读取 Immix state；
  // - large object（超过单个 block payload）当前回退到 `malloc`；小对象走 Immix blocks。
  // - T1409a：引入 thread-local current block，使常见分配路径不再持有全局 GC 锁。
  ScoopGcImmixState *state = scoop_gc_immix_state_from_heap(&scoop_gc_heap);
  if (state == 0 || !state->lock_inited) {
    // 理论上 runtime_init 会先调用 heap_init 初始化 state；这里保守回退。
    void *p = malloc((size_t)object_size);
    if (p == 0) {
      SCOOP_RT_LOG("scoop_alloc: oom (size=%" PRIu64 ")", object_size);
      return 0;
    }

    ScoopGcObjectHeader *hdr = (ScoopGcObjectHeader *)p;
    hdr->next = 0;
    hdr->type_desc = 0;
    hdr->size_bytes = object_size;
    hdr->flags = 0;
    hdr->mark = 0;

    void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj);
    scoop_gc_heap_register_object(hdr);
    return p;
  }

  void *p = 0;

  size_t cap = scoop_gc_immix_block_payload_capacity();
  if ((size_t)object_size > cap) {
    p = malloc((size_t)object_size);
  } else {
    // T1412b：nursery 优先（bump-only + 上限）。nursery 用尽后回退到 old allocator。
    if (state->nursery_max_blocks != 0) {
      (void)pthread_mutex_lock(&state->lock);
      p = scoop_gc_immix_nursery_alloc_locked(state, (size_t)object_size, (size_t)sizeof(void *));
      (void)pthread_mutex_unlock(&state->lock);
    }

    // bump-in-hole（Immix v0 / T1409a）：
    // - 线程优先在自己的 current block 内分配（无锁快路径）；
    // - 当 block 放不下时，才进入全局锁从 block pool 取一个新 block（refill）。
    ScoopGcImmixBlock *block = (ScoopGcImmixBlock *)scoop_tls.gc_immix_current_block;

    for (uint32_t tries = 0; tries < 64 && p == 0; tries++) {
      if (block == 0) {
        // 1) 无锁：先尝试从 TLS cache 取一个 block。
        block = scoop_gc_immix_tls_cache_pop();

        // 2) cache 空：持锁批量 refill，然后再 pop 一个。
        if (block == 0) {
          (void)pthread_mutex_lock(&state->lock);
          scoop_gc_immix_tls_cache_refill_locked(state);
          (void)pthread_mutex_unlock(&state->lock);
          block = scoop_gc_immix_tls_cache_pop();
        }

        scoop_tls.gc_immix_current_block = (void *)block;
      }

      if (block == 0) {
        break;
      }

      p = scoop_gc_immix_block_alloc(block, (size_t)object_size, (size_t)sizeof(void *));
      if (p != 0) {
        break;
      }

      // 当前 block 放不下：丢弃 thread-local 指针并尝试 refill 新 block。
      block = 0;
      scoop_tls.gc_immix_current_block = 0;
    }
  }

  if (p == 0) {
    SCOOP_RT_LOG("scoop_alloc: oom (size=%" PRIu64 ")", object_size);
    return 0;
  }

  // 初始化对象头（v0）：保持字段为确定值，便于测试/调试与后续 GC 接入。
  ScoopGcObjectHeader *hdr = (ScoopGcObjectHeader *)p;
  hdr->next = 0;
  hdr->type_desc = 0;
  hdr->size_bytes = object_size;
  hdr->flags = 0;
  hdr->mark = 0;

  // 登记到 heap 链表（用于 sweep；Immix backend 下为 lock-free push）。
  void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj);
  scoop_gc_heap_register_object(hdr);
  return p;
#else
  void *p = malloc((size_t)object_size);
  if (p == 0) {
    SCOOP_RT_LOG("scoop_alloc: oom (size=%" PRIu64 ")", object_size);
    return 0;
  }

  // 初始化对象头（v0）：保持字段为确定值，便于测试/调试与后续 GC 接入。
  ScoopGcObjectHeader *hdr = (ScoopGcObjectHeader *)p;
  hdr->next = 0; // 先置 0，随后会被登记到 heap 链表。
  hdr->type_desc = 0;
  hdr->size_bytes = object_size;
  hdr->flags = 0;
  hdr->mark = 0;

  // 登记到 heap 链表（用于 sweep）。多线程下由 GC 模块加锁保护（TODO T0911）。
  void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj);
  scoop_gc_heap_register_object(hdr);

  return p;
#endif
}

// 手动触发一次 GC（带 statepoint/stackmap record 的 wrapper）。
//
// 说明：
// - `scoop_gc_collect()` 本身返回 `void`；在 LLVM statepoint-example 策略下，某些情况下
//   仅依赖返回 `void` 的调用点不会生成可用于 stackmap roots 的 statepoint record；
// - 为了让 sysroot 测试辅助 `__scoop_gc_collect()` 也能稳定产出 record（并在 GC 期间枚举 roots），
//   这里提供一个返回指针的 wrapper：后端可将其调用点作为 safepoint 重写为 statepoint。
//
// 返回值：
// - 当前固定返回 NULL；仅用于让调用点在 IR 层面具备“返回 GC ref”的形状，从而生成 stackmaps。
void *scoop_gc_collect_safepoint(void) {
  scoop_gc_collect();
  return 0;
}

// Typed alloc：在 `scoop_alloc` 的基础上写入对象头的 `type_desc`。
//
// 说明：
// - `scoop_alloc` 负责：safepoint、底层分配、初始化对象头（size/mark 等）与 heap 登记；
// - 本函数只补齐 `hdr->type_desc`，使 GC 能通过 `ScoopTypeDescriptor` 扫描对象内部引用字段；
// - `type_desc == NULL` 合法：表示该对象没有可扫描引用字段（或暂未接入 type descriptor）。
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes) {
  void *p = scoop_alloc(size_bytes);
  if (p == 0) {
    return 0;
  }

  ScoopGcObjectHeader *hdr = (ScoopGcObjectHeader *)p;
  hdr->type_desc = type_desc;
  return p;
}

// 打印（不追加换行）。
//
// 说明：该 API 由 sysroot 的 `print(String)` 映射到 runtime 符号（见 TODO T0821/T0822）。
void scoop_print(const ScoopString *value) {
  if (value == 0) {
    return;
  }
  if (value->data == 0 || value->len == 0) {
    return;
  }
  if (value->len > (uint64_t)SIZE_MAX) {
    return;
  }

  // 错误模型（early stage）：
  // - platform write 失败时当前不抛错（未来可升级为 `Raise<RuntimeError>`）。
  (void)scoop_platform_io_write_stdout_all(value->data, (size_t)value->len);
}

// 打印并追加换行。
//
// 说明：该 API 由 sysroot 的 `println(String)` 映射到 runtime 符号（见 TODO T0821/T0822）。
void scoop_println(const ScoopString *value) {
  scoop_print(value);
  (void)scoop_platform_io_write_stdout_byte((uint8_t)'\n');
}

// 前置声明：stdin readLine 会复用本文件后续定义的字符串构造 helper。
static const ScoopString *scoop_string_from_owned_bytes(uint8_t *value, uint64_t len);

// --- std v2：io（T1318e） ---
//
// 说明：
// - 这组 API 用于在 `print/println` 之外，提供最小的 stdin/stdout/stderr 抽象；
// - early stage 先固定 API 形状：stdout/stderr 的写入 + stdin 的 readLine（UTF-8）；
// - 错误模型后续可升级为 `Raise<RuntimeError>`，当前阶段统一用 NULL（None）表示失败/EOF。

// `scoop.io.Stdout.writeString(value: String): Unit`
void scoop_io_stdout_write_string(const ScoopString *value) {
  if (value == 0) {
    return;
  }
  if (value->data == 0 || value->len == 0) {
    return;
  }
  if (value->len > (uint64_t)SIZE_MAX) {
    return;
  }

  (void)scoop_platform_io_write_stdout_all(value->data, (size_t)value->len);
}

// `scoop.io.stdoutWriteLine(value: String): Unit`
//
// 说明：
// - 该函数用于 fixtures/示例的最小可观测性（TODO T1320c）；
// - 行尾换行目前固定为 `\n`（与 `println` 一致）；更复杂的平台差异后续通过 backend 隔离。
void scoop_io_stdout_write_line(const ScoopString *value) {
  if (value != 0 && value->data != 0 && value->len != 0 && value->len <= (uint64_t)SIZE_MAX) {
    (void)scoop_platform_io_write_stdout_all(value->data, (size_t)value->len);
  }
  (void)scoop_platform_io_write_stdout_byte((uint8_t)'\n');
}

// `scoop.io.Stderr.writeString(value: String): Unit`
void scoop_io_stderr_write_string(const ScoopString *value) {
  if (value == 0) {
    return;
  }
  if (value->data == 0 || value->len == 0) {
    return;
  }
  if (value->len > (uint64_t)SIZE_MAX) {
    return;
  }

  (void)scoop_platform_io_write_stderr_all(value->data, (size_t)value->len);
}

// `scoop.io.stderrWriteLine(value: String): Unit`
//
// 说明：
// - 该函数用于 fixtures/示例的最小可观测性（TODO T1320c）；
// - 行尾换行目前固定为 `\n`（与 `println` 一致）；更复杂的平台差异后续通过 backend 隔离。
void scoop_io_stderr_write_line(const ScoopString *value) {
  if (value != 0 && value->data != 0 && value->len != 0 && value->len <= (uint64_t)SIZE_MAX) {
    (void)scoop_platform_io_write_stderr_all(value->data, (size_t)value->len);
  }
  (void)scoop_platform_io_write_stderr_byte((uint8_t)'\n');
}

// `scoop.io.Stdin.readLine(): String?`
//
// 返回值约定：
// - 返回 NULL 表示 `None`（EOF 或错误）；
// - 返回非 NULL 表示 `Some(String)`，其中 String 为 runtime `ScoopString*`。
const ScoopString *scoop_io_stdin_read_line_utf8(void) {
  // v0：最小实现 —— 从 stdin 读取直到 '\n' 或 EOF；返回的字符串不包含行尾换行。
  //
  // 说明：
  // - 这里不依赖 POSIX `getline`，避免 `ssize_t`/可用性差异；
  // - buffer 采用 `realloc` 递增扩容；发生 OOM 时直接返回 NULL（等价于 `None`）。
  uint8_t *buf = 0;
  size_t cap = 0;
  size_t len = 0;
  int terminated_by_eof = 0;

  for (;;) {
    uint8_t ch = 0;
    size_t nread = 0;
    if (!scoop_platform_io_read_stdin(&ch, 1, &nread)) {
      if (buf != 0) {
        free(buf);
      }
      return 0;
    }
    if (nread == 0) {
      terminated_by_eof = 1;
      break;
    }
    if (ch == (uint8_t)'\n') {
      break;
    }

    if (len == cap) {
      size_t next_cap = cap == 0 ? 64 : cap * 2;
      uint8_t *next = (uint8_t *)realloc(buf, next_cap);
      if (next == 0) {
        if (buf != 0) {
          free(buf);
        }
        return 0;
      }
      buf = next;
      cap = next_cap;
    }
    buf[len++] = ch;
  }

  // EOF 且未读取到任何字节：视为 `None`。
  // 注意：空行（仅包含 '\n'）应当返回 `Some("")`，因此这里必须区分 “EOF” 与 “空行”。
  if (len == 0 && terminated_by_eof) {
    if (buf != 0) {
      free(buf);
    }
    return 0;
  }

  // 处理 Windows 风格行尾：若内容以 '\r' 结尾（`\r\n`），去掉 '\r'。
  if (len > 0 && buf[len - 1] == '\r') {
    len--;
  }

  return scoop_string_from_owned_bytes(buf, (uint64_t)len);
}

// 最小格式化工具：把整数写入 UTF-8 buffer，并返回写入的字节数（不含 '\0'）。
//
// 说明：
// - 该 API 用于 early stage 的 f-string 插值 `{Int}`（TODO T0823）；
// - 采用 “caller 提供 buffer + cap” 的形式，避免在 runtime 侧引入堆分配依赖；
// - 当前实现依赖 libc `snprintf`；后续可替换为无 libc 的实现，或接入真正的 String API。
uint64_t scoop_format_i64(int64_t value, uint8_t *out, uint64_t cap) {
  if (out == 0 || cap == 0) {
    return 0;
  }

  int n = snprintf((char *)out, (size_t)cap, "%" PRId64, value);
  if (n <= 0) {
    return 0;
  }

  uint64_t u = (uint64_t)n;
  // 若发生截断，按“已写入的最大长度”返回（保守且可用）。
  if (u >= cap) {
    return cap - 1;
  }
  return u;
}

uint64_t scoop_format_u64(uint64_t value, uint8_t *out, uint64_t cap) {
  if (out == 0 || cap == 0) {
    return 0;
  }

  int n = snprintf((char *)out, (size_t)cap, "%" PRIu64, value);
  if (n <= 0) {
    return 0;
  }

  uint64_t u = (uint64_t)n;
  if (u >= cap) {
    return cap - 1;
  }
  return u;
}

// --- std v2：env / time（T1318a）---
//
// 说明：
// - 这组 API 作为“平台能力”，不应新增编译器 intrinsic；
//   由 runtime lib 提供最小 C ABI，再由 sysroot/std 表面封装（见 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`）。
// - 当前实现仅覆盖 host POSIX/desktop 的最小 happy path；错误处理与资源释放策略后续补齐。

static const ScoopString *scoop_string_empty(void) {
  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    return 0;
  }
  out_str->len = 0;
  out_str->data = 0;
  return out_str;
}

// 用静态字节序列构造 runtime String（不拷贝 bytes）。
//
// 说明：
// - `value` 必须指向进程生命周期内有效的只读数据（例如字符串字面量的全局常量、或 runtime 内建常量）；
// - 当前阶段不接入 type descriptor/release，因此该 String 不会释放 `value` 指向的内存。
static const ScoopString *scoop_string_from_static_bytes(const uint8_t *value, uint64_t len) {
  if (len == 0) {
    return scoop_string_empty();
  }
  if (value == 0) {
    return 0;
  }

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    return 0;
  }
  out_str->len = len;
  out_str->data = value;
  return out_str;
}

static const ScoopString *scoop_string_from_cstr(const char *value) {
  if (value == 0) {
    return 0;
  }
  size_t n = strlen(value);
  if (n == 0) {
    return scoop_string_empty();
  }

  uint8_t *out = (uint8_t *)malloc(n);
  if (out == 0) {
    return 0;
  }
  (void)memcpy(out, value, n);

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    free(out);
    return 0;
  }
  out_str->len = (uint64_t)n;
  out_str->data = out;
  return out_str;
}

static const ScoopString *scoop_string_from_bytes(const uint8_t *value, uint64_t len) {
  if (value == 0) {
    return 0;
  }
  if (len == 0) {
    return scoop_string_empty();
  }

  uint8_t *out = (uint8_t *)malloc((size_t)len);
  if (out == 0) {
    return 0;
  }
  (void)memcpy(out, value, (size_t)len);

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    free(out);
    return 0;
  }
  out_str->len = len;
  out_str->data = out;
  return out_str;
}

// 以 “转移所有权” 的方式构造 runtime String（避免二次拷贝）。
static const ScoopString *scoop_string_from_owned_bytes(uint8_t *value, uint64_t len) {
  if (len == 0) {
    if (value != 0) {
      free(value);
    }
    return scoop_string_empty();
  }
  if (value == 0) {
    return 0;
  }

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    free(value);
    return 0;
  }
  out_str->len = len;
  out_str->data = value;
  return out_str;
}

static char *scoop_cstr_from_scoop_string(const ScoopString *value) {
  if (value == 0) {
    return 0;
  }
  if (value->data == 0 || value->len == 0) {
    return 0;
  }

  uint64_t n = value->len;
  char *out = (char *)malloc((size_t)n + 1);
  if (out == 0) {
    return 0;
  }
  (void)memcpy(out, value->data, (size_t)n);
  out[n] = '\0';
  return out;
}

// `scoop.env.get(key: String): String?`
//
// 约定：
// - 返回 NULL 表示 `None`（与 `Option<RefType>` 的 niche 表示对齐）；
// - 返回非 NULL 表示 `Some(String)`，其中 String 为 runtime `ScoopString*`。
const ScoopString *scoop_env_get(const ScoopString *key) {
  if (key == 0 || key->data == 0) {
    return 0;
  }

  // getenv 需要 NUL 结尾的 key：复制一份并追加 '\0'。
  uint64_t key_len = key->len;
  char *key_cstr = (char *)malloc((size_t)key_len + 1);
  if (key_cstr == 0) {
    return 0;
  }
  if (key_len > 0) {
    (void)memcpy(key_cstr, key->data, (size_t)key_len);
  }
  key_cstr[key_len] = '\0';

  const char *value = scoop_platform_env_getenv(key_cstr);
  free(key_cstr);

  return scoop_string_from_cstr(value);
}

// `scoop.time.nowUnixMillis(): Int`
//
// 说明：
// - 使用 `gettimeofday` 作为跨 Unix 平台的最小实现（避免旧平台 `clock_gettime` 的链接差异）；
// - 以 “Unix epoch 毫秒（UTC）” 表示当前时间戳；
// - 失败时返回 0（后续可升级为 `Raise<RuntimeError>` 或 `Result`）。
int64_t scoop_time_now_unix_millis(void) {
  int64_t unix_millis = 0;
  if (!scoop_platform_time_now_unix_millis(&unix_millis)) {
    return 0;
  }

  return unix_millis;
}

// --- std v2：fs（T1318b）---
//
// 说明：
// - 该组 API 提供“最小可执行的文本（UTF-8）文件读写”能力；
// - 当前阶段不做完整错误模型：读失败返回 NULL（对应 `None`），写失败返回非 0；
// - 实现面向 host POSIX/desktop：`fopen/fread/fwrite`。

// `scoop.fs.readAllText(path: String): String?`
//
// 约定：
// - 返回 NULL 表示 `None`（读失败/文件不存在/平台不支持）；
// - 返回非 NULL 表示 `Some(String)`。
const ScoopString *scoop_fs_read_all_text_utf8(const ScoopString *path) {
  char *path_cstr = scoop_cstr_from_scoop_string(path);
  if (path_cstr == 0) {
    return 0;
  }

  FILE *f = fopen(path_cstr, "rb");
  free(path_cstr);
  if (f == 0) {
    return 0;
  }

  if (fseek(f, 0, SEEK_END) != 0) {
    (void)fclose(f);
    return 0;
  }
  long n_long = ftell(f);
  if (n_long < 0) {
    (void)fclose(f);
    return 0;
  }
  if (fseek(f, 0, SEEK_SET) != 0) {
    (void)fclose(f);
    return 0;
  }

  uint64_t n = (uint64_t)n_long;
  if (n == 0) {
    (void)fclose(f);
    return scoop_string_empty();
  }

  if (n > (uint64_t)SIZE_MAX) {
    (void)fclose(f);
    return 0;
  }

  uint8_t *buf = (uint8_t *)malloc((size_t)n);
  if (buf == 0) {
    (void)fclose(f);
    return 0;
  }

  size_t got = fread(buf, 1, (size_t)n, f);
  (void)fclose(f);
  if (got != (size_t)n) {
    free(buf);
    return 0;
  }

  const ScoopString *out = scoop_string_from_bytes(buf, n);
  free(buf);
  return out;
}

// `scoop.fs.writeAllText(path: String, content: String): Int`
//
// 约定：
// - 返回 0 表示成功；
// - 返回非 0 表示失败。
int64_t scoop_fs_write_all_text_utf8(const ScoopString *path, const ScoopString *content) {
  char *path_cstr = scoop_cstr_from_scoop_string(path);
  if (path_cstr == 0) {
    return 1;
  }

  FILE *f = fopen(path_cstr, "wb");
  free(path_cstr);
  if (f == 0) {
    return 2;
  }

  uint64_t n = 0;
  const uint8_t *data = 0;
  if (content != 0) {
    n = content->len;
    data = content->data;
  }

  if (n > 0) {
    if (data == 0) {
      (void)fclose(f);
      return 3;
    }
    if (n > (uint64_t)SIZE_MAX) {
      (void)fclose(f);
      return 4;
    }
    size_t wrote = fwrite(data, 1, (size_t)n, f);
    if (wrote != (size_t)n) {
      (void)fclose(f);
      return 5;
    }
  }

  if (fclose(f) != 0) {
    return 6;
  }
  return 0;
}

// --- std v2：process（T1318c） ---
//
// 说明：
// - `scoop.process.args()` 读取启动参数（argv，不含 argv[0]）；
// - `scoop.process.exit(code)` 主动退出当前进程；
// - 当前阶段只做 host 平台 happy path：不处理宽字符（Windows）、不做复杂错误模型。

// 进程启动参数（由 LLVM 入口 `main(argc, argv)` 在最早期写入）。
static int32_t scoop_process_argc = 0;
static const char **scoop_process_argv = 0;
static void *scoop_process_args_cache = 0;

// `scoop_process_init(argc, argv)`：由入口 main 调用，保存 argv 指针。
void scoop_process_init(int32_t argc, const char **argv) {
  scoop_process_argc = argc;
  scoop_process_argv = argv;
  scoop_process_args_cache = 0;
}

// `scoop.process.exit(code: Int): Unit`
//
// 说明：直接映射到 libc `exit(3)`；不做 unwind/清理语义。
void scoop_process_exit(int64_t code) {
  exit((int)code);
}

// `scoop.process.args(): Array<String>`
//
// 约定：
// - 返回的数组不包含 argv[0]（与 Kotlin 的 main(args) 对齐）；
// - 首次调用时构造并缓存（early stage：可能泄漏；后续由 GC/运行时托管补齐）。
void *scoop_process_args_array(void) {
  if (scoop_process_args_cache != 0) {
    return scoop_process_args_cache;
  }

  // Array builder 由 `runtime/c/scoop_array.c` 提供。
  void *scoop_array_builder_new(void);
  void scoop_array_builder_push_ref(void *builder, void *value);
  void *scoop_array_builder_build_array(void *builder);

  void *builder = scoop_array_builder_new();
  if (builder == 0) {
    return 0;
  }

  int32_t argc = scoop_process_argc;
  const char **argv = scoop_process_argv;
  if (argc <= 1 || argv == 0) {
    void *arr = scoop_array_builder_build_array(builder);
    scoop_process_args_cache = arr;
    return arr;
  }

  // 跳过 argv[0]（程序路径），只保留用户参数。
  for (int32_t i = 1; i < argc; i++) {
    const char *s = argv[i];
    const ScoopString *str = scoop_string_from_cstr(s);
    scoop_array_builder_push_ref(builder, (void *)str);
  }

  void *arr = scoop_array_builder_build_array(builder);
  scoop_process_args_cache = arr;
  return arr;
}

// --- std v2：path（T1318d） ---
//
// 说明：
// - 该组 API 提供“最小可执行的路径操作”：normalize/join/basename/dirname；
// - 当前阶段仅按 host 分隔符规则处理：POSIX 用 `/`，Windows（若未来支持）用 `\\`；
// - 该实现是 “字符串层面的最小归一化”，不尝试做完整的 filesystem 语义（例如符号链接解析）。

static uint8_t scoop_path_sep_byte(void) {
#ifdef _WIN32
  return (uint8_t)'\\';
#else
  return (uint8_t)'/';
#endif
}

static int scoop_path_is_sep(uint8_t b) {
#ifdef _WIN32
  return b == (uint8_t)'/' || b == (uint8_t)'\\';
#else
  return b == (uint8_t)'/';
#endif
}

#ifdef _WIN32
static int scoop_path_is_alpha(uint8_t b) {
  return (b >= (uint8_t)'A' && b <= (uint8_t)'Z') || (b >= (uint8_t)'a' && b <= (uint8_t)'z');
}
#endif

static int scoop_path_is_absolute(const uint8_t *data, uint64_t len) {
  if (data == 0 || len == 0) {
    return 0;
  }
  if (scoop_path_is_sep(data[0])) {
    return 1;
  }
#ifdef _WIN32
  // 简化：把 `C:\...` / `C:...` 视为绝对（更严格规则留给后续任务）。
  if (len >= 2 && scoop_path_is_alpha(data[0]) && data[1] == (uint8_t)':') {
    return 1;
  }
#endif
  return 0;
}

static const ScoopString *scoop_path_root_string(void) {
#ifdef _WIN32
  return scoop_string_from_static_bytes(SCOOP_BACKSLASH_BYTES, 1);
#else
  return scoop_string_from_static_bytes(SCOOP_SLASH_BYTES, 1);
#endif
}

// `scoop.path.normalize(path: String): String`
const ScoopString *scoop_path_normalize(const ScoopString *path) {
  if (path == 0 || path->len == 0 || path->data == 0) {
    return scoop_string_empty();
  }

  const uint8_t *in = path->data;
  uint64_t n = path->len;
  if (n > (uint64_t)SIZE_MAX) {
    return 0;
  }

  uint8_t sep = scoop_path_sep_byte();
  uint8_t *out = (uint8_t *)malloc((size_t)n);
  if (out == 0) {
    return 0;
  }

  uint64_t j = 0;
  int prev_sep = 0;
  for (uint64_t i = 0; i < n; i++) {
    uint8_t b = in[i];
    if (scoop_path_is_sep(b)) {
      if (j == 0) {
        out[j++] = sep;
        prev_sep = 1;
        continue;
      }
      if (!prev_sep) {
        out[j++] = sep;
        prev_sep = 1;
      }
      continue;
    }

    out[j++] = b;
    prev_sep = 0;
  }

  // trim trailing separators（保留根路径 `/`；Windows 的 `C:\` 特例也保留）
  while (j > 1 && out[j - 1] == sep) {
#ifdef _WIN32
    if (j == 3 && scoop_path_is_alpha(out[0]) && out[1] == (uint8_t)':' && out[2] == sep) {
      break;
    }
#endif
    j--;
  }

  return scoop_string_from_owned_bytes(out, j);
}

// `scoop.path.join(base: String, child: String): String`
const ScoopString *scoop_path_join(const ScoopString *base, const ScoopString *child) {
  if (child == 0 || child->len == 0 || child->data == 0) {
    return scoop_path_normalize(base);
  }
  if (base == 0 || base->len == 0 || base->data == 0) {
    return scoop_path_normalize(child);
  }

  if (scoop_path_is_absolute(child->data, child->len)) {
    return scoop_path_normalize(child);
  }

  const uint8_t *base_data = base->data;
  uint64_t base_len = base->len;
  const uint8_t *child_data = child->data;
  uint64_t child_len = child->len;

  // 去掉 base 末尾多余分隔符（保留根路径 `/`）。
  uint64_t base_end = base_len;
  while (base_end > 1 && scoop_path_is_sep(base_data[base_end - 1])) {
    base_end--;
  }
  // 去掉 child 开头的分隔符，避免 join 出现重复分隔符。
  uint64_t child_start = 0;
  while (child_start < child_len && scoop_path_is_sep(child_data[child_start])) {
    child_start++;
  }

  uint8_t sep = scoop_path_sep_byte();
  int need_sep = 0;
  if (base_end > 0 && child_start < child_len && !scoop_path_is_sep(base_data[base_end - 1])) {
    need_sep = 1;
  }

  uint64_t out_len = base_end + (need_sep ? 1 : 0) + (child_len - child_start);
  if (out_len == 0) {
    return scoop_string_empty();
  }
  if (out_len > (uint64_t)SIZE_MAX) {
    return 0;
  }

  uint8_t *buf = (uint8_t *)malloc((size_t)out_len);
  if (buf == 0) {
    return 0;
  }

  if (base_end > 0) {
    (void)memcpy(buf, base_data, (size_t)base_end);
  }
  uint64_t pos = base_end;
  if (need_sep) {
    buf[pos++] = sep;
  }
  uint64_t right_len = child_len - child_start;
  if (right_len > 0) {
    (void)memcpy(buf + pos, child_data + child_start, (size_t)right_len);
  }

  ScoopString tmp = {0};
  tmp.len = out_len;
  tmp.data = buf;
  const ScoopString *norm = scoop_path_normalize(&tmp);
  free(buf);
  return norm;
}

// `scoop.path.basename(path: String): String`
const ScoopString *scoop_path_basename(const ScoopString *path) {
  const ScoopString *norm = scoop_path_normalize(path);
  if (norm == 0 || norm->len == 0 || norm->data == 0) {
    return scoop_string_empty();
  }

  const uint8_t *data = norm->data;
  uint64_t n = norm->len;

  // 根路径 `/`：basename 为 `/`。
  if (n == 1 && scoop_path_is_sep(data[0])) {
    return norm;
  }

  // 忽略末尾分隔符。
  while (n > 1 && scoop_path_is_sep(data[n - 1])) {
    n--;
  }

  // 找最后一个分隔符。
  uint64_t last_sep = (uint64_t)-1;
  for (uint64_t i = n; i > 0; i--) {
    if (scoop_path_is_sep(data[i - 1])) {
      last_sep = i - 1;
      break;
    }
  }
  if (last_sep == (uint64_t)-1) {
    return norm;
  }
  if (last_sep == 0 && n == 1) {
    return norm;
  }

  uint64_t start = last_sep + 1;
  uint64_t len = n - start;
  if (len == 0) {
    return scoop_string_empty();
  }
  return scoop_string_from_bytes(data + start, len);
}

// `scoop.path.dirname(path: String): String`
const ScoopString *scoop_path_dirname(const ScoopString *path) {
  const ScoopString *norm = scoop_path_normalize(path);
  if (norm == 0 || norm->len == 0 || norm->data == 0) {
    return scoop_string_from_static_bytes(SCOOP_DOT_BYTES, 1);
  }

  const uint8_t *data = norm->data;
  uint64_t n = norm->len;

  // 根路径 `/`：dirname 为 `/`。
  if (n == 1 && scoop_path_is_sep(data[0])) {
    return norm;
  }

  // 忽略末尾分隔符。
  while (n > 1 && scoop_path_is_sep(data[n - 1])) {
    n--;
  }

  // 找最后一个分隔符。
  uint64_t last_sep = (uint64_t)-1;
  for (uint64_t i = n; i > 0; i--) {
    if (scoop_path_is_sep(data[i - 1])) {
      last_sep = i - 1;
      break;
    }
  }
  if (last_sep == (uint64_t)-1) {
    return scoop_string_from_static_bytes(SCOOP_DOT_BYTES, 1);
  }
  if (last_sep == 0) {
    return scoop_path_root_string();
  }

  uint64_t dir_len = last_sep;
  // 去掉 dirname 末尾多余分隔符（保留根路径）。
  while (dir_len > 1 && scoop_path_is_sep(data[dir_len - 1])) {
    dir_len--;
  }
  if (dir_len == 0) {
    return scoop_string_from_static_bytes(SCOOP_DOT_BYTES, 1);
  }
  if (dir_len == 1 && scoop_path_is_sep(data[0])) {
    return scoop_path_root_string();
  }
  return scoop_string_from_bytes(data, dir_len);
}
