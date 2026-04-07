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
// - 当前实现以 “word array” 作为元素承载：每个元素都是一个 “word-sized slot”（`uintptr_t`）：
//   - 整数/布尔：按 u64 bits 编码后写入 slot；
//   - 引用/字符串指针：按 ptr→uintptr_t bits 编码后写入 slot；
// - 该 ABI 与编译器侧的 `coerce_u64_word()` 对齐，便于后续扩展为更复杂的 payload（TODO T0630）。
//
// 注意（early stage 限制）：
// - Array/ArrayBuilder 以 “word slots（uintptr_t）” 承载元素，既可存放整数 bits，也可存放 GC 指针；
// - 为保证 `Array<Ref>` / `Array<String>` 等场景下 GC 可追踪元素，本文件会在构造数组时
//   把 `header.type_desc` 设为带 trace_fn 的 descriptor，并在 GC trace 中把每个 slot 当作 `void**` 访问。
// - 对于 “word array”（例如 `Array<Int>` / `Array<Bool>`），type_desc 仍可为 NULL 或指向 no-trace
//   descriptor（不会扫描元素，避免把整数误当作指针）。

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
// `{ header: ScoopGcObjectHeader, len: u64, data: [len x uintptr_t] }`
typedef struct ScoopArray {
  ScoopGcObjectHeader header;
  uint64_t len;
  uintptr_t data[];
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
  // 元素承载种类（用于选择 array/build 的 type descriptor）：
  // - 0: unknown（尚未 push）
  // - 1: word（push_u64）
  // - 2: ref（push_ref）
  uint32_t elem_kind;
  uint32_t _reserved_u32;
  uintptr_t *data;
} ScoopArrayBuilder;

#define SCOOP_ARRAY_ELEM_KIND_UNKNOWN 0u
#define SCOOP_ARRAY_ELEM_KIND_WORD 1u
#define SCOOP_ARRAY_ELEM_KIND_REF 2u

static uint64_t scoop_array_trace_ref_elems(void *object, ScoopGcTraceVisitor visitor, void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)object;

  // 健壮性：根据 header.size_bytes 裁剪扫描范围，避免 len 被污染时越界。
  uint64_t size_bytes = arr->header.size_bytes;
  uint64_t data_off = (uint64_t)offsetof(ScoopArray, data);
  if (size_bytes <= data_off) {
    return 0;
  }

  uint64_t avail = size_bytes - data_off;
  uint64_t max_len = avail / (uint64_t)sizeof(uintptr_t);
  uint64_t len = arr->len;
  if (len > max_len) {
    len = max_len;
  }

  uint64_t visited = 0;
  for (uint64_t i = 0; i < len; i++) {
    void **slot = (void **)&arr->data[i];
    visitor(slot, ctx);
    visited += 1;
  }
  return visited;
}

static uint64_t scoop_array_builder_trace_ref_elems(void *object,
                                                    ScoopGcTraceVisitor visitor,
                                                    void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopArrayBuilder *b = (ScoopArrayBuilder *)object;
  if (b->elem_kind != SCOOP_ARRAY_ELEM_KIND_REF) {
    return 0;
  }
  if (b->data == 0 || b->len == 0) {
    return 0;
  }

  uint64_t len = b->len;
  if (b->cap > 0 && len > b->cap) {
    len = b->cap;
  }

  uint64_t visited = 0;
  for (uint64_t i = 0; i < len; i++) {
    void **slot = (void **)&b->data[i];
    visitor(slot, ctx);
    visited += 1;
  }
  return visited;
}

static void scoop_array_builder_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopArrayBuilder *b = (ScoopArrayBuilder *)object;
  if (b->data != 0) {
    free(b->data);
    b->data = 0;
  }
  b->len = 0;
  b->cap = 0;
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
}

