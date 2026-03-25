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

uint32_t scoop_runtime_is_initialized(void) {
  return scoop_rt_initialized;
}

uint32_t scoop_runtime_init_count(void) {
  return scoop_rt_init_calls;
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

  SCOOP_RT_LOG("scoop_runtime_init: ok (ScoopString size=%zu, data_off=%zu)",
               sizeof(ScoopString),
               offsetof(ScoopString, data));
}

// 最小占位分配 API（后续替换为真正 GC 分配）
void *scoop_alloc(uint64_t size) {
  // TODO: 使用 GC 分配器
  (void)size;
  return 0;
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
