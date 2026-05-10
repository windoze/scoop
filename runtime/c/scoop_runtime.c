// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
#include <stdatomic.h>
#include <inttypes.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc.h"
#include "scoop_gc_backend.h"
#include "scoop_gc_immix_internal.h"
#include "scoop_root_frame.h"
#include "scoop_stackmap.h"
#include "scoop_tls_internal.h"
#include "platform/platform.h"
#include "platform/unwind.h"

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

// 前置声明：String helper（定义在文件后部）。
static const ScoopString *scoop_string_empty(void);
static const ScoopString *scoop_string_from_static_bytes(const uint8_t *value, uint64_t len);

// 通用分配入口由 runtime substrate 提供；这里保留中性前置声明，供 typed/string helpers 复用。
void *scoop_alloc(uint64_t size);

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
static inline ScoopGcImmixBlock *scoop_gc_immix_nursery_take_block_locked(
    ScoopGcImmixState *state) {
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

// explicit root frame：每线程 frame chain 栈顶。
SCOOP_THREAD_LOCAL ScoopRootFrameHeader *__scoop_explicit_root_frame_top = 0;
uint32_t scoop_thread_is_registered(void) {
  return scoop_tls.registered;
}

// `scoop_runtime_init` 定义在文件后部；这里给出前置声明以避免隐式声明警告。
void scoop_runtime_init(void);

// GC native transition (defined in scoop_gc.c / backend): transition to IN_NATIVE
// before blocking system calls, allowing STW GC to skip this thread.
void scoop_enter_native(void ***root_slots, uint32_t root_slots_len);
void scoop_leave_native(void);
void scoop_gc_thread_clear_managed_root_snapshot_current(void);

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

  __scoop_explicit_root_frame_top = 0;

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

  // GC safety (T0106): pin `value` before scoop_alloc — GC could relocate/collect it,
  // and the OOM path below returns `value` which must remain valid.
  scoop_pin((void *)value);

  ScoopString *out_str = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  if (out_str == 0) {
    // OOM：尽力回收已分配的 buffer。
    free(out);
    free(starts);
    free(ends);
    scoop_unpin((void *)value);
    return value;
  }

  out_str->len = out_len;
  out_str->data = out;

  scoop_unpin((void *)value);
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

  SCOOP_RT_LOG("scoop_runtime_init: ok (ScoopString size=%zu, data_off=%zu)",
               sizeof(ScoopString),
               offsetof(ScoopString, data));
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

// --- Shared string helpers ---
//
// 说明：
// - 这些 helper 供 process argv 适配与底层字符串实现共用；
// - 它们不是对外 runtime ABI，只在本文件内部复用。

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
// --- fatal trap / entry argv（T5000e3a / T5000e3c） ---
//
// 说明：
// - `scoop.core.panic(message)`：bottom-typed fatal trap surface；当前实现仍直接 exit(3)；
// - 可执行入口 `main(args: Array<String>)` 通过 runtime helper 直接接收 native argv；
// - 当前阶段只做 host 平台 happy path：不处理宽字符（Windows）、不做复杂错误模型。

// `scoop.core.panic(message: String): Nothing`
//
// 说明：
// - 当前阶段只要求“立即终止”语义，message 先不做 stderr 格式化；
// - fatal trap 与 `main` 的正常返回 contract 分离：这里仍直接 exit(3)，
//   不复用已经删除的 `scoop.process.exit(...)` surface。
void scoop_panic(const void *message) {
  (void)message;
  exit(3);
}

// refactor effect lowering：pure caller 本地消费 ordinary RuntimeError 后的已发布终止入口。
//
// 约定：
// - 参数是已物化的 `RuntimeError` payload object；
// - 当前实现仍只要求“立即终止”，暂不承担 stderr 格式化；
// - LLVM refactor backend 必须通过这个已发布入口结束 `LocalRuntimeError` synthetic state，
//   不能在 backend 现场自行发明隐藏 trap 路径。
void scoop_runtime_error_fatal(const void *runtime_error) {
  (void)runtime_error;
  exit(3);
}

// `scoop_entry_argv_array(argc, argv): Array<String>`
//
// 约定：
// - 直接按 native 程序边界的 argv 形状构造，包含 argv[0]；
// - 当前阶段仅由 LLVM 入口 wrapper 调用一次，因此不再保留全局缓存/过渡 surface。
void *scoop_entry_argv_array(int32_t argc, const char **argv) {
  // Array builder 由 `runtime/c/scoop_array.c` 提供。
  void *scoop_array_builder_new(void);
  void scoop_array_builder_push_ref(void *builder, void *value);
  void *scoop_array_builder_build_array(void *builder);

  void *builder = scoop_array_builder_new();
  if (builder == 0) {
    return 0;
  }

  // GC safety (T0106): pin `builder` — it's GC-managed and held across
  // scoop_string_from_cstr / scoop_array_builder_build_array calls that trigger scoop_alloc.
  scoop_pin(builder);

  if (argc <= 0 || argv == 0) {
    void *arr = scoop_array_builder_build_array(builder);
    scoop_unpin(builder);
    return arr;
  }

  for (int32_t i = 0; i < argc; i++) {
    const char *s = argv[i];
    const ScoopString *str = scoop_string_from_cstr(s);
    scoop_array_builder_push_ref(builder, (void *)str);
  }

  void *arr = scoop_array_builder_build_array(builder);
  scoop_unpin(builder);
  return arr;
}

// --- std v2：Text 基础（T1810）---
//
// 说明：
// - 基础字符串操作的 C 实现，供 sysroot/core.scoop 的 String 方法路由使用。
// - 当前为字节级操作（UTF-8 byte length/offset）；后续可扩展 codepoint/grapheme 版本。
// - 所有函数遵循 runtime/c 现有风格：null check → 边界处理 → 实际逻辑。

// scoop_string_length：返回字符串的 UTF-8 字节长度。
int64_t scoop_string_length(const ScoopString *s) {
  if (s == 0) {
    return 0;
  }
  return (int64_t)s->len;
}

// T0122: substring/starts_with/ends_with/index_of/contains/split
// 已迁移到 sysroot/string.scoop（纯 Scoop 实现）。

// ---------------------------------------------------------------------------
// T1812: 数値↔文本 変換 (Int.toString / String.toInt)
// ---------------------------------------------------------------------------

// T0114: scoop_bool_to_string：将布尔值转换为 "true" 或 "false"。
// 返回 GC-managed ScoopString*。
const ScoopString *scoop_bool_to_string(int64_t value) {
  static const uint8_t TRUE_BYTES[]  = { 't', 'r', 'u', 'e' };
  static const uint8_t FALSE_BYTES[] = { 'f', 'a', 'l', 's', 'e' };
  if (value != 0) {
    return scoop_string_from_static_bytes(TRUE_BYTES, 4);
  } else {
    return scoop_string_from_static_bytes(FALSE_BYTES, 5);
  }
}

// T0146c2: scoop_char_to_string：将单个 Unicode scalar value 编码为 UTF-8 字符串。
//
// 说明：
// - `Char` 的运行时表示为 i32 codepoint（U+0000..U+10FFFF）；
// - 对非法 codepoint（越界或 surrogate）保守降级为 U+FFFD；
// - 返回值为 GC-managed ScoopString*。
const ScoopString *scoop_char_to_string(int32_t codepoint) {
  uint32_t cp = (uint32_t)codepoint;
  if (cp > 0x10FFFFu || (cp >= 0xD800u && cp <= 0xDFFFu)) {
    cp = 0xFFFDu;
  }

  uint8_t buf[4];
  uint64_t len = 0;
  if (cp <= 0x7Fu) {
    buf[0] = (uint8_t)cp;
    len = 1;
  } else if (cp <= 0x7FFu) {
    buf[0] = (uint8_t)(0xC0u | (cp >> 6));
    buf[1] = (uint8_t)(0x80u | (cp & 0x3Fu));
    len = 2;
  } else if (cp <= 0xFFFFu) {
    buf[0] = (uint8_t)(0xE0u | (cp >> 12));
    buf[1] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
    buf[2] = (uint8_t)(0x80u | (cp & 0x3Fu));
    len = 3;
  } else {
    buf[0] = (uint8_t)(0xF0u | (cp >> 18));
    buf[1] = (uint8_t)(0x80u | ((cp >> 12) & 0x3Fu));
    buf[2] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
    buf[3] = (uint8_t)(0x80u | (cp & 0x3Fu));
    len = 4;
  }

  return scoop_string_from_bytes(buf, len);
}

// scoop_int_to_string：将 int64_t 转换为十进制字符串表示。
// 返回 GC-managed ScoopString*。
const ScoopString *scoop_int_to_string(int64_t value) {
  // snprintf with NULL first to determine length, then allocate.
  // INT64_MIN = -9223372036854775808 → max 20 digits + sign + NUL = 22 bytes.
  char buf[22];
  int n = snprintf(buf, sizeof(buf), "%lld", (long long)value);
  if (n <= 0) {
    return scoop_string_empty();
  }

  return scoop_string_from_bytes((const uint8_t *)buf, (uint64_t)n);
}

static const ScoopString *scoop_float_to_string_common(double value, int precision) {
  static const uint8_t NAN_BYTES[]      = { 'N', 'a', 'N' };
  static const uint8_t INF_BYTES[]      = { 'I', 'n', 'f', 'i', 'n', 'i', 't', 'y' };
  static const uint8_t NEG_INF_BYTES[]  = { '-', 'I', 'n', 'f', 'i', 'n', 'i', 't', 'y' };

  if (isnan(value)) {
    return scoop_string_from_static_bytes(NAN_BYTES, 3);
  }
  if (isinf(value)) {
    if (value < 0.0) {
      return scoop_string_from_static_bytes(NEG_INF_BYTES, 9);
    }
    return scoop_string_from_static_bytes(INF_BYTES, 8);
  }

  char buf[64];
  int n = snprintf(buf, sizeof(buf), "%.*g", precision, value);
  if (n <= 0) {
    return scoop_string_empty();
  }

  int has_decimal_or_exp = 0;
  for (int i = 0; i < n; i++) {
    if (buf[i] == '.' || buf[i] == 'e' || buf[i] == 'E') {
      has_decimal_or_exp = 1;
      break;
    }
  }
  if (!has_decimal_or_exp && n <= (int)(sizeof(buf) - 3)) {
    buf[n++] = '.';
    buf[n++] = '0';
  }

  return scoop_string_from_bytes((const uint8_t *)buf, (uint64_t)n);
}

// scoop_float64_to_string：将 double 转换为可读十进制字符串。
// 约定：NaN/Infinity 使用稳定文本；有限值若无小数点/指数，则补 `.0`。
const ScoopString *scoop_float64_to_string(double value) {
  return scoop_float_to_string_common(value, 17);
}

// scoop_float32_to_string：将 float 转换为可读十进制字符串。
const ScoopString *scoop_float32_to_string(float value) {
  return scoop_float_to_string_common((double)value, 9);
}

static int64_t scoop_float_to_int_common(double value) {
  if (isnan(value)) {
    return 0;
  }
  if (value >= (double)INT64_MAX) {
    return INT64_MAX;
  }
  if (value <= (double)INT64_MIN) {
    return INT64_MIN;
  }
  return (int64_t)value;
}

// scoop_float64_to_int：double -> Int，NaN 返回 0，越界时饱和到 int64 边界。
int64_t scoop_float64_to_int(double value) {
  return scoop_float_to_int_common(value);
}

// scoop_float32_to_int：float -> Int，NaN 返回 0，越界时饱和到 int64 边界。
int64_t scoop_float32_to_int(float value) {
  return scoop_float_to_int_common((double)value);
}

// scoop_string_to_int：将十进制字符串解析为 int64_t。
// 对非数字输入返回 0（v0 简单路径；后续可引入 Option<Int>）。
// 支持可选的前导 '-' 或 '+' 号，跳过前导空白。
int64_t scoop_string_to_int(const ScoopString *s) {
  if (s == 0 || s->data == 0 || s->len == 0) {
    return 0;
  }

  // 复制到 NUL-terminated 缓冲区（ScoopString 不保证 NUL 终止）。
  uint64_t len = s->len;
  if (len > 64) len = 64; // 防止栈溢出；int64_t 最多 20 位
  char buf[65];
  (void)memcpy(buf, s->data, (size_t)len);
  buf[len] = '\0';

  char *endptr = 0;
  long long result = strtoll(buf, &endptr, 10);
  // 如果没有成功解析任何字符，返回 0。
  if (endptr == buf) {
    return 0;
  }
  return (int64_t)result;
}

// scoop_string_to_float64：为后续 String.toFloat64() 预留 runtime 符号。
// v0 语义：解析失败时返回 0.0；仅接受当前字节串前缀中的合法十进制浮点表示。
double scoop_string_to_float64(const ScoopString *s) {
  if (s == 0 || s->data == 0 || s->len == 0) {
    return 0.0;
  }

  uint64_t len = s->len;
  if (len > 127) len = 127;

  char buf[128];
  (void)memcpy(buf, s->data, (size_t)len);
  buf[len] = '\0';

  char *endptr = 0;
  double result = strtod(buf, &endptr);
  if (endptr == buf) {
    return 0.0;
  }
  return result;
}

// ---------------------------------------------------------------------------
// T1816: String.concat — 连接两个字符串
// ---------------------------------------------------------------------------

// scoop_string_concat：连接两个 ScoopString，返回新的 GC-managed ScoopString*。
//
// GC 安全性：a 和 b 均为 GC heap 上的对象。scoop_alloc（通过 scoop_string_from_bytes）
// 可能触发 GC，因此在分配前 pin 住 a 和 b，防止 raw 指针悬空。
const ScoopString *scoop_string_concat(const ScoopString *a, const ScoopString *b) {
  // Null/empty cases: return the non-null/non-empty side (or empty).
  if (a == 0 || a->data == 0 || a->len == 0) {
    if (b == 0 || b->data == 0 || b->len == 0) {
      return scoop_string_empty();
    }
    return b;
  }
  if (b == 0 || b->data == 0 || b->len == 0) {
    return a;
  }

  uint64_t alen = a->len;
  uint64_t blen = b->len;
  uint64_t total = alen + blen;

  // Pin both inputs — malloc + scoop_alloc may trigger GC.
  scoop_pin((void *)a);
  scoop_pin((void *)b);

  // Allocate a temporary buffer, copy both halves, then create a GC string.
  uint8_t *buf = (uint8_t *)malloc((size_t)total);
  if (buf == 0) {
    scoop_unpin((void *)a);
    scoop_unpin((void *)b);
    return scoop_string_empty();
  }

  (void)memcpy(buf, a->data, (size_t)alen);
  (void)memcpy(buf + alen, b->data, (size_t)blen);

  const ScoopString *result = scoop_string_from_bytes(buf, total);
  free(buf);

  scoop_unpin((void *)a);
  scoop_unpin((void *)b);
  return result;
}

// ---------------------------------------------------------------------------
// T1817: String.hash — FNV-1a hash
// ---------------------------------------------------------------------------

// scoop_string_hash：对字符串内容计算 FNV-1a 哈希值，返回 i64。
//
// 算法：FNV-1a（Fowler-Noll-Vo）——简单、分布良好、适合短字符串。
// - offset basis: 14695981039346656037
// - prime: 1099511628211
int64_t scoop_string_hash(const ScoopString *s) {
  if (s == 0 || s->data == 0 || s->len == 0) {
    return 0;
  }
  uint64_t hash = 14695981039346656037ULL;
  for (int64_t i = 0; i < (int64_t)s->len; i++) {
    hash ^= (uint64_t)(uint8_t)s->data[i];
    hash *= 1099511628211ULL;
  }
  return (int64_t)hash;
}

// ---------------------------------------------------------------------------
// T0107: String.equals — structural equality comparison
// ---------------------------------------------------------------------------

// scoop_string_equals：比较两个字符串是否内容相等。返回 1（相等）或 0（不相等）。
// 语义：长度相同且字节序列相同。NULL 或空字符串之间的比较按长度判断。
int64_t scoop_string_equals(const ScoopString *a, const ScoopString *b) {
  // Same pointer (including both NULL) → equal
  if (a == b) {
    return 1;
  }
  // One NULL, the other not → not equal
  if (a == 0 || b == 0) {
    return 0;
  }
  // Different lengths → not equal
  if (a->len != b->len) {
    return 0;
  }
  // Both empty → equal
  if (a->len == 0) {
    return 1;
  }
  // Data pointer checks
  if (a->data == 0 && b->data == 0) {
    return 1;
  }
  if (a->data == 0 || b->data == 0) {
    return 0;
  }
  return memcmp(a->data, b->data, (size_t)a->len) == 0 ? 1 : 0;
}

// T0122: trim/trimStart/trimEnd + is_ascii_whitespace helper
// 已迁移到 sysroot/string.scoop（纯 Scoop 实现）。

// ── T0115: String 补齐（未迁移部分） ─────────────────────────────────

// `String.isEmpty(): Bool` — returns 1 if length is zero, 0 otherwise.
int64_t scoop_string_is_empty(const ScoopString *s) {
  if (s == 0) {
    return 1;
  }
  return s->len == 0 ? 1 : 0;
}

// `String.replace(old: String, new: String): String`
// Replace all occurrences of `old` with `new_str`. GC-safe: pin inputs before alloc.
const ScoopString *scoop_string_replace(const ScoopString *s,
                                        const ScoopString *old,
                                        const ScoopString *new_str) {
  if (s == 0 || s->len == 0 || s->data == 0) {
    return s;
  }
  if (old == 0 || old->len == 0 || old->data == 0) {
    return s;
  }
  if (new_str == 0) {
    // Treat null replacement as empty string.
    new_str = scoop_string_empty();
  }

  // Count occurrences of `old` in `s`.
  uint64_t count = 0;
  uint64_t pos = 0;
  while (pos + old->len <= s->len) {
    if (memcmp(s->data + pos, old->data, (size_t)old->len) == 0) {
      count++;
      pos += old->len;
    } else {
      pos++;
    }
  }

  if (count == 0) {
    return s;
  }

  // Calculate result length.
  uint64_t result_len = s->len - (count * old->len) + (count * new_str->len);
  if (result_len == 0) {
    return scoop_string_empty();
  }

  uint8_t *buf = (uint8_t *)malloc((size_t)result_len);
  if (buf == 0) {
    return s;
  }

  // Build result.
  uint64_t src = 0;
  uint64_t dst = 0;
  while (src < s->len) {
    if (src + old->len <= s->len &&
        memcmp(s->data + src, old->data, (size_t)old->len) == 0) {
      if (new_str->len > 0 && new_str->data != 0) {
        memcpy(buf + dst, new_str->data, (size_t)new_str->len);
      }
      dst += new_str->len;
      src += old->len;
    } else {
      buf[dst++] = s->data[src++];
    }
  }

  // Pin inputs before GC allocation.
  scoop_pin((void *)s);
  scoop_pin((void *)old);
  scoop_pin((void *)new_str);

  const ScoopString *result = scoop_string_from_bytes(buf, result_len);
  free(buf);

  scoop_unpin((void *)new_str);
  scoop_unpin((void *)old);
  scoop_unpin((void *)s);

  return result;
}

// `String.charAt(index: Int): Int` — returns the byte value at the given index.
// Out-of-bounds returns -1 (consistent with indexOf returning -1 for not-found).
int64_t scoop_string_char_at(const ScoopString *s, int64_t index) {
  if (s == 0 || s->data == 0 || index < 0 || (uint64_t)index >= s->len) {
    return -1;
  }
  return (int64_t)s->data[index];
}

// `String.repeat(n: Int): String` — repeat the string n times.
const ScoopString *scoop_string_repeat(const ScoopString *s, int64_t n) {
  if (s == 0 || s->len == 0 || s->data == 0 || n <= 0) {
    return scoop_string_empty();
  }
  if (n == 1) {
    return s;
  }

  uint64_t result_len = s->len * (uint64_t)n;
  uint8_t *buf = (uint8_t *)malloc((size_t)result_len);
  if (buf == 0) {
    return s;
  }

  for (int64_t i = 0; i < n; i++) {
    memcpy(buf + ((uint64_t)i * s->len), s->data, (size_t)s->len);
  }

  // Pin input before GC allocation.
  scoop_pin((void *)s);
  const ScoopString *result = scoop_string_from_bytes(buf, result_len);
  free(buf);
  scoop_unpin((void *)s);

  return result;
}

// `String.compareTo(other: String): Int` — lexicographic comparison.
// Returns negative if s < other, 0 if equal, positive if s > other.
int64_t scoop_string_compare_to(const ScoopString *a, const ScoopString *b) {
  if (a == b) {
    return 0;
  }
  if (a == 0) {
    return -1;
  }
  if (b == 0) {
    return 1;
  }
  uint64_t min_len = a->len < b->len ? a->len : b->len;
  if (min_len > 0 && a->data != 0 && b->data != 0) {
    int cmp = memcmp(a->data, b->data, (size_t)min_len);
    if (cmp != 0) {
      return (int64_t)cmp;
    }
  }
  // If prefixes are equal, shorter string is "less".
  if (a->len < b->len) {
    return -1;
  }
  if (a->len > b->len) {
    return 1;
  }
  return 0;
}

// T0121: `String.unsafeSliceBytes(byteOffset, byteLength)` — @Unsafe intrinsic.
// Creates a new String from a byte range of the source String without UTF-8 validation.
// Caller guarantees: offset >= 0, offset + len <= source.byteLength(), range on UTF-8 boundary.
const ScoopString *scoop_string_unsafe_slice_bytes(const ScoopString *source, int64_t byte_offset, int64_t byte_length) {
  if (source == 0 || source->data == 0) {
    return scoop_string_empty();
  }
  if (byte_length <= 0) {
    return scoop_string_empty();
  }
  if (byte_offset < 0) {
    byte_offset = 0;
  }
  int64_t src_len = (int64_t)source->len;
  if (byte_offset >= src_len) {
    return scoop_string_empty();
  }
  if (byte_offset + byte_length > src_len) {
    byte_length = src_len - byte_offset;
  }
  // Pin source — scoop_string_from_bytes calls scoop_alloc which may trigger GC.
  scoop_pin((void *)source);
  const ScoopString *result = scoop_string_from_bytes(source->data + byte_offset, (uint64_t)byte_length);
  scoop_unpin((void *)source);
  return result;
}
