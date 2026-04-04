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

#include "scoop_gc.h"

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

// Stackmap record locations → roots slots visit errors（TODO T1506a）.
//
// 约定：
// - 0 表示成功；
// - 非 0 表示遇到“无法转换为可更新 `void** slot`”的 location 或 record 结构错误；
// - 错误码应保持稳定（便于测试/诊断回归）。
typedef enum ScoopStackmapVisitError {
  SCOOP_STACKMAP_VISIT_OK = 0,
  SCOOP_STACKMAP_VISIT_ERR_INVALID_ARGUMENT = 1,
  SCOOP_STACKMAP_VISIT_ERR_RECORD_TOO_SHORT = 2,
  SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED = 3,
  SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_LOCATION = 4,
  SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_DWARF_REG = 5,
  SCOOP_STACKMAP_VISIT_ERR_UNALIGNED_SLOT = 6,
} ScoopStackmapVisitError;

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

// 遍历 stackmap record 的 locations，并把可转换为“可更新 roots slot”的条目以 `void** slot`
// 的形式交给 visitor。
//
// 当前阶段（T1506a）的最小约束：
// - 只处理 pointer-sized locations（`location_size == sizeof(void*)`）；其它 size 先跳过；
// - 只支持以 SP（CFA）为基址的 Direct/Indirect locations；
// - 对于非 Direct/Indirect 的 pointer-sized location（例如 Register/Constant），v0 选择忽略：
//   - 这些条目可能来自 statepoint/patchpoint 的 deopt/metadata，并不一定是 GC roots；
//   - 当前实现无法对寄存器做写回更新（moving GC 需要 `void** slot`），因此不应 fail-fast。
//
// 参数：
// - `frame_sp`：该帧的 SP/CFA（由 platform/unwind 提供，作为 stackmap location 基址）。
// - `frame_fp`：该帧的 FP（若平台层可提供；用于处理以 FP 为基址的 locations；为 0 表示未知/不可用）。
//
// 返回：
// - visitor 调用次数（即扫描到的 non-null roots slot 个数）。
// - 若 `out_error` 非空：成功写入 `SCOOP_STACKMAP_VISIT_OK`，失败写入错误码。
uint64_t scoop_stackmap_record_visit_root_slots(const ScoopStackmapRecordRef *rec,
                                                uintptr_t frame_sp,
                                                uintptr_t frame_fp,
                                                ScoopGcTraceVisitor visitor,
                                                void *ctx,
                                                uint32_t *out_error);

// 清空 registry（主要用于测试；也允许在早期 bootstrap 阶段重复 init）。
void scoop_stackmap_registry_reset(void);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // SCOOP_STACKMAP_H
