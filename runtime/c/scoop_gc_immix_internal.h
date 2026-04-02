// Internal helpers for Immix GC backend (v0, single-thread, non-moving).
//
// 注意：
// - 该头文件为 runtime/c 内部使用，不属于对外 ABI；
// - 仅在 `SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX` 时生效；
// - 设计目标对应 TODO T1406b：先落地 block/line allocator 的最小元数据与分配路径，
//   mark-region/sweep 的完整策略在 T1406c 推进。

#pragma once

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

#include "scoop_gc.h"

#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

// --- Immix 参数（v0） ---
//
// 常见 Immix 配置：
// - block：32 KiB
// - line：128 B
//
// v0 目标：
// - 只实现 bump-in-block 的分配路径；
// - 维护 line alloc/mark bitmap 的最小元数据；
// - mark-region（按对象 trace 标记 line）+ partial block 复用（region sweep）在 T1406c 落地。

#define SCOOP_GC_IMMIX_BLOCK_SIZE (32u * 1024u)
#define SCOOP_GC_IMMIX_LINE_SIZE 128u

#define SCOOP_GC_IMMIX_LINES_PER_BLOCK \
  (SCOOP_GC_IMMIX_BLOCK_SIZE / SCOOP_GC_IMMIX_LINE_SIZE)
#define SCOOP_GC_IMMIX_BITMAP_WORDS (SCOOP_GC_IMMIX_LINES_PER_BLOCK / 64u)

#if (SCOOP_GC_IMMIX_BLOCK_SIZE & (SCOOP_GC_IMMIX_BLOCK_SIZE - 1u)) != 0
#error "SCOOP_GC_IMMIX_BLOCK_SIZE must be power of two"
#endif

// "SCOOPIMM"（magic，用于从对象指针反推 block 时做健壮性校验）。
#define SCOOP_GC_IMMIX_BLOCK_MAGIC 0x53434F4F50494D4Dull

typedef struct ScoopGcImmixBlock {
  uint64_t magic;

  // 链表：
  // - next_all：state->all_blocks（用于复位/回收整块 block）
  // - next_free：state->free_blocks（空 block 可直接复用）
  struct ScoopGcImmixBlock *next_all;
  struct ScoopGcImmixBlock *next_free;

  uint8_t *cursor;
  // 当前 hole 的上界（exclusive）。分配在 hole 内 bump；hole 不够时查找下一个 hole。
  // 注意：hole 的边界以 line 为粒度计算（line alloc bitmap），hole 内允许按字节 bump 分配。
  uint8_t *hole_limit;
  uint8_t *payload_start;
  uint8_t *limit;

  // 统计：当前 block 内“仍被 heap.objects 链表持有的对象”数量。
  // v0 仅用于判断整块 block 是否可回收（=0）。
  uint32_t live_objects;
  uint32_t _reserved_u32;

  // line bitmap（每个 bit 对应一个 line）。
  uint64_t line_alloc_bits[SCOOP_GC_IMMIX_BITMAP_WORDS];
  uint64_t line_mark_bits[SCOOP_GC_IMMIX_BITMAP_WORDS];
} ScoopGcImmixBlock;

typedef struct ScoopGcImmixState {
  pthread_mutex_t lock;
  uint32_t lock_inited;

  // best-effort：若看到多个线程参与注册，则 GC collect 退化为 no-op；
  // 同时分配路径会回退到 per-object malloc，避免 data race。
  uint32_t multi_thread_seen;
  uint32_t owner_thread_set;
  pthread_t owner_thread;

  ScoopGcImmixBlock *all_blocks;
  // partially free blocks（live_objects>0 且存在 hole）：优先用于分配以降低碎片化。
  ScoopGcImmixBlock *reusable_blocks;
  ScoopGcImmixBlock *free_blocks;
  ScoopGcImmixBlock *current_block;
} ScoopGcImmixState;

static inline ScoopGcImmixState *scoop_gc_immix_state_from_heap(ScoopGcHeap *heap) {
  if (heap == 0) {
    return 0;
  }
  return (ScoopGcImmixState *)heap->free_list;
}

static inline void scoop_gc_immix_heap_set_state(ScoopGcHeap *heap, ScoopGcImmixState *state) {
  if (heap == 0) {
    return;
  }
  heap->free_list = (ScoopGcFreeBlock *)state;
}

static inline size_t scoop_gc_immix_align_up_size(size_t value, size_t alignment) {
  if (alignment == 0) {
    return value;
  }
  size_t rem = value % alignment;
  if (rem == 0) {
    return value;
  }
  size_t delta = alignment - rem;
  if (value > (SIZE_MAX - delta)) {
    return value;
  }
  return value + delta;
}

