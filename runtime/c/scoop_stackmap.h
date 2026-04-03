// LLVM StackMap registry (runtime-side).
//
// 说明：
// - 本模块负责在运行期解析并索引 LLVM StackMap section（`.llvm_stackmaps` / `__llvm_stackmaps`）。
// - 目标是为后续 “基于 statepoint/stackmap 的精确 roots 枚举（T1505/T1506）” 提供底座：
//   - module 注册（主程序 + 其它已链接模块）
//   - `return_address -> record` 的查询（排序数组 + 二分查找）
// - 早期阶段（T1504）我们只要求做到：
//   - 能解析 records，并建立 `return_address` 索引；
//   - 能在 `scoop_runtime_init()` 时尽力从当前进程中找到 stackmap section 并注册（找不到则不失败）。

#ifndef SCOOP_STACKMAP_H
#define SCOOP_STACKMAP_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// 一个最小的 StackMap record 引用（view）。
//
// 注意：
// - `record_ptr` 指向的是“被注册的 stackmap section 内存”，其生命周期由调用方保证：
//   - 对于 `register_current_process()`：指向进程镜像中的只读 section，生命周期等同进程；
//   - 对于 `register_section(bytes,len)`：调用方必须保证 bytes 在 registry 生命周期内有效（通常为 static）。
typedef struct ScoopStackmapRecordRef {
  uintptr_t return_address;
  uintptr_t function_address;
  uint32_t instruction_offset;
  uint64_t patchpoint_id;

  const uint8_t *record_ptr;
  uint32_t record_size;
} ScoopStackmapRecordRef;

// 注册一段 stackmap section（内存中的 bytes）。
//
// 返回：成功注册的 records 数量（解析失败或无 records 时返回 0）。
uint32_t scoop_stackmap_registry_register_section(const uint8_t *bytes, size_t len);

// 从当前进程镜像中定位并注册 stackmap section。
//
// 返回：成功注册的 records 数量（未找到/平台不支持时返回 0；不会导致 runtime 初始化失败）。
uint32_t scoop_stackmap_registry_register_current_process(void);

// 返回当前 registry 的 records 数量。
uint32_t scoop_stackmap_registry_record_count(void);

// 按 return address 查询 record。
//
// 返回：1 表示找到并写入 out；0 表示未找到（out 不变）。
uint32_t scoop_stackmap_registry_lookup(uintptr_t return_address, ScoopStackmapRecordRef *out);

// 清空 registry（主要用于测试；也允许在早期 bootstrap 阶段重复 init）。
void scoop_stackmap_registry_reset(void);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // SCOOP_STACKMAP_H

