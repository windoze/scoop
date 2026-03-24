// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
#include <inttypes.h>
#include <stdio.h>

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

// 运行时初始化（后续可由编译器生成的 main 调用）
void scoop_runtime_init(void) {
  // TODO: 初始化 GC / TLS / 线程注册
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