static inline uintptr_t scoop_gc_immix_align_up_ptr(uintptr_t value, size_t alignment) {
  if (alignment == 0) {
    return value;
  }
  uintptr_t a = (uintptr_t)alignment;
  uintptr_t rem = value % a;
  if (rem == 0) {
    return value;
  }
  return value + (a - rem);
}

static inline void scoop_gc_immix_bitmap_set_bit(uint64_t *words, size_t word_len, size_t bit) {
  if (words == 0) {
    return;
  }
  size_t idx = bit / 64u;
  if (idx >= word_len) {
    return;
  }
  uint64_t mask = (uint64_t)1u << (uint64_t)(bit % 64u);
  words[idx] |= mask;
}

static inline uint32_t scoop_gc_immix_bitmap_test_bit(const uint64_t *words,
                                                      size_t word_len,
                                                      size_t bit) {
  if (words == 0) {
    return 0;
  }
  size_t idx = bit / 64u;
  if (idx >= word_len) {
    return 0;
  }
  uint64_t mask = (uint64_t)1u << (uint64_t)(bit % 64u);
  return (words[idx] & mask) != 0;
}

static inline void scoop_gc_immix_bitmap_clear_bit(uint64_t *words, size_t word_len, size_t bit) {
  if (words == 0) {
    return;
  }
  size_t idx = bit / 64u;
  if (idx >= word_len) {
    return;
  }
  uint64_t mask = (uint64_t)1u << (uint64_t)(bit % 64u);
  words[idx] &= ~mask;
}

static inline void scoop_gc_immix_bitmap_set_range(uint64_t *words,
                                                   size_t word_len,
                                                   size_t start_bit,
                                                   size_t end_bit) {
  if (words == 0) {
    return;
  }
  if (start_bit > end_bit) {
    return;
  }
  for (size_t bit = start_bit; bit <= end_bit; bit++) {
    scoop_gc_immix_bitmap_set_bit(words, word_len, bit);
  }
}

static inline void scoop_gc_immix_bitmap_clear_range(uint64_t *words,
                                                     size_t word_len,
                                                     size_t start_bit,
                                                     size_t end_bit) {
  if (words == 0) {
    return;
  }
  if (start_bit > end_bit) {
    return;
  }
  for (size_t bit = start_bit; bit <= end_bit; bit++) {
    scoop_gc_immix_bitmap_clear_bit(words, word_len, bit);
  }
}

