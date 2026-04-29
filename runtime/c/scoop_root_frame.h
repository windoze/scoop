// Scoop explicit root frame ABI helpers (internal for now).
//
// 目的（T5001c1）：
// - 固化 explicit root frame descriptor/header 的基础布局；
// - 提供按 `header -> desc -> offsets` 恢复 `void** slot` 的最小 helper；
// - 暂不在本阶段切换默认 roots 枚举路径，后续由 `T5001c2` 接入 GC runtime。

#ifndef SCOOP_ROOT_FRAME_H
#define SCOOP_ROOT_FRAME_H

#include <stddef.h>
#include <stdint.h>

#include "scoop_gc.h"
#include "scoop_tls_internal.h"

typedef struct ScoopRootFrameDesc {
  uint32_t slot_count;
  const uint32_t *slot_offsets;
} ScoopRootFrameDesc;

typedef struct ScoopRootFrameHeader {
  struct ScoopRootFrameHeader *prev;
  const ScoopRootFrameDesc *desc;
} ScoopRootFrameHeader;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopRootFrameHeader, prev) == 0,
               "ScoopRootFrameHeader.prev offset must be 0");
_Static_assert(offsetof(ScoopRootFrameHeader, desc) == sizeof(void *),
               "ScoopRootFrameHeader.desc offset must be sizeof(void*)");
_Static_assert((sizeof(ScoopRootFrameHeader) % sizeof(void *)) == 0,
               "ScoopRootFrameHeader size must be pointer-aligned");
#endif

// 每线程 explicit frame chain 栈顶。
//
// 约定：
// - `hdr` 必须是 frame object 的首字段；
// - 因此 `ScoopRootFrameHeader*` 同时也是 frame base，可直接与 descriptor offsets 组合。
extern SCOOP_THREAD_LOCAL ScoopRootFrameHeader *__scoop_explicit_root_frame_top;

typedef enum ScoopRootFrameVisitError {
  SCOOP_ROOT_FRAME_VISIT_OK = 0,
  SCOOP_ROOT_FRAME_VISIT_ERR_INVALID_ARGUMENT = 1,
  SCOOP_ROOT_FRAME_VISIT_ERR_MISSING_DESC = 2,
  SCOOP_ROOT_FRAME_VISIT_ERR_MISSING_SLOT_OFFSETS = 3,
} ScoopRootFrameVisitError;

typedef struct ScoopRootFrameVisitResult {
  uint64_t slots_visited;
  uint32_t frames_visited;
  uint32_t visit_error;
} ScoopRootFrameVisitResult;

static inline void scoop_root_frame_visit_result_reset(ScoopRootFrameVisitResult *out) {
  if (out == 0) {
    return;
  }
  out->slots_visited = 0;
  out->frames_visited = 0;
  out->visit_error = SCOOP_ROOT_FRAME_VISIT_OK;
}

static inline void **scoop_root_frame_slot_at_offset(ScoopRootFrameHeader *frame,
                                                     uint32_t slot_offset) {
  if (frame == 0) {
    return 0;
  }
  return (void **)((uint8_t *)frame + (uintptr_t)slot_offset);
}

// 沿 explicit frame chain 逐帧枚举 roots slots。
//
// 说明：
// - `top == NULL` 是合法的“当前线程无 explicit frame”状态，返回 0 且不报错；
// - `slot_count == 0` 的 frame 也是合法输入：记为 1 个 visited frame，但不会访问任何 slot；
// - 若 frame 存在但缺少 `desc`，或 `slot_count > 0` 却缺少 `slot_offsets`，视为 runtime contract 违规。
static inline uint64_t scoop_root_frame_visit_slots(ScoopRootFrameHeader *top,
                                                    ScoopGcTraceVisitor visitor,
                                                    void *ctx,
                                                    ScoopRootFrameVisitResult *out_result) {
  scoop_root_frame_visit_result_reset(out_result);

  if (visitor == 0) {
    if (out_result != 0) {
      out_result->visit_error = SCOOP_ROOT_FRAME_VISIT_ERR_INVALID_ARGUMENT;
    }
    return 0;
  }

  uint64_t slots_visited = 0;
  uint32_t frames_visited = 0;

  for (ScoopRootFrameHeader *frame = top; frame != 0; frame = frame->prev) {
    frames_visited += 1;

    const ScoopRootFrameDesc *desc = frame->desc;
    if (desc == 0) {
      if (out_result != 0) {
        out_result->slots_visited = slots_visited;
        out_result->frames_visited = frames_visited;
        out_result->visit_error = SCOOP_ROOT_FRAME_VISIT_ERR_MISSING_DESC;
      }
      return slots_visited;
    }

    if (desc->slot_count == 0) {
      continue;
    }

    if (desc->slot_offsets == 0) {
      if (out_result != 0) {
        out_result->slots_visited = slots_visited;
        out_result->frames_visited = frames_visited;
        out_result->visit_error = SCOOP_ROOT_FRAME_VISIT_ERR_MISSING_SLOT_OFFSETS;
      }
      return slots_visited;
    }

    for (uint32_t i = 0; i < desc->slot_count; i++) {
      void **slot = scoop_root_frame_slot_at_offset(frame, desc->slot_offsets[i]);
      visitor(slot, ctx);
      slots_visited += 1;
    }
  }

  if (out_result != 0) {
    out_result->slots_visited = slots_visited;
    out_result->frames_visited = frames_visited;
    out_result->visit_error = SCOOP_ROOT_FRAME_VISIT_OK;
  }
  return slots_visited;
}

#endif // SCOOP_ROOT_FRAME_H
