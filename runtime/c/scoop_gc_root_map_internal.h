// Scoop GC managed root map helpers (internal, header-only).
//
// 目的（T5001b）：
// - 把 runtime 上层看到的“managed frame roots 来源”统一收口为 `void** slot` visitor；
// - 让 GC/verify/update 只依赖 root-map 抽象，而不是直接嵌入 stackmap 解析细节；
// - 为后续 explicit root frame 保留与 stackmap 并列的实现边界。

#ifndef SCOOP_GC_ROOT_MAP_INTERNAL_H
#define SCOOP_GC_ROOT_MAP_INTERNAL_H

#include <stdint.h>

#include "platform/unwind.h"
#include "scoop_gc.h"
#include "scoop_stackmap.h"

typedef enum ScoopGcManagedRootMapKind {
  SCOOP_GC_MANAGED_ROOT_MAP_NONE = 0,
  SCOOP_GC_MANAGED_ROOT_MAP_STACKMAP = 1,
  SCOOP_GC_MANAGED_ROOT_MAP_EXPLICIT_FRAME = 2,
} ScoopGcManagedRootMapKind;

typedef struct ScoopGcManagedRootMap {
  ScoopGcManagedRootMapKind kind;
  union {
    void *stack_walking_ctx;
    void *explicit_root_frame_top;
  } source;
} ScoopGcManagedRootMap;

typedef enum ScoopGcRootMapVisitError {
  SCOOP_GC_ROOT_MAP_VISIT_OK = 0,
  SCOOP_GC_ROOT_MAP_VISIT_ERR_INVALID_ARGUMENT = 1,
  SCOOP_GC_ROOT_MAP_VISIT_ERR_UNSUPPORTED_KIND = 1024,
} ScoopGcRootMapVisitError;

typedef struct ScoopGcRootMapVisitResult {
  uint64_t slots_visited;
  // 当前 stackmap root map 下表示命中的 records 数；后续 explicit-frame root map 下将表示命中的 frame 数。
  uint32_t units_hit;
  uint32_t visit_error;
} ScoopGcRootMapVisitResult;

typedef struct ScoopGcStackmapRootMapWalkCtx {
  ScoopGcTraceVisitor visitor;
  void *visitor_ctx;
  ScoopGcRootMapVisitResult result;
} ScoopGcStackmapRootMapWalkCtx;

static inline ScoopGcManagedRootMap scoop_gc_managed_root_map_none(void) {
  ScoopGcManagedRootMap map = {0};
  map.kind = SCOOP_GC_MANAGED_ROOT_MAP_NONE;
  return map;
}

static inline ScoopGcManagedRootMap scoop_gc_managed_root_map_from_stackmap_ctx(
    void *stack_walking_ctx) {
  ScoopGcManagedRootMap map = {0};
  map.kind = SCOOP_GC_MANAGED_ROOT_MAP_STACKMAP;
  map.source.stack_walking_ctx = stack_walking_ctx;
  return map;
}

static inline void scoop_gc_root_map_visit_result_reset(ScoopGcRootMapVisitResult *out) {
  if (out == 0) {
    return;
  }
  out->slots_visited = 0;
  out->units_hit = 0;
  out->visit_error = SCOOP_GC_ROOT_MAP_VISIT_OK;
}

static inline uint32_t scoop_gc_stackmap_root_map_frame_visitor(uintptr_t sp,
                                                                uintptr_t ra,
                                                                uintptr_t fp,
                                                                void *user_data) {
  if (user_data == 0) {
    return 0;
  }

  ScoopGcStackmapRootMapWalkCtx *ctx = (ScoopGcStackmapRootMapWalkCtx *)user_data;
  if (ctx->result.visit_error != SCOOP_GC_ROOT_MAP_VISIT_OK) {
    return 0;
  }

  ScoopStackmapRecordRef rec = {0};
  if (!scoop_stackmap_registry_lookup(ra, &rec)) {
    // 始终继续 walk 非 managed 帧，确保调用链上更高层 managed 帧仍可被枚举。
    return 1;
  }

  ctx->result.units_hit += 1;

  uint32_t visit_err = SCOOP_STACKMAP_VISIT_OK;
  ctx->result.slots_visited +=
      scoop_stackmap_record_visit_root_slots(&rec, sp, fp, ctx->visitor, ctx->visitor_ctx, &visit_err);

  if (visit_err != SCOOP_STACKMAP_VISIT_OK) {
    ctx->result.visit_error = visit_err;
    return 0;
  }

  return 1;
}

static inline uint64_t scoop_gc_stackmap_root_map_visit_slots(
    void *stack_walking_ctx,
    ScoopGcTraceVisitor visitor,
    void *ctx,
    ScoopGcRootMapVisitResult *out_result) {
  scoop_gc_root_map_visit_result_reset(out_result);

  if (stack_walking_ctx == 0 || visitor == 0) {
    if (out_result != 0) {
      out_result->visit_error = SCOOP_GC_ROOT_MAP_VISIT_ERR_INVALID_ARGUMENT;
    }
    return 0;
  }

  ScoopGcStackmapRootMapWalkCtx walk = {
      .visitor = visitor,
      .visitor_ctx = ctx,
      .result = {
          .slots_visited = 0,
          .units_hit = 0,
          .visit_error = SCOOP_GC_ROOT_MAP_VISIT_OK,
      },
  };

  const uint32_t skip_frames = 0;
  (void)scoop_platform_unwind_ctx_walk_frames(
      stack_walking_ctx, skip_frames, scoop_gc_stackmap_root_map_frame_visitor, (void *)&walk);

  if (out_result != 0) {
    *out_result = walk.result;
  }
  return walk.result.slots_visited;
}

static inline uint64_t scoop_gc_root_map_visit_slots(const ScoopGcManagedRootMap *root_map,
                                                     ScoopGcTraceVisitor visitor,
                                                     void *ctx,
                                                     ScoopGcRootMapVisitResult *out_result) {
  scoop_gc_root_map_visit_result_reset(out_result);

  if (root_map == 0 || visitor == 0) {
    if (out_result != 0) {
      out_result->visit_error = SCOOP_GC_ROOT_MAP_VISIT_ERR_INVALID_ARGUMENT;
    }
    return 0;
  }

  switch (root_map->kind) {
  case SCOOP_GC_MANAGED_ROOT_MAP_NONE:
    return 0;
  case SCOOP_GC_MANAGED_ROOT_MAP_STACKMAP:
    return scoop_gc_stackmap_root_map_visit_slots(
        root_map->source.stack_walking_ctx, visitor, ctx, out_result);
  case SCOOP_GC_MANAGED_ROOT_MAP_EXPLICIT_FRAME:
  default:
    if (out_result != 0) {
      out_result->visit_error = SCOOP_GC_ROOT_MAP_VISIT_ERR_UNSUPPORTED_KIND;
    }
    return 0;
  }
}

#endif