static const ScoopTypeDescriptor SCOOP_ARRAY_WORD_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    // Array 是变长对象：这里记录最小 header size 作为调试信息；真实大小由 `hdr.size_bytes` 决定。
    .size_bytes = sizeof(ScoopArray),
    .align_bytes = (uint64_t)_Alignof(ScoopArray),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = 0,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static const ScoopTypeDescriptor SCOOP_ARRAY_REF_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopArray),
    .align_bytes = (uint64_t)_Alignof(ScoopArray),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_array_trace_ref_elems,
    .release_fn = 0,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static const ScoopTypeDescriptor SCOOP_ARRAY_BUILDER_WORD_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopArrayBuilder),
    .align_bytes = (uint64_t)_Alignof(ScoopArrayBuilder),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_array_builder_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static const ScoopTypeDescriptor SCOOP_ARRAY_BUILDER_REF_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopArrayBuilder),
    .align_bytes = (uint64_t)_Alignof(ScoopArrayBuilder),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_array_builder_trace_ref_elems,
    .release_fn = scoop_array_builder_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

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
  uint64_t max_cap = (uint64_t)(SIZE_MAX / sizeof(uintptr_t));
  if (new_cap > max_cap) {
    return 0;
  }

  size_t bytes = (size_t)new_cap * sizeof(uintptr_t);
  uintptr_t *p = (uintptr_t *)realloc(b->data, bytes);
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
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
  b->_reserved_u32 = 0;
  b->data = 0;
  return (void *)b;
}

void scoop_array_builder_push_u64(void *builder, uint64_t value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (b == 0) {
    return;
  }

  if (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_UNKNOWN) {
    b->elem_kind = SCOOP_ARRAY_ELEM_KIND_WORD;
    b->header.type_desc = &SCOOP_ARRAY_BUILDER_WORD_TYPE_DESC;
  }
  if (b->elem_kind != SCOOP_ARRAY_ELEM_KIND_WORD) {
    // 不允许混用 push_u64/push_ref：视为编译器/stdlib 约定错误，保持运行时不崩溃即可。
    return;
  }

  if (b->len >= b->cap) {
    if (!scoop_array_builder_grow(b)) {
      // early stage：OOM/overflow 直接忽略（保持运行时可链接/不崩溃）。
      return;
    }
  }

  b->data[b->len] = (uintptr_t)value;
  b->len += 1;
}

void scoop_array_builder_push_ref(void *builder, void *value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (b == 0) {
    return;
  }

  if (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_UNKNOWN) {
    b->elem_kind = SCOOP_ARRAY_ELEM_KIND_REF;
    b->header.type_desc = &SCOOP_ARRAY_BUILDER_REF_TYPE_DESC;
  }
  if (b->elem_kind != SCOOP_ARRAY_ELEM_KIND_REF) {
    // 不允许混用 push_u64/push_ref：视为编译器/stdlib 约定错误，保持运行时不崩溃即可。
    return;
  }

  if (b->len >= b->cap) {
    if (!scoop_array_builder_grow(b)) {
      // early stage：OOM/overflow 直接忽略（保持运行时可链接/不崩溃）。
      return;
    }
  }

  b->data[b->len] = (uintptr_t)value;
  b->len += 1;
}

static void *scoop_array_builder_build_common(ScoopArrayBuilder *b) {
  if (b == 0) {
    return 0;
  }

  uint64_t len = b->len;
  uint64_t bytes = (uint64_t)sizeof(ScoopArray);
  if (len > 0) {
    uint64_t add = len * (uint64_t)sizeof(uintptr_t);
    if (add / (uint64_t)sizeof(uintptr_t) != len) {
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

  // 关键：写入 array object 的 type descriptor。
  // - ref 数组：GC 会扫描每个 slot 并追踪其中的指针；
  // - word 数组：不扫描（避免把整数误当作指针）。
  arr->header.type_desc =
      (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) ? &SCOOP_ARRAY_REF_TYPE_DESC : &SCOOP_ARRAY_WORD_TYPE_DESC;

  arr->len = len;
  if (len > 0 && b->data != 0) {
    // `bytes` 已被 overflow guard 保护，因此这里的 size_t 转换是安全的。
    (void)memcpy(arr->data, b->data, (size_t)len * sizeof(uintptr_t));
  }

  // 释放临时 buffer，避免在大量 array literal 下泄漏。
  if (b->data != 0) {
    free(b->data);
    b->data = 0;
  }
  b->len = 0;
  b->cap = 0;
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;

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

  return (uint64_t)arr->data[idx];
}

void *scoop_array_get_ref(void *array_obj, int64_t index) {
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

  return (void *)arr->data[idx];
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

  arr->data[idx] = (uintptr_t)value;
}

void scoop_array_set_ref(void *array_obj, int64_t index, void *value) {
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

  // T1412d：引用写入必须走统一写屏障，避免在 Immix nursery 模式下形成 old→nursery 指针。
  // 注意：Array 的元素槽位为 `uintptr_t`（word slots），因此这里传入 “slot 的地址” 供 barrier 用 memcpy 写回。
  (void)scoop_gc_write_barrier((void *)&arr->data[idx], value);
}
