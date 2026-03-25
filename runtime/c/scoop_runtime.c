// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc.h"

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
// - 当前仅作为 codegen/runtime 之间的“可链接 ABI”落点：
//   - 字符串数据视为 UTF-8 字节序列；
//   - `data` 可指向只读静态数据（例如字符串字面量）；
// - 未来会补齐：对象头、GC 跟踪、共享/拷贝策略、完整 String API 等。
typedef struct ScoopString {
  uint64_t len;
  const uint8_t *data;
} ScoopString;

// ABI 断言：保证 codegen 侧对 `ScoopString` 的布局假设稳定。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(uint64_t) == 8, "uint64_t must be 8 bytes");
_Static_assert(offsetof(ScoopString, len) == 0, "ScoopString.len offset must be 0");
_Static_assert(offsetof(ScoopString, data) == 8, "ScoopString.data offset must be 8");
_Static_assert(sizeof(ScoopString) == 16, "ScoopString size must be 16 bytes");
#endif

static const ScoopString SCOOP_EMPTY_STRING = {0, 0};

// 运行时全局状态（early stage）。
//
// 说明：
// - 当前阶段只需要“可被初始化且可观察”，不引入 GC/TLS/线程；
// - 未来会扩展为：线程注册、TLS、effect slots、GC heap 等（TODO T0903/T0904/...）。
static uint32_t scoop_rt_initialized = 0;
static uint32_t scoop_rt_init_calls = 0;

// GC heap（v0：数据结构骨架）。
//
// 说明：
// - 当前阶段仅初始化结构体，不接入 `scoop_alloc`；
// - 后续任务会把 `scoop_alloc` 改为在 heap 中登记对象，并实现 mark-sweep（TODO T0910）。
static ScoopGcHeap scoop_gc_heap;

uint32_t scoop_runtime_is_initialized(void) {
  return scoop_rt_initialized;
}

uint32_t scoop_runtime_init_count(void) {
  return scoop_rt_init_calls;
}

// 每线程 TLS 状态（early stage：占位）。
//
// 说明：
// - 目前只提供“线程是否已注册”的观测与基本清理；
// - 后续会扩展：
//   - GC：`current_frame`（shadow stack 链头，TODO T0905）
//   - effect：handler stack / perform slot / flag（TODO T0906）
typedef struct ScoopThreadTls {
  // 1 表示已注册到 runtime；0 表示未注册。
  uint32_t registered;

  // 保留字段：未来用于版本/flags 等。
  uint32_t _reserved_u32;

  // GC：shadow stack 当前帧链头（TODO T0905）。
  ScoopGcFrame *gc_current_frame;

  // effect runtime（TODO T0906/...）：预留字段（未来用于 handler stack 等）。
  void *_reserved0;
  void *_reserved1;
  void *_reserved2;
} ScoopThreadTls;

static SCOOP_THREAD_LOCAL ScoopThreadTls scoop_tls = {0};

// --- effect runtime v0（TODO T0906） ---
//
// 说明：
// - 本阶段只提供 flag + 单个 perform slot 的 TLS 骨架；不实现 dispatch；
// - codegen/lowering 会在后续任务（T0613+）接入对这些 TLS 符号的读写；
// - 这些符号名用于仓库内部实现/测试，并不承诺稳定 ABI（见 spec 备注）。
typedef union ScoopEffectSlotValue {
  void *as_ptr;
  uint64_t as_u64;
  int64_t as_i64;
} ScoopEffectSlotValue;

typedef struct ScoopEffectPerformSlot {
  // operation tag（由 lowering 写入；当前阶段仅占位）。
  uint32_t op_tag;

  // 保留字段：对齐/扩展。
  uint32_t _reserved_u32;

  // 最小载荷：单 slot（指针/整型）。
  ScoopEffectSlotValue value;
} ScoopEffectPerformSlot;

// flag-based unwinding：每线程 active flag（0=inactive，1=active）。
SCOOP_THREAD_LOCAL uint32_t __scoop_effect_active = 0;

// flag-based unwinding：每线程 perform slot（后续由 `perform` 写入）。
SCOOP_THREAD_LOCAL ScoopEffectPerformSlot __scoop_effect_perform_slot = {0};

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
}

void scoop_thread_unregister(void) {
  if (!scoop_tls.registered) {
    return;
  }

  // 早期阶段：注销时清空 TLS，避免后续测试/手动调试场景出现悬挂状态。
  scoop_tls.registered = 0;
  scoop_tls.gc_current_frame = 0;
  scoop_tls._reserved0 = 0;
  scoop_tls._reserved1 = 0;
  scoop_tls._reserved2 = 0;

  // effect runtime：清空 flag/slot（TODO T0906）。
  __scoop_effect_active = 0;
  (void)memset(&__scoop_effect_perform_slot, 0, sizeof(__scoop_effect_perform_slot));
}

// effect runtime（TODO T0906）：set/clear API（仅用于最小回归与后续 lowering 接入）。
uint32_t scoop_effect_is_active(void) {
  return __scoop_effect_active;
}

void scoop_effect_set_active(void) {
  __scoop_effect_active = 1;
}