static inline size_t scoop_gc_immix_block_reserved_lines(const ScoopGcImmixBlock *block) {
  if (block == 0 || block->payload_start == 0) {
    return 0;
  }

  uintptr_t base = (uintptr_t)block;
  uintptr_t payload = (uintptr_t)block->payload_start;
  if (payload < base) {
    return 0;
  }

  size_t reserved_lines =
      (size_t)((payload - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  if (reserved_lines > (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    reserved_lines = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK;
  }
  return reserved_lines;
}

static inline void scoop_gc_immix_block_reset(ScoopGcImmixBlock *block) {
  if (block == 0) {
    return;
  }

  block->magic = SCOOP_GC_IMMIX_BLOCK_MAGIC;
  block->next_free = 0;
  block->live_objects = 0;

  // 清空 bitmaps。
  for (size_t i = 0; i < SCOOP_GC_IMMIX_BITMAP_WORDS; i++) {
    block->line_alloc_bits[i] = 0;
    block->line_mark_bits[i] = 0;
  }

  uint8_t *base = (uint8_t *)block;
  size_t payload_off =
      scoop_gc_immix_align_up_size(sizeof(ScoopGcImmixBlock), SCOOP_GC_IMMIX_LINE_SIZE);
  if (payload_off > (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE) {
    payload_off = (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE;
  }

  block->payload_start = base + payload_off;
  block->cursor = block->payload_start;
  block->limit = base + (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE;
  block->hole_limit = block->limit;

  // 预留区（block header + padding）不允许被分配：标记为已分配的 lines。
  size_t reserved_lines = payload_off / (size_t)SCOOP_GC_IMMIX_LINE_SIZE;
  if (reserved_lines > 0) {
    scoop_gc_immix_bitmap_set_range(block->line_alloc_bits,
                                    SCOOP_GC_IMMIX_BITMAP_WORDS,
                                    0,
                                    reserved_lines - 1);
  }
}

static inline size_t scoop_gc_immix_block_payload_capacity(void) {
  size_t payload_off =
      scoop_gc_immix_align_up_size(sizeof(ScoopGcImmixBlock), SCOOP_GC_IMMIX_LINE_SIZE);
  if (payload_off >= (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE) {
    return 0;
  }
  return (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE - payload_off;
}

static inline ScoopGcImmixBlock *scoop_gc_immix_block_alloc_new(void) {
  void *p = 0;
  // POSIX：对齐到 block size，便于从 object ptr 反推出 block base。
  if (posix_memalign(&p, (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE, (size_t)SCOOP_GC_IMMIX_BLOCK_SIZE) !=
      0) {
    return 0;
  }
  if (p == 0) {
    return 0;
  }

  ScoopGcImmixBlock *block = (ScoopGcImmixBlock *)p;
  // next_all/next_free 在 state 层设置；这里先清理。
  (void)memset(block, 0, sizeof(*block));
  scoop_gc_immix_block_reset(block);
  return block;
}

static inline void scoop_gc_immix_block_mark_allocated_range(ScoopGcImmixBlock *block,
                                                             const uint8_t *start,
                                                             size_t size) {
  if (block == 0 || start == 0 || size == 0) {
    return;
  }

  uintptr_t base = (uintptr_t)block;
  uintptr_t p0 = (uintptr_t)start;
  uintptr_t p1 = p0 + (uintptr_t)size - 1u;

  if (p0 < base) {
    return;
  }
  if (p1 < p0) {
    return;
  }

  size_t start_line =
      (size_t)((p0 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  size_t end_line =
      (size_t)((p1 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  if (start_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    return;
  }
  if (end_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    end_line = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK - 1u;
  }

  scoop_gc_immix_bitmap_set_range(block->line_alloc_bits,
                                  SCOOP_GC_IMMIX_BITMAP_WORDS,
                                  start_line,
                                  end_line);
}

static inline void scoop_gc_immix_block_mark_marked_range(ScoopGcImmixBlock *block,
                                                          const uint8_t *start,
                                                          size_t size) {
  if (block == 0 || start == 0 || size == 0) {
    return;
  }

  uintptr_t base = (uintptr_t)block;
  uintptr_t p0 = (uintptr_t)start;
  uintptr_t p1 = p0 + (uintptr_t)size - 1u;

  if (p0 < base) {
    return;
  }
  if (p1 < p0) {
    return;
  }

  size_t start_line =
      (size_t)((p0 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  size_t end_line =
      (size_t)((p1 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  if (start_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    return;
  }
  if (end_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    end_line = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK - 1u;
  }

  scoop_gc_immix_bitmap_set_range(block->line_mark_bits,
                                  SCOOP_GC_IMMIX_BITMAP_WORDS,
                                  start_line,
                                  end_line);
}

static inline void scoop_gc_immix_block_clear_mark_bits(ScoopGcImmixBlock *block) {
  if (block == 0) {
    return;
  }
  for (size_t i = 0; i < SCOOP_GC_IMMIX_BITMAP_WORDS; i++) {
    block->line_mark_bits[i] = 0;
  }
}

// 查找从 start_line 开始的下一个 hole（连续的“未分配 line”区间），并设置：
// - cursor：hole 起点（line 边界）
// - hole_limit：hole 终点（exclusive，line 边界）
//
// 返回：找到 hole 则为 1，否则为 0。
static inline uint32_t scoop_gc_immix_block_find_next_hole(ScoopGcImmixBlock *block,
                                                           size_t start_line) {
  if (block == 0) {
    return 0;
  }

  size_t reserved = scoop_gc_immix_block_reserved_lines(block);
  if (start_line < reserved) {
    start_line = reserved;
  }
  if (start_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    return 0;
  }

  size_t i = start_line;
  while (i < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    while (i < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK &&
           scoop_gc_immix_bitmap_test_bit(block->line_alloc_bits,
                                          SCOOP_GC_IMMIX_BITMAP_WORDS,
                                          i)) {
      i++;
    }
    if (i >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
      break;
    }

    size_t hole_start = i;
    while (i < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK &&
           !scoop_gc_immix_bitmap_test_bit(block->line_alloc_bits,
                                           SCOOP_GC_IMMIX_BITMAP_WORDS,
                                           i)) {
      i++;
    }
    size_t hole_end = i;

    uintptr_t base = (uintptr_t)block;
    uintptr_t p0 = base + (uintptr_t)hole_start * (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE;
    uintptr_t p1 = base + (uintptr_t)hole_end * (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE;

    if ((uint8_t *)p0 < block->payload_start) {
      p0 = (uintptr_t)block->payload_start;
    }
    if ((uint8_t *)p1 > block->limit) {
      p1 = (uintptr_t)block->limit;
    }
    if (p1 <= p0) {
      continue;
    }

    block->cursor = (uint8_t *)p0;
    block->hole_limit = (uint8_t *)p1;
    return 1;
  }

  block->cursor = block->limit;
  block->hole_limit = block->limit;
  return 0;
}

static inline void scoop_gc_immix_block_setup_first_hole(ScoopGcImmixBlock *block) {
  if (block == 0) {
    return;
  }

  size_t reserved = scoop_gc_immix_block_reserved_lines(block);
  (void)scoop_gc_immix_block_find_next_hole(block, reserved);
}

// Immix v0：在当前 block 的 hole 内 bump 分配；hole 不够时查找下一个 hole。
//
// 注意：
// - hole 的边界以 line 为粒度（alloc bitmap）确定；
// - hole 内按字节 bump，可容纳多个对象（即便它们共享同一 line）。
static inline void *scoop_gc_immix_block_alloc(ScoopGcImmixBlock *block,
                                               size_t size,
                                               size_t alignment) {
  if (block == 0 || size == 0) {
    return 0;
  }
  if (alignment == 0) {
    alignment = 1;
  }

  if (block->payload_start == 0 || block->limit == 0) {
    return 0;
  }

  if (block->cursor == 0) {
    block->cursor = block->payload_start;
  }
  if (block->hole_limit == 0) {
    block->hole_limit = block->limit;
  }
  if (block->cursor < block->payload_start) {
    block->cursor = block->payload_start;
  }

  // 若 cursor 已越过当前 hole，尝试查找下一个 hole（从 cursor 所在 line 开始）。
  if (block->cursor >= block->hole_limit) {
    uintptr_t base = (uintptr_t)block;
    uintptr_t p = (uintptr_t)block->cursor;
    size_t line = (size_t)((p - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
    (void)scoop_gc_immix_block_find_next_hole(block, line);
  }

  for (uint32_t attempts = 0; attempts < 64; attempts++) {
    uintptr_t cursor = (uintptr_t)block->cursor;
    uintptr_t aligned = scoop_gc_immix_align_up_ptr(cursor, alignment);
    if (aligned < cursor) {
      return 0;
    }

    uintptr_t next = aligned + (uintptr_t)size;
    if (next < aligned) {
      return 0;
    }

    if (next <= (uintptr_t)block->hole_limit) {
      block->cursor = (uint8_t *)next;
      block->live_objects += 1;
      scoop_gc_immix_block_mark_allocated_range(block, (const uint8_t *)aligned, size);
      return (void *)aligned;
    }

    // 当前 hole 放不下：从 hole_limit 之后查找下一个 hole。
    uintptr_t base = (uintptr_t)block;
    uintptr_t p = (uintptr_t)block->hole_limit;
    size_t line = (size_t)((p - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
    if (!scoop_gc_immix_block_find_next_hole(block, line)) {
      return 0;
    }
  }

  return 0;
}

static inline void *scoop_gc_immix_block_alloc_bump(ScoopGcImmixBlock *block,
                                                    size_t size,
                                                    size_t alignment) {
  if (block == 0 || size == 0) {
    return 0;
  }

  uintptr_t cursor = (uintptr_t)block->cursor;
  uintptr_t aligned = scoop_gc_immix_align_up_ptr(cursor, alignment);
  if (aligned < cursor) {
    return 0;
  }

  uintptr_t next = aligned + (uintptr_t)size;
  if (next < aligned) {
    return 0;
  }
  if (next > (uintptr_t)block->limit) {
    return 0;
  }

  block->cursor = (uint8_t *)next;
  block->live_objects += 1;
  scoop_gc_immix_block_mark_allocated_range(block, (const uint8_t *)aligned, size);
  return (void *)aligned;
}

static inline ScoopGcImmixBlock *scoop_gc_immix_state_take_block(ScoopGcImmixState *state) {
  if (state == 0) {
    return 0;
  }

  ScoopGcImmixBlock *block = 0;
  if (state->reusable_blocks != 0) {
    block = state->reusable_blocks;
    state->reusable_blocks = block->next_free;
    block->next_free = 0;
  } else
  if (state->free_blocks != 0) {
    block = state->free_blocks;
    state->free_blocks = block->next_free;
    block->next_free = 0;
    // 空闲 block 在进入 free list 时已 reset；这里无需再 reset。
  } else {
    block = scoop_gc_immix_block_alloc_new();
    if (block == 0) {
      return 0;
    }
    block->next_all = state->all_blocks;
    state->all_blocks = block;
  }

  state->current_block = block;
  return block;
}

static inline ScoopGcImmixBlock *scoop_gc_immix_block_from_object(void *object) {
  if (object == 0) {
    return 0;
  }

  uintptr_t base =
      ((uintptr_t)object) & ~((uintptr_t)SCOOP_GC_IMMIX_BLOCK_SIZE - 1u);
  ScoopGcImmixBlock *block = (ScoopGcImmixBlock *)base;
  if (block->magic != SCOOP_GC_IMMIX_BLOCK_MAGIC) {
    return 0;
  }
  return block;
}

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
