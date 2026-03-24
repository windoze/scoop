// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>
#include <stddef.h>
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
