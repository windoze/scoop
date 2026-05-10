// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
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
