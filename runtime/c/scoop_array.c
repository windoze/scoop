// Scoop C runtime: Array / MutableArray primitive support (early stage).
//
// 目标（TODO T1317d）：
// - 为 `Array<T>` / `MutableArray<T>` 提供最小运行期 primitive：
//   - 分配（通过 array literal builder）
//   - 长度（len/size）
//   - 索引读写（get/set）
// - 让 LLVM codegen 可以在不引入 stdlib 语义的情况下生成可执行代码。
//
// 说明：
// - 当前实现以 “word array” 作为元素承载：每个元素都是一个 `uint64_t`：
//   - 整数/布尔：按 u64 编码；
//   - 引用/字符串指针：按 ptr→u64 编码；
// - 该 ABI 与编译器侧的 `coerce_u64_word()` 对齐，便于后续扩展为更复杂的 payload（TODO T0630）。
//
// 注意（early stage 限制）：
// - 本实现未接入 type descriptor，因此 GC 不会扫描数组元素中的指针（`type_desc == 0`）。
//   在没有显式 `__scoop_gc_collect()` 的 run-pass fixtures 场景下足够；更完整的指针追踪留给
//   后续 “typed alloc + type descriptor trace” 任务补齐（TODO T0907+）。

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc.h"

// `scoop_alloc` 在 `scoop_runtime.c` 中实现；这里仅声明供本模块使用。
void *scoop_alloc(uint64_t size);

// --- Array object layout ---
//
// 运行期对象布局：
// `{ header: ScoopGcObjectHeader, len: u64, data: [len x u64] }`
typedef struct ScoopArray {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t data[];
} ScoopArray;

// --- ArrayBuilder layout ---
//
// builder 是 lowering 产物（见 `hir/lower.rs` 的 array literal lowering）：
// - `__scoop_array_builder_new()` 创建 builder
// - `__scoop_array_builder_push(builder, value)` 逐个 push
// - `__scoop_array_builder_build_array(builder)` 产出 Array
// - `__scoop_array_builder_build_mutable_array(builder)` 产出 MutableArray
//
// 为保持实现简单：
// - builder 自身使用 `scoop_alloc` 作为 GC-managed 对象（避免 GC roots 指向非 GC 对象导致崩溃）；
// - builder 的临时 buffer 使用 `malloc/realloc`（builder 被 build 后会 `free` 该 buffer）。
typedef struct ScoopArrayBuilder {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t cap;
  uint64_t *data;
} ScoopArrayBuilder;

static uint32_t scoop_array_builder_grow(ScoopArrayBuilder *b) {
  if (b == 0) {
    return 0;
  }

  uint64_t old_cap = b->cap;
  uint64_t new_cap = (old_cap == 0) ? 4u : old_cap * 2u;
  if (new_cap < old_cap) {
    // overflow
    return 0;
  }

  // size_t overflow guard（避免 `realloc` 参数回卷）。
  uint64_t max_cap = (uint64_t)(SIZE_MAX / sizeof(uint64_t));
  if (new_cap > max_cap) {
    return 0;
  }

  size_t bytes = (size_t)new_cap * sizeof(uint64_t);
  uint64_t *p = (uint64_t *)realloc(b->data, bytes);
  if (p == 0) {
    return 0;
  }

  b->data = p;
  b->cap = new_cap;
  return 1;
}

void *scoop_array_builder_new(void) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)scoop_alloc((uint64_t)sizeof(ScoopArrayBuilder));
  if (b == 0) {
    return 0;
  }

  // `scoop_alloc` 已初始化对象头（size/mark 等）；这里补齐 builder 字段。
  b->len = 0;
  b->cap = 0;
  b->data = 0;
  return (void *)b;
}

void scoop_array_builder_push_u64(void *builder, uint64_t value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (b == 0) {
    return;
  }

  if (b->len >= b->cap) {
    if (!scoop_array_builder_grow(b)) {
      // early stage：OOM/overflow 直接忽略（保持运行时可链接/不崩溃）。
      return;
    }
  }

  b->data[b->len] = value;
  b->len += 1;
}

static void *scoop_array_builder_build_common(ScoopArrayBuilder *b) {
  if (b == 0) {
    return 0;
  }

  uint64_t len = b->len;
  uint64_t bytes = (uint64_t)sizeof(ScoopArray);
  if (len > 0) {
    uint64_t add = len * (uint64_t)sizeof(uint64_t);
    if (add / (uint64_t)sizeof(uint64_t) != len) {
      // overflow
      return 0;
    }
    if (UINT64_MAX - bytes < add) {
      // overflow
      return 0;
    }
    bytes += add;
  }

  ScoopArray *arr = (ScoopArray *)scoop_alloc(bytes);
  if (arr == 0) {
    return 0;
  }

  arr->len = len;
  if (len > 0 && b->data != 0) {
    // `bytes` 已被 overflow guard 保护，因此这里的 size_t 转换是安全的。
    (void)memcpy(arr->data, b->data, (size_t)len * sizeof(uint64_t));
  }

  // 释放临时 buffer，避免在大量 array literal 下泄漏。
  if (b->data != 0) {
    free(b->data);
    b->data = 0;
  }
  b->len = 0;
  b->cap = 0;

  return (void *)arr;
}

void *scoop_array_builder_build_array(void *builder) {
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder);
}

void *scoop_array_builder_build_mutable_array(void *builder) {
  // early stage：MutableArray 与 Array 共享同一底层表示（固定长度 word buffer）。
  // 更完整的容量策略由 stdlib 在上层实现（TODO T1317e）。
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder);
}

uint64_t scoop_array_len(void *array_obj) {
  if (array_obj == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  return arr->len;
}

uint64_t scoop_array_get_u64(void *array_obj, int64_t index) {
  if (array_obj == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (index < 0) {
    return 0;
  }

  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return 0;
  }

  return arr->data[idx];
}

void scoop_array_set_u64(void *array_obj, int64_t index, uint64_t value) {
  if (array_obj == 0) {
    return;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (index < 0) {
    return;
  }

  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return;
  }

  arr->data[idx] = value;
}
