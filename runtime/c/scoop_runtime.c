// Scoop C runtime (early stage).
//
// 这是早期 bootstrap 版本：
// - 先提供最小的“可链接”符号集合
// - 后续会逐步加入：GC、线程注册、effect TLS、pin/unpin 等

#include <stdint.h>

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