void scoop_effect_clear(void) {
  __scoop_effect_active = 0;
  (void)memset(&__scoop_effect_perform_slot, 0, sizeof(__scoop_effect_perform_slot));
}

// --- GC / shadow stack（TODO T0905） ---

ScoopGcFrame *scoop_gc_current_frame(void) {
  return scoop_tls.gc_current_frame;
}

void scoop_gc_frame_push(ScoopGcFrame *frame) {
  if (frame == 0) {
    return;
  }

  // 在早期阶段尽量保持接口易用：允许在未显式 init/register 的情况下被调用。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  frame->prev = scoop_tls.gc_current_frame;
  scoop_tls.gc_current_frame = frame;
}

void scoop_gc_frame_pop(ScoopGcFrame *frame) {
  if (frame == 0) {
    return;
  }

  // 健壮性：pop 必须匹配最近一次 push；否则保持状态不变并在 debug 下输出日志。
  if (scoop_tls.gc_current_frame != frame) {
    SCOOP_RT_LOG("scoop_gc_frame_pop: mismatch (current=%p, frame=%p)",
                 (void *)scoop_tls.gc_current_frame,
                 (void *)frame);
    return;
  }

  scoop_tls.gc_current_frame = frame->prev;
  frame->prev = 0;
}

uint64_t scoop_gc_debug_count_roots_current_thread(void) {
  // 说明：
  // - 该函数用于“伪 GC 扫描”回归（TODO T0816），只做 shadow stack 遍历；
  // - 为了避免意外的未初始化使用，这里保持与 push 一致：允许在未显式 init/register
  //   的情况下被调用。
  if (!scoop_tls.registered) {
    scoop_thread_register();
  }

  uint64_t count = 0;
  ScoopGcFrame *frame = scoop_tls.gc_current_frame;

  // 健壮性：若 frame 链被破坏（形成环/或 root_count 异常），这里做保守上限避免死循环
  // 或越界访问导致崩溃；debug 日志可用于手动排查插桩问题。
  uint32_t frame_steps = 0;
  const uint32_t max_frames = 1024u * 1024u;
  const uint32_t max_roots_per_frame = 1024u * 1024u;

  while (frame != 0) {
    if (frame_steps++ > max_frames) {
      SCOOP_RT_LOG("scoop_gc_debug_count_roots_current_thread: too many frames, abort scan");
      break;
    }

    uint32_t n = frame->root_count;
    if (n > max_roots_per_frame) {
      SCOOP_RT_LOG("scoop_gc_debug_count_roots_current_thread: suspicious root_count=%" PRIu32,
                   n);
      n = max_roots_per_frame;
    }

    for (uint32_t i = 0; i < n; i++) {
      if (frame->roots[i] != 0) {
        count++;
      }
    }

    frame = frame->prev;
  }

  return count;
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
// - 输出通过 `malloc` 分配（暂不接入 GC / `scoop_alloc`；TODO T0902/T0817）；
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
    return &SCOOP_EMPTY_STRING;
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

  ScoopString *out_str = (ScoopString *)malloc(sizeof(ScoopString));
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
  // 说明：当前阶段允许被重复调用（避免在多入口/测试场景下因重复 init 直接崩溃）。
  // 线程安全的 once 初始化将在引入线程注册后再补齐（TODO T0903/T0911）。
  if (scoop_rt_initialized) {
    scoop_rt_init_calls++;
    SCOOP_RT_LOG("scoop_runtime_init: already initialized (calls=%" PRIu32 ")",
                 scoop_rt_init_calls);
    return;
  }

  scoop_rt_initialized = 1;
  scoop_rt_init_calls = 1;

  scoop_gc_heap_init(&scoop_gc_heap);

  SCOOP_RT_LOG("scoop_runtime_init: ok (ScoopString size=%zu, data_off=%zu)",
               sizeof(ScoopString),
               offsetof(ScoopString, data));
}

// 最小占位分配 API（后续替换为真正 GC 分配）
void *scoop_alloc(uint64_t size) {
  // 说明（early stage）：
  // - 当前以 libc `malloc` 作为最小可用实现，保证 codegen 侧能稳定拿到非空指针；
  // - 暂不做对象头/类型信息写入（由后续 codegen + GC 任务补齐）；
  // - OOM 时返回 NULL，由上层决定如何处理（未来可映射到 Raise<RuntimeError>）。
  if (size == 0) {
    // `malloc(0)` 的返回值在不同实现上可能为 NULL 或唯一指针；为保持可预期，这里统一分配 1 字节。
    size = 1;
  }
  if (size > (uint64_t)SIZE_MAX) {
    return 0;
  }

  void *p = malloc((size_t)size);
  if (p == 0) {
    SCOOP_RT_LOG("scoop_alloc: oom (size=%" PRIu64 ")", size);
  }
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

  // `fwrite` 返回值目前不做错误处理；后续可改为向 `Raise<RuntimeError>` 报错。
  (void)fwrite(value->data, 1, (size_t)value->len, stdout);
}

// 打印并追加换行。
//
// 说明：该 API 由 sysroot 的 `println(String)` 映射到 runtime 符号（见 TODO T0821/T0822）。
void scoop_println(const ScoopString *value) {
  scoop_print(value);
  (void)fputc('\n', stdout);
  (void)fflush(stdout);
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
