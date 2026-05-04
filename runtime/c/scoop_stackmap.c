// LLVM StackMap registry (runtime-side).
//
// 说明（T1504）：
// - 运行时需要能够通过“栈帧的 return address”定位到对应的 stackmap record，
//   从而在 stop-the-world 时枚举该帧的 roots（后续任务 T1505/T1506）。
// - 本文件提供：
//   - StackMap section 解析（version 3 为主：header + function records + constants + records）
//   - `return_address -> record` 的索引结构（排序数组 + 二分查找）
//   - 从当前进程镜像中尽力定位 stackmap section 并注册
//
// 注意：
// - 当前解析逻辑以 little-endian 为主（host x86_64/arm64 均为 LE）。
// - 为了让 “未包含 stackmap section 的程序” 仍能运行：找不到 section 时不报错，只返回 0。

#if defined(__linux__)
#define _GNU_SOURCE   // dl_iterate_phdr / struct dl_phdr_info require __USE_GNU
#endif

#include "scoop_stackmap.h"

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(__APPLE__)
#include <mach-o/dyld.h>
#include <mach-o/getsect.h>
#include <mach-o/loader.h>
#endif

#if defined(__linux__)
#include <elf.h>
#include <link.h>
#endif

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>
#endif

// --- 小工具：LE 读写与边界检查 ---

static int scoop_stackmap_read_u8(const uint8_t *bytes,
                                  size_t len,
                                  size_t *off,
                                  uint8_t *out) {
  if (bytes == 0 || out == 0 || off == 0) {
    return 0;
  }
  if (*off + 1u > len) {
    return 0;
  }
  *out = bytes[*off];
  *off += 1u;
  return 1;
}

static int scoop_stackmap_read_u16_le(const uint8_t *bytes,
                                      size_t len,
                                      size_t *off,
                                      uint16_t *out) {
  if (bytes == 0 || out == 0 || off == 0) {
    return 0;
  }
  if (*off + 2u > len) {
    return 0;
  }
  const uint16_t v = (uint16_t)(bytes[*off] | ((uint16_t)bytes[*off + 1u] << 8u));
  *out = v;
  *off += 2u;
  return 1;
}

static int scoop_stackmap_read_u32_le(const uint8_t *bytes,
                                      size_t len,
                                      size_t *off,
                                      uint32_t *out) {
  if (bytes == 0 || out == 0 || off == 0) {
    return 0;
  }
  if (*off + 4u > len) {
    return 0;
  }
  const uint32_t v = (uint32_t)bytes[*off] |
                     ((uint32_t)bytes[*off + 1u] << 8u) |
                     ((uint32_t)bytes[*off + 2u] << 16u) |
                     ((uint32_t)bytes[*off + 3u] << 24u);
  *out = v;
  *off += 4u;
  return 1;
}

static int scoop_stackmap_read_u64_le(const uint8_t *bytes,
                                      size_t len,
                                      size_t *off,
                                      uint64_t *out) {
  if (bytes == 0 || out == 0 || off == 0) {
    return 0;
  }
  if (*off + 8u > len) {
    return 0;
  }
  uint64_t v = 0;
  for (uint32_t i = 0; i < 8u; i++) {
    v |= ((uint64_t)bytes[*off + (size_t)i]) << (8u * i);
  }
  *out = v;
  *off += 8u;
  return 1;
}

static int scoop_stackmap_read_i32_le(const uint8_t *bytes,
                                      size_t len,
                                      size_t *off,
                                      int32_t *out) {
  if (bytes == 0 || out == 0 || off == 0) {
    return 0;
  }
  uint32_t raw = 0;
  if (!scoop_stackmap_read_u32_le(bytes, len, off, &raw)) {
    return 0;
  }
  *out = (int32_t)raw;
  return 1;
}

static int scoop_stackmap_skip_bytes(size_t len, size_t *off, size_t n) {
  if (off == 0) {
    return 0;
  }
  if (n > len) {
    return 0;
  }
  if (*off + n > len) {
    return 0;
  }
  *off += n;
  return 1;
}

static size_t scoop_stackmap_align_up(size_t v, size_t align) {
  if (align == 0) {
    return v;
  }
  const size_t mask = align - 1u;
  return (v + mask) & ~mask;
}

// --- StackMap record locations -> roots slots（TODO T1506a） ---

// Stackmap location kinds（LLVM StackMap v3，见 StackMap section spec）。
#define SCOOP_STACKMAP_LOC_REGISTER 1u
#define SCOOP_STACKMAP_LOC_DIRECT 2u
#define SCOOP_STACKMAP_LOC_INDIRECT 3u
#define SCOOP_STACKMAP_LOC_CONSTANT 4u
#define SCOOP_STACKMAP_LOC_CONSTANT_INDEX 5u

// StackMap v3：每个 location 固定 12 字节（Type/Reserved/Size/Reg/Reserved/Offset）。
#define SCOOP_STACKMAP_LOC_BYTE_LEN 12u

static int scoop_stackmap_dwarf_reg_is_sp(uint16_t dwarf_reg) {
// DWARF register numbers（subset）：
// - x86_64：RSP = 7
// - AArch64：SP = 31
#if defined(__x86_64__) || defined(_M_X64)
  return dwarf_reg == 7u;
#elif defined(__aarch64__) || defined(__arm64__)
  return dwarf_reg == 31u;
#else
  (void)dwarf_reg;
  return 0;
#endif
}

// 在部分构建配置（尤其是 -O0 / debug）中，statepoint stackmap 的 Direct/Indirect 可能以“frame pointer”为基址：
// - x86_64：RBP = 6（DWARF reg）
// - AArch64：FP(x29) = 29（DWARF reg）
//
// v0 不直接捕获寄存器文件，因此需要从 CFA（`frame_sp`）近似恢复 FP。
// 做法：尝试若干个常见的 `FP = CFA - delta` 候选，并用 “*(FP + ptr_size) ≈ return_address”
// 验证（return address 来自 `rec->return_address`）。
static int scoop_stackmap_dwarf_reg_is_fp(uint16_t dwarf_reg) {
#if defined(__x86_64__) || defined(_M_X64)
  return dwarf_reg == 6u;
#elif defined(__aarch64__) || defined(__arm64__)
  return dwarf_reg == 29u;
#else
  (void)dwarf_reg;
  return 0;
#endif
}

static intptr_t scoop_stackmap_guess_fp_base_from_cfa(uintptr_t cfa, uintptr_t expected_ra) {
  if (cfa == 0) {
    return 0;
  }
  const uintptr_t ptr_size = (uintptr_t)sizeof(void *);
  if (ptr_size == 0) {
    return 0;
  }

  // 常见候选：不同 ABI/优化级别下，CFA 与 FP 的距离可能不同；保持范围小且可回归。
  const uintptr_t deltas[] = {16u, 24u, 32u, 40u, 48u, 56u, 64u};

  for (size_t i = 0; i < (sizeof(deltas) / sizeof(deltas[0])); i++) {
    const uintptr_t delta = deltas[i];
    if (cfa <= delta) {
      continue;
    }
    const uintptr_t fp = cfa - delta;
    if ((fp % ptr_size) != 0u) {
      continue;
    }

    // 按常见 frame layout：return address 通常位于 [FP + ptr_size]。
    const uintptr_t mem_ra = *(const uintptr_t *)(fp + ptr_size);
    if (mem_ra == 0) {
      continue;
    }

    intptr_t diff = (intptr_t)mem_ra - (intptr_t)expected_ra;
    if (diff < 0) {
      diff = -diff;
    }

    // 容忍少量偏移（stackmap lookup 本身也允许 return address 近似匹配）。
    if ((uintptr_t)diff <= 256u) {
      // 二次校验：frame pointer 链的形状应合理（saved_fp 指向更外层帧，且距离在可接受范围内）。
      const uintptr_t saved_fp = *(const uintptr_t *)fp;
      if (saved_fp == 0) {
        continue;
      }
      if ((saved_fp % ptr_size) != 0u) {
        continue;
      }
      if (saved_fp <= fp) {
        continue;
      }
      if ((saved_fp - fp) > (uintptr_t)(1024u * 1024u)) {
        continue;
      }
      return (intptr_t)fp;
    }
  }

  return 0;
}

// `SCOOP_STACKMAP_STRICT=1`：用于启用 fail-fast 诊断（A2/A3）。
// 注意：该函数定义在 registry 段落中；这里前置声明以便 locations 解析使用。
static int scoop_stackmap_strict_mode_enabled(void);

typedef struct ScoopStackmapLocationMin {
  uint8_t type;
  uint16_t size;
  uint16_t dwarf_reg;
  int32_t offset;
} ScoopStackmapLocationMin;

static int scoop_stackmap_record_read_location_at(const uint8_t *bytes,
                                                  size_t len,
                                                  size_t locs_off,
                                                  uint16_t idx,
                                                  ScoopStackmapLocationMin *out) {
  if (bytes == 0 || out == 0) {
    return 0;
  }

  const size_t off0 = locs_off + ((size_t)idx * (size_t)SCOOP_STACKMAP_LOC_BYTE_LEN);
  size_t off = off0;

  uint8_t loc_type = 0;
  uint8_t loc_reserved0 = 0;
  uint16_t loc_size = 0;
  uint16_t dwarf_reg = 0;
  uint16_t loc_reserved1 = 0;
  int32_t loc_off_i32 = 0;

  if (!scoop_stackmap_read_u8(bytes, len, &off, &loc_type) ||
      !scoop_stackmap_read_u8(bytes, len, &off, &loc_reserved0) ||
      !scoop_stackmap_read_u16_le(bytes, len, &off, &loc_size) ||
      !scoop_stackmap_read_u16_le(bytes, len, &off, &dwarf_reg) ||
      !scoop_stackmap_read_u16_le(bytes, len, &off, &loc_reserved1) ||
      !scoop_stackmap_read_i32_le(bytes, len, &off, &loc_off_i32)) {
    return 0;
  }

  (void)loc_reserved0;
  (void)loc_reserved1;

  out->type = loc_type;
  out->size = loc_size;
  out->dwarf_reg = dwarf_reg;
  out->offset = loc_off_i32;
  return 1;
}

static int scoop_stackmap_location_is_root_slot_shape(const ScoopStackmapLocationMin *loc,
                                                      uint16_t want_size) {
  if (loc == 0) {
    return 0;
  }
  if (loc->size != want_size) {
    return 0;
  }
  if (loc->type != SCOOP_STACKMAP_LOC_DIRECT && loc->type != SCOOP_STACKMAP_LOC_INDIRECT) {
    return 0;
  }
  return scoop_stackmap_dwarf_reg_is_sp(loc->dwarf_reg) ||
         scoop_stackmap_dwarf_reg_is_fp(loc->dwarf_reg);
}

uint64_t scoop_stackmap_record_visit_root_slots(const ScoopStackmapRecordRef *rec,
                                                uintptr_t frame_sp,
                                                uintptr_t frame_fp,
                                                ScoopGcTraceVisitor visitor,
                                                void *ctx,
                                                uint32_t *out_error) {
  if (out_error != 0) {
    *out_error = SCOOP_STACKMAP_VISIT_OK;
  }

  if (rec == 0 || visitor == 0 || rec->record_ptr == 0 || rec->record_size == 0 || frame_sp == 0) {
    if (out_error != 0) {
      *out_error = SCOOP_STACKMAP_VISIT_ERR_INVALID_ARGUMENT;
    }
    return 0;
  }

  const uint8_t *bytes = rec->record_ptr;
  const size_t len = (size_t)rec->record_size;
  size_t off = 0;

  // record layout（version 3）：
  //  u64 PatchPointID
  //  u32 InstructionOffset
  //  u16 Reserved
  //  u16 NumLocations
  //  Location[NumLocations]（每个 12 bytes）
  uint64_t patchpoint_id = 0;
  uint32_t instruction_offset = 0;
  uint16_t reserved0 = 0;
  uint16_t num_locations = 0;

  if (!scoop_stackmap_read_u64_le(bytes, len, &off, &patchpoint_id) ||
      !scoop_stackmap_read_u32_le(bytes, len, &off, &instruction_offset) ||
      !scoop_stackmap_read_u16_le(bytes, len, &off, &reserved0) ||
      !scoop_stackmap_read_u16_le(bytes, len, &off, &num_locations)) {
    (void)patchpoint_id;
    (void)instruction_offset;
    (void)reserved0;
    if (out_error != 0) {
      *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_TOO_SHORT;
    }
    return 0;
  }

  // 健壮性：避免异常 num_locations 造成越界/长循环。
  if ((size_t)num_locations > ((len - off) / (size_t)SCOOP_STACKMAP_LOC_BYTE_LEN)) {
    if (out_error != 0) {
      *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED;
    }
    return 0;
  }

  const uint16_t want_size =
      (sizeof(void *) > (size_t)UINT16_MAX) ? (uint16_t)UINT16_MAX : (uint16_t)sizeof(void *);

  uint64_t visited = 0;

  // --- GC roots locations 契约（GC-FIX Phase A1/A4） ---
  //
  // 目标：
  // - roots 必须是“可寻址且可写回”的 slots（moving/compaction 的关键语义）；
  // - 因此我们只扫描编译器保证的 roots slots 后缀，而不是“扫所有 pointer-sized Direct/Indirect”。
  //
  // 契约（与 `crates/scoopc/src/stackmap.rs::verify_roots_contract` 一致）：
  // - roots locations 是 `locations` 列表的连续后缀；
  // - 后缀内每个 roots location 都必须是可写回 slot：
  //   - kind=Direct/Indirect
  //   - size=pointer-sized
  //   - base reg=SP/FP（runtime 可计算地址）
  // - roots locations 数量必须为偶数（statepoint base/derived 成对语义）。
  //
  // 若契约被破坏，纯 stackmap 模式会出现 silent mis-collection；因此这里返回稳定错误码，
  // 由上层（GC）决定 fail-fast。

  const size_t locs_off = off;

  // 1) roots 后缀起点：从尾部向前扫描，直到遇到第一个“不像 roots slot”的 location。
  uint16_t roots_start = num_locations;
  while (roots_start > 0) {
    const uint16_t idx = (uint16_t)(roots_start - 1u);
    ScoopStackmapLocationMin loc = {0};
    if (!scoop_stackmap_record_read_location_at(bytes, len, locs_off, idx, &loc)) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED;
      }
      return 0;
    }
    if (!scoop_stackmap_location_is_root_slot_shape(&loc, want_size)) {
      break;
    }
    roots_start -= 1u;
  }

  const uint16_t roots_len = (uint16_t)(num_locations - roots_start);
  if ((roots_len % 2u) != 0u) {
    if (out_error != 0) {
      *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_LOCATION;
    }
    return 0;
  }

  // 2) roots 必须是连续后缀：roots_start 之前不允许再出现“像 roots slot”的 location。
  for (uint16_t i = 0; i < roots_start; i++) {
    ScoopStackmapLocationMin loc = {0};
    if (!scoop_stackmap_record_read_location_at(bytes, len, locs_off, i, &loc)) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED;
      }
      return 0;
    }

    // 健壮性：pointer-sized Direct/Indirect 但 base reg 非 SP/FP，runtime 无法计算 slot 地址。
    if (loc.size == want_size &&
        (loc.type == SCOOP_STACKMAP_LOC_DIRECT || loc.type == SCOOP_STACKMAP_LOC_INDIRECT) &&
        !(scoop_stackmap_dwarf_reg_is_sp(loc.dwarf_reg) ||
          scoop_stackmap_dwarf_reg_is_fp(loc.dwarf_reg))) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_DWARF_REG;
      }
      return 0;
    }

    if (scoop_stackmap_location_is_root_slot_shape(&loc, want_size)) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_LOCATION;
      }
      return 0;
    }
  }

  // 3) 仅遍历 roots slots 后缀，把每个 location 转换为可写回的 `void** slot` 并回调 visitor。
  for (uint16_t i = roots_start; i < num_locations; i++) {
    ScoopStackmapLocationMin loc = {0};
    if (!scoop_stackmap_record_read_location_at(bytes, len, locs_off, i, &loc)) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED;
      }
      return visited;
    }

    if (!scoop_stackmap_location_is_root_slot_shape(&loc, want_size)) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_LOCATION;
      }
      return visited;
    }

    // NOTE:
    // - stackmap offset 是有符号 32-bit（可为负），因此这里用 intptr_t 做指针算术；
    // - `frame_sp` 语义为 CFA（call frame address，来自 platform/unwind）。
    intptr_t base_i = 0;
    if (scoop_stackmap_dwarf_reg_is_sp(loc.dwarf_reg)) {
      base_i = (intptr_t)frame_sp;
    } else if (scoop_stackmap_dwarf_reg_is_fp(loc.dwarf_reg)) {
      // 优先使用 platform/unwind 直接提供的 FP。
      //
      // A3：纯模式下不允许长期依赖“从 CFA 猜 FP”的启发式，因此在严格模式（`SCOOP_STACKMAP_STRICT=1`）
      // 下若缺失 FP，直接视为错误并 fail-fast（由上层决定如何处理）。
      base_i = (intptr_t)frame_fp;
      if (base_i == 0 && !scoop_stackmap_strict_mode_enabled()) {
        base_i = scoop_stackmap_guess_fp_base_from_cfa(frame_sp, rec->return_address);
      }
      if (base_i == 0) {
        if (out_error != 0) {
          *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_DWARF_REG;
        }
        return visited;
      }
    } else {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_UNSUPPORTED_DWARF_REG;
      }
      return visited;
    }

    const intptr_t addr_i = base_i + (intptr_t)loc.offset;
    if (addr_i == 0) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_RECORD_MALFORMED;
      }
      return visited;
    }

    const uintptr_t addr = (uintptr_t)addr_i;
    if ((addr % (uintptr_t)sizeof(void *)) != 0u) {
      if (out_error != 0) {
        *out_error = SCOOP_STACKMAP_VISIT_ERR_UNALIGNED_SLOT;
      }
      return visited;
    }

    if (loc.type == SCOOP_STACKMAP_LOC_DIRECT) {
      void **slot = (void **)addr;
      if (*slot == 0) {
        continue;
      }
      visitor(slot, ctx);
      visited += 1;
      continue;
    }

    if (loc.type == SCOOP_STACKMAP_LOC_INDIRECT) {
      // LLVM statepoint stackmaps（当前 host 目标）会把 roots slots 编码为 `Indirect`（T1503b/C2a），
      // 但其语义对我们来说仍然是“可写回的根槽位地址 = base + offset”。
      //
      // 重要：moving/compaction 需要把新地址写回 slot；因此这里将 `Indirect` 与 `Direct`
      // 统一为“slot = (base + off)”的处理方式。
      void **slot = (void **)addr;
      if (*slot == 0) {
        continue;
      }
      visitor(slot, ctx);
      visited += 1;
      continue;
    }
  }

  return visited;
}

// --- registry：排序数组 + 二分查找 ---

typedef struct ScoopStackmapRegistry {
  ScoopStackmapRecordRef *entries;
  size_t len;
  size_t cap;

  // 用于避免在 `scoop_runtime_init()` 的重复调用中重复注册（早期 runtime 允许重复 init）。
  uint32_t registered_current_process;
} ScoopStackmapRegistry;

static ScoopStackmapRegistry scoop_stackmap_registry = {0};
static pthread_mutex_t scoop_stackmap_registry_lock = PTHREAD_MUTEX_INITIALIZER;

static int scoop_stackmap_parse_bool_env_default0(const char *key) {
  const char *v = getenv(key);
  if (v == 0 || v[0] == '\0') {
    return 0;
  }
  // 常见真值：1/true/yes/on（大小写不敏感）。
  if (strcmp(v, "1") == 0 || strcmp(v, "true") == 0 || strcmp(v, "TRUE") == 0 ||
      strcmp(v, "yes") == 0 || strcmp(v, "YES") == 0 || strcmp(v, "on") == 0 ||
      strcmp(v, "ON") == 0) {
    return 1;
  }
  return 0;
}

static int scoop_stackmap_strict_mode_enabled(void) {
  static int cached = -1;
  if (cached >= 0) {
    return cached;
  }
  cached = scoop_stackmap_parse_bool_env_default0("SCOOP_STACKMAP_STRICT");
  return cached;
}

static int scoop_stackmap_record_identity_eq(const ScoopStackmapRecordRef *a,
                                             const ScoopStackmapRecordRef *b) {
  if (a == 0 || b == 0) {
    return 0;
  }
  return a->function_address == b->function_address && a->instruction_offset == b->instruction_offset &&
         a->patchpoint_id == b->patchpoint_id && a->record_size == b->record_size;
}

static int scoop_stackmap_registry_reserve_locked(size_t additional) {
  const size_t need = scoop_stackmap_registry.len + additional;
  if (need <= scoop_stackmap_registry.cap) {
    return 1;
  }

  size_t new_cap = scoop_stackmap_registry.cap;
  if (new_cap == 0) {
    new_cap = 64u;
  }
  while (new_cap < need) {
    // 2x growth；避免 overflow。
    if (new_cap > (SIZE_MAX / 2u)) {
      new_cap = need;
      break;
    }
    new_cap *= 2u;
  }

  if (new_cap > (SIZE_MAX / sizeof(ScoopStackmapRecordRef))) {
    return 0;
  }

  void *new_ptr = realloc(scoop_stackmap_registry.entries,
                          new_cap * sizeof(ScoopStackmapRecordRef));
  if (new_ptr == 0) {
    return 0;
  }

  scoop_stackmap_registry.entries = (ScoopStackmapRecordRef *)new_ptr;
  scoop_stackmap_registry.cap = new_cap;
  return 1;
}

static int scoop_stackmap_cmp_return_address(const void *a, const void *b) {
  const ScoopStackmapRecordRef *ra = (const ScoopStackmapRecordRef *)a;
  const ScoopStackmapRecordRef *rb = (const ScoopStackmapRecordRef *)b;
  if (ra->return_address < rb->return_address) {
    return -1;
  }
  if (ra->return_address > rb->return_address) {
    return 1;
  }
  return 0;
}

static void scoop_stackmap_registry_sort_and_dedupe_locked(void) {
  if (scoop_stackmap_registry.len <= 1) {
    return;
  }

  qsort(scoop_stackmap_registry.entries,
        scoop_stackmap_registry.len,
        sizeof(ScoopStackmapRecordRef),
        scoop_stackmap_cmp_return_address);

  // 去重：相同 return_address 只保留第一条（未来如需更强诊断，可升级为“冲突即报错/日志”）。
  size_t write = 1;
  for (size_t read = 1; read < scoop_stackmap_registry.len; read++) {
    if (scoop_stackmap_registry.entries[read].return_address ==
        scoop_stackmap_registry.entries[write - 1].return_address) {
      // A2：同一 key 出现多个不同 record 属于“无歧义命中”破坏，严格模式下应 fail-fast。
      if (!scoop_stackmap_record_identity_eq(&scoop_stackmap_registry.entries[read],
                                             &scoop_stackmap_registry.entries[write - 1])) {
        if (scoop_stackmap_strict_mode_enabled()) {
          (void)fprintf(stderr,
                        "[scooprt][stackmap] conflict: duplicate return_address=0x%lx "
                        "(patchpoint_id=%llu vs %llu)\n",
                        (unsigned long)scoop_stackmap_registry.entries[read].return_address,
                        (unsigned long long)scoop_stackmap_registry.entries[write - 1].patchpoint_id,
                        (unsigned long long)scoop_stackmap_registry.entries[read].patchpoint_id);
          abort();
        }
      }
      continue;
    }
    scoop_stackmap_registry.entries[write] = scoop_stackmap_registry.entries[read];
    write++;
  }
  scoop_stackmap_registry.len = write;
}

static uint32_t scoop_stackmap_registry_register_entries(ScoopStackmapRecordRef *entries,
                                                         size_t entry_len) {
  if (entries == 0 || entry_len == 0) {
    return 0;
  }

  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);

  if (!scoop_stackmap_registry_reserve_locked(entry_len)) {
    (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
    return 0;
  }

  memcpy(&scoop_stackmap_registry.entries[scoop_stackmap_registry.len],
         entries,
         entry_len * sizeof(ScoopStackmapRecordRef));
  scoop_stackmap_registry.len += entry_len;

  scoop_stackmap_registry_sort_and_dedupe_locked();

  const uint32_t out_added =
      (entry_len > (size_t)UINT32_MAX) ? UINT32_MAX : (uint32_t)entry_len;
  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
  return out_added;
}

// --- StackMap section 解析（version 3） ---

typedef struct ScoopStackmapFunctionRec {
  uint64_t function_address;
  uint64_t record_count;
} ScoopStackmapFunctionRec;

// 解析 section 并返回“本次解析得到的 entries”（malloc 出来的数组；调用方负责 free）。
static uint32_t scoop_stackmap_parse_section(const uint8_t *bytes,
                                             size_t len,
                                             ScoopStackmapRecordRef **out_entries,
                                             size_t *out_len) {
  if (out_entries == 0 || out_len == 0) {
    return 0;
  }
  *out_entries = 0;
  *out_len = 0;

  if (bytes == 0 || len < 16u) {
    return 0;
  }

  size_t off = 0;
  uint8_t version = 0;
  uint8_t reserved0 = 0;
  uint16_t reserved1 = 0;
  uint32_t num_functions = 0;
  uint32_t num_constants = 0;
  uint32_t num_records = 0;

  if (!scoop_stackmap_read_u8(bytes, len, &off, &version)) {
    return 0;
  }
  if (!scoop_stackmap_read_u8(bytes, len, &off, &reserved0)) {
    return 0;
  }
  (void)reserved0;
  if (!scoop_stackmap_read_u16_le(bytes, len, &off, &reserved1)) {
    return 0;
  }
  (void)reserved1;
  if (!scoop_stackmap_read_u32_le(bytes, len, &off, &num_functions)) {
    return 0;
  }
  if (!scoop_stackmap_read_u32_le(bytes, len, &off, &num_constants)) {
    return 0;
  }
  if (!scoop_stackmap_read_u32_le(bytes, len, &off, &num_records)) {
    return 0;
  }

  // 当前只支持 LLVM 常见的 v3 格式；其它版本先保守拒绝（返回 0，不 crash）。
  if (version != 3u) {
    return 0;
  }

  if (num_functions == 0 || num_records == 0) {
    return 0;
  }

  if ((size_t)num_functions > (SIZE_MAX / sizeof(ScoopStackmapFunctionRec))) {
    return 0;
  }

  ScoopStackmapFunctionRec *fns =
      (ScoopStackmapFunctionRec *)calloc((size_t)num_functions,
                                         sizeof(ScoopStackmapFunctionRec));
  if (fns == 0) {
    return 0;
  }

  uint64_t records_sum = 0;

  for (uint32_t i = 0; i < num_functions; i++) {
    uint64_t fn_addr = 0;
    uint64_t stack_size = 0;
    uint64_t rec_count = 0;
    if (!scoop_stackmap_read_u64_le(bytes, len, &off, &fn_addr) ||
        !scoop_stackmap_read_u64_le(bytes, len, &off, &stack_size) ||
        !scoop_stackmap_read_u64_le(bytes, len, &off, &rec_count)) {
      free(fns);
      return 0;
    }
    (void)stack_size;
    fns[i].function_address = fn_addr;
    fns[i].record_count = rec_count;

    // 记录总数用于 sanity check（但不强制一致：不同 LLVM 版本/链接策略可能导致差异）。
    if (UINT64_MAX - records_sum < rec_count) {
      free(fns);
      return 0;
    }
    records_sum += rec_count;
  }

  // constants：每个 constant 是一个 u64。
  for (uint32_t i = 0; i < num_constants; i++) {
    uint64_t ignored = 0;
    if (!scoop_stackmap_read_u64_le(bytes, len, &off, &ignored)) {
      free(fns);
      return 0;
    }
  }

  // 以 header 的 num_records 作为上界，避免 records_sum 非法导致 OOM。
  const uint64_t planned = (records_sum < (uint64_t)num_records) ? records_sum : (uint64_t)num_records;
  if (planned == 0) {
    free(fns);
    return 0;
  }
  if (planned > (uint64_t)(SIZE_MAX / sizeof(ScoopStackmapRecordRef))) {
    free(fns);
    return 0;
  }

  ScoopStackmapRecordRef *entries =
      (ScoopStackmapRecordRef *)calloc((size_t)planned, sizeof(ScoopStackmapRecordRef));
  if (entries == 0) {
    free(fns);
    return 0;
  }

  size_t entry_len = 0;

  // record layout（version 3）：
  //  u64 PatchPointID
  //  u32 InstructionOffset
  //  u16 Reserved
  //  u16 NumLocations
  //  Location[NumLocations] （每个 12 bytes）
  //  u16 NumLiveOuts
  //  u16 Reserved
  //  LiveOut[NumLiveOuts] （每个 4 bytes）
  //  padding to 8 bytes
  for (uint32_t fi = 0; fi < num_functions; fi++) {
    const uint64_t fn_addr = fns[fi].function_address;
    const uint64_t rec_count = fns[fi].record_count;
    for (uint64_t ri = 0; ri < rec_count; ri++) {
      if (entry_len >= (size_t)planned) {
        break;
      }

      const size_t record_start = off;

      uint64_t patchpoint_id = 0;
      uint32_t instruction_offset = 0;
      uint16_t rec_reserved = 0;
      uint16_t num_locations = 0;

      if (!scoop_stackmap_read_u64_le(bytes, len, &off, &patchpoint_id) ||
          !scoop_stackmap_read_u32_le(bytes, len, &off, &instruction_offset) ||
          !scoop_stackmap_read_u16_le(bytes, len, &off, &rec_reserved) ||
          !scoop_stackmap_read_u16_le(bytes, len, &off, &num_locations)) {
        free(entries);
        free(fns);
        return 0;
      }
      (void)rec_reserved;

      // locations：每个 12 bytes。
      const size_t locations_bytes = (size_t)num_locations * 12u;
      if (!scoop_stackmap_skip_bytes(len, &off, locations_bytes)) {
        free(entries);
        free(fns);
        return 0;
      }

      // 注意：LLVM stackmap records 在 locations 之后通常会做一次 8-byte 对齐，
      // 以保证后续 liveouts header 的读取位置满足对齐约定。
      // 若缺少该对齐，会导致后续 record 起始偏移错位（真实 statepoint 产物常见）。
      const size_t after_locations_aligned = scoop_stackmap_align_up(off, 8u);
      if (after_locations_aligned > len) {
        free(entries);
        free(fns);
        return 0;
      }
      off = after_locations_aligned;

      uint16_t num_live_outs = 0;
      uint16_t live_reserved = 0;
      if (!scoop_stackmap_read_u16_le(bytes, len, &off, &num_live_outs) ||
          !scoop_stackmap_read_u16_le(bytes, len, &off, &live_reserved)) {
        free(entries);
        free(fns);
        return 0;
      }
      (void)live_reserved;

      // liveouts：每个 4 bytes。
      const size_t liveouts_bytes = (size_t)num_live_outs * 4u;
      if (!scoop_stackmap_skip_bytes(len, &off, liveouts_bytes)) {
        free(entries);
        free(fns);
        return 0;
      }

      // record 末尾按 8 字节对齐（LLVM emission 的常见约定）。
      const size_t aligned = scoop_stackmap_align_up(off, 8u);
      if (aligned > len) {
        free(entries);
        free(fns);
        return 0;
      }
      off = aligned;
      const size_t record_end = off;

      // return address 约定：
      // - LLVM stackmap record 的 `InstructionOffset` 是“相对 function_address 的偏移”。
      // - 在后续 stack walking 中我们用栈帧的 return address（ip）做 key；
      //   这里先采用 `return_address = function_address + instruction_offset` 的最小规则。
      //
      // 注：
      // - 不同后端/指令集上 “callsite offset vs return address” 可能存在 +call_size 或 -1 的差异；
      // - 为了更贴近后续 GC stack walking 的输入（栈上 return address），lookup 侧会容忍小范围偏移。
      const uint64_t ra64 = fn_addr + (uint64_t)instruction_offset;
      if ((uintptr_t)ra64 != ra64) {
        // 32-bit 平台或溢出：跳过该 record。
        continue;
      }

      ScoopStackmapRecordRef *e = &entries[entry_len];
      e->return_address = (uintptr_t)ra64;
      e->function_address = (uintptr_t)fn_addr;
      e->instruction_offset = instruction_offset;
      e->patchpoint_id = patchpoint_id;
      e->record_ptr = bytes + record_start;
      e->record_size = (uint32_t)(record_end - record_start);

      entry_len++;
    }
  }

  free(fns);
  if (entry_len == 0) {
    free(entries);
    return 0;
  }

  *out_entries = entries;
  *out_len = entry_len;
  return (uint32_t)entry_len;
}

// --- public API ---

uint32_t scoop_stackmap_registry_register_section(const uint8_t *bytes, size_t len) {
  ScoopStackmapRecordRef *entries = 0;
  size_t entry_len = 0;
  const uint32_t parsed = scoop_stackmap_parse_section(bytes, len, &entries, &entry_len);
  if (parsed == 0 || entries == 0 || entry_len == 0) {
    return 0;
  }

  const uint32_t out_added = scoop_stackmap_registry_register_entries(entries, entry_len);
  free(entries);
  return out_added;
}

#if defined(__linux__)
typedef struct ScoopStackmapElfAddrRanges {
  uintptr_t starts[64];
  uintptr_t ends[64];
  size_t len;
} ScoopStackmapElfAddrRanges;

static void scoop_stackmap_elf_ranges_push(ScoopStackmapElfAddrRanges *r,
                                           uintptr_t start,
                                           uintptr_t end) {
  if (r == 0) {
    return;
  }
  if (start == 0 || end <= start) {
    return;
  }
  if (r->len >= (sizeof(r->starts) / sizeof(r->starts[0]))) {
    return;
  }
  r->starts[r->len] = start;
  r->ends[r->len] = end;
  r->len++;
}

static int scoop_stackmap_elf_addr_in_ranges(const ScoopStackmapElfAddrRanges *r, uintptr_t addr) {
  if (r == 0 || addr == 0) {
    return 0;
  }
  for (size_t i = 0; i < r->len; i++) {
    if (addr >= r->starts[i] && addr < r->ends[i]) {
      return 1;
    }
  }
  return 0;
}

typedef struct ScoopStackmapElfScanCtx {
  uint32_t total_added;
} ScoopStackmapElfScanCtx;

static int scoop_stackmap_elf_scan_one_image(struct dl_phdr_info *info,
                                             size_t info_size,
                                             void *data) {
  (void)info_size;
  ScoopStackmapElfScanCtx *ctx = (ScoopStackmapElfScanCtx *)data;
  if (ctx == 0 || info == 0 || info->dlpi_phdr == 0) {
    return 0;
  }

  ScoopStackmapElfAddrRanges exec = {0};
  ScoopStackmapElfAddrRanges scan = {0};

  for (uint16_t i = 0; i < info->dlpi_phnum; i++) {
    const ElfW(Phdr) *ph = &info->dlpi_phdr[i];
    if (ph->p_type != PT_LOAD) {
      continue;
    }
    const uintptr_t start = (uintptr_t)info->dlpi_addr + (uintptr_t)ph->p_vaddr;
    const uintptr_t end = start + (uintptr_t)ph->p_memsz;
    if (end <= start) {
      continue;
    }

    if (ph->p_flags & PF_X) {
      scoop_stackmap_elf_ranges_push(&exec, start, end);
    }
    // stackmaps 通常位于 rodata / relro 等可读数据段；不应在可执行段中。
    if ((ph->p_flags & PF_R) && !(ph->p_flags & PF_X)) {
      scoop_stackmap_elf_ranges_push(&scan, start, end);
    }
  }

  if (exec.len == 0 || scan.len == 0) {
    return 0;
  }

  // 说明：
  // - ELF 在运行期通常无法通过 section header table 按名字定位 `.llvm_stackmaps`；
  // - 这里采用“在可读非可执行段内扫描 stackmap header” 的 best-effort 策略：
  //   - 先用 header 前 4 bytes（v3 常见为 `03 00 00 00`）做快速过滤；
  //   - 成功 parse 后再验证：至少有 1 条 record 的 `function_address` 落在本 image 的可执行段范围内；
  //   - 验证通过后注册该段作为 stackmap section（一个 image 通常只包含 1 段 stackmaps）。
  for (size_t si = 0; si < scan.len; si++) {
    const uint8_t *seg_bytes = (const uint8_t *)scan.starts[si];
    const size_t seg_len = (size_t)(scan.ends[si] - scan.starts[si]);
    if (seg_bytes == 0 || seg_len < 16u) {
      continue;
    }

    // 8 字节步长：stackmap records 在多数平台按 8 对齐；同时加速扫描。
    for (size_t off = 0; off + 16u <= seg_len; off += 8u) {
      const uint8_t *p = seg_bytes + off;
      if (!(p[0] == 3u && p[1] == 0u && p[2] == 0u && p[3] == 0u)) {
        continue;
      }

      ScoopStackmapRecordRef *entries = 0;
      size_t entry_len = 0;
      const uint32_t parsed =
          scoop_stackmap_parse_section(p, seg_len - off, &entries, &entry_len);
      if (parsed == 0 || entries == 0 || entry_len == 0) {
        continue;
      }

      int ok = 0;
      for (size_t ei = 0; ei < entry_len; ei++) {
        if (scoop_stackmap_elf_addr_in_ranges(&exec, entries[ei].function_address)) {
          ok = 1;
          break;
        }
      }

      if (!ok) {
        free(entries);
        continue;
      }

      ctx->total_added += scoop_stackmap_registry_register_entries(entries, entry_len);
      free(entries);
      // 一个 image 只注册一次（避免在 rodata 里误命中多个 header，导致重复扫描与重复注册）。
      return 0;
    }
  }

  return 0;
}

static uint32_t scoop_stackmap_registry_register_all_elf_images(void) {
  ScoopStackmapElfScanCtx ctx = {0};
  (void)dl_iterate_phdr(scoop_stackmap_elf_scan_one_image, &ctx);
  return ctx.total_added;
}
#endif

#if defined(_WIN32)
static int scoop_stackmap_coff_name_matches(const char *name) {
  if (name == 0 || name[0] == 0) {
    return 0;
  }
  // PE/COFF 的 section name 只有 8 bytes；长名字可能被截断或通过 string table 引用。
  // 这里做 best-effort：
  // - 精确匹配 `.llvm_stackmaps`
  // - 或匹配被截断的常见前缀 `.llvm_st`
  if (strcmp(name, ".llvm_stackmaps") == 0 || strcmp(name, "__llvm_stackmaps") == 0) {
    return 1;
  }
  if (strncmp(name, ".llvm_st", 7) == 0) {
    return 1;
  }
  return 0;
}

static uint32_t scoop_stackmap_registry_register_pe_image(const uint8_t *base) {
  if (base == 0) {
    return 0;
  }

  const IMAGE_DOS_HEADER *dos = (const IMAGE_DOS_HEADER *)base;
  if (dos->e_magic != IMAGE_DOS_SIGNATURE) {
    return 0;
  }

  const uint8_t *nt_ptr = base + (size_t)dos->e_lfanew;
  const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)nt_ptr;
  if (nt->Signature != IMAGE_NT_SIGNATURE) {
    return 0;
  }

  const IMAGE_FILE_HEADER *file = &nt->FileHeader;
  const IMAGE_SECTION_HEADER *sect = IMAGE_FIRST_SECTION(nt);
  if (sect == 0) {
    return 0;
  }

  uint32_t total_added = 0;
  for (uint16_t i = 0; i < file->NumberOfSections; i++) {
    char name_buf[9] = {0};
    memcpy(name_buf, sect[i].Name, 8);
    name_buf[8] = 0;

    // 若 name_buf 形如 "/123"，理论上表示 string table 偏移；但 PE image 通常不带 symbol table。
    // 这里仅做保守处理：直接当作非目标 section。
    if (!scoop_stackmap_coff_name_matches(name_buf)) {
      continue;
    }

    const uintptr_t va = (uintptr_t)sect[i].VirtualAddress;
    const size_t vsize =
        (sect[i].Misc.VirtualSize != 0) ? (size_t)sect[i].Misc.VirtualSize
                                        : (size_t)sect[i].SizeOfRawData;
    if (vsize == 0) {
      continue;
    }
    const uint8_t *bytes = base + (size_t)va;
    total_added += scoop_stackmap_registry_register_section(bytes, vsize);
  }

  return total_added;
}

static uint32_t scoop_stackmap_registry_register_all_pe_modules(void) {
  uint32_t total_added = 0;

  const DWORD pid = GetCurrentProcessId();
  const HANDLE snap =
      CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
  if (snap == INVALID_HANDLE_VALUE) {
    HMODULE self = GetModuleHandleA(0);
    if (self != 0) {
      total_added += scoop_stackmap_registry_register_pe_image((const uint8_t *)self);
    }
    return total_added;
  }

  MODULEENTRY32 me = {0};
  me.dwSize = sizeof(me);
  if (Module32First(snap, &me)) {
    do {
      total_added += scoop_stackmap_registry_register_pe_image((const uint8_t *)me.modBaseAddr);
    } while (Module32Next(snap, &me));
  }
  (void)CloseHandle(snap);

  return total_added;
}
#endif

uint32_t scoop_stackmap_registry_register_current_process(void) {
  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);
  if (scoop_stackmap_registry.registered_current_process) {
    (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
    return 0;
  }
  scoop_stackmap_registry.registered_current_process = 1;
  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);

  uint32_t total_added = 0;

#if defined(__APPLE__)
  // macOS：遍历 dyld images，按 segment/section 名称定位 stackmaps。
#if defined(__LP64__)
  // 先尝试直接从主程序（`_mh_execute_header`）定位 stackmaps section：
  // - 该路径不依赖 dyld image 索引，便于在极早期阶段快速确认“main binary 可发现”；
  // - section 数据地址由 getsectiondata() 解析，避免直接读取已废弃的 section header API。
  extern const struct mach_header_64 _mh_execute_header;
  uint32_t main_index = UINT32_MAX;
#endif

  const uint32_t image_count = _dyld_image_count();
  for (uint32_t i = 0; i < image_count; i++) {
    const struct mach_header *hdr = _dyld_get_image_header(i);
    if (hdr == 0) {
      continue;
    }

#if defined(__LP64__)
    // 记录主程序的 dyld image index（避免假设 main 固定为 0）。
    if (main_index == UINT32_MAX && hdr == (const struct mach_header *)&_mh_execute_header) {
      main_index = i;
    }

    // getsectiondata() 是 macOS 13+ 推荐接口，返回已可读取的 section 数据地址。
    unsigned long sect_size = 0;
    const uint8_t *data = getsectiondata((const struct mach_header_64 *)hdr,
                                         "__LLVM_STACKMAPS",
                                         "__llvm_stackmaps",
                                         &sect_size);
    if (data == 0 || sect_size == 0) {
      continue;
    }
    total_added += scoop_stackmap_registry_register_section(data, (size_t)sect_size);
#else
    // 早期阶段仅支持 64-bit host。
    (void)hdr;
#endif
  }

#if defined(__LP64__)
  // 若主程序尚未注册（例如遍历 dyld images 时跳过/未命中），在这里补一次“主程序优先”注册。
  if (main_index != UINT32_MAX) {
    unsigned long main_sect_size = 0;
    const uint8_t *main_data = getsectiondata(&_mh_execute_header,
                                              "__LLVM_STACKMAPS",
                                              "__llvm_stackmaps",
                                              &main_sect_size);
    if (main_data != 0 && main_sect_size != 0) {
      total_added +=
          scoop_stackmap_registry_register_section(main_data, (size_t)main_sect_size);
    }
  }
#endif
#elif defined(_WIN32)
  // Windows/COFF：遍历进程中已加载 modules，定位并注册 `.llvm_stackmaps` section（T1504b）。
  total_added += scoop_stackmap_registry_register_all_pe_modules();
#else
  // ELF：尝试使用 GNU ld/LLD 的 `__start_/__stop_` section symbols。
  //
  // 注意：
  // - section 名为 `.llvm_stackmaps` 时，对应的 symbol 为 `__start_llvm_stackmaps`/`__stop_llvm_stackmaps`。
  // - 这些符号是否存在取决于 link editor；因此这里用 weak 引用，缺失则跳过。
  extern const uint8_t __start_llvm_stackmaps[] __attribute__((weak));
  extern const uint8_t __stop_llvm_stackmaps[] __attribute__((weak));

  if (__start_llvm_stackmaps != 0 && __stop_llvm_stackmaps != 0) {
    const size_t sect_len = (size_t)(__stop_llvm_stackmaps - __start_llvm_stackmaps);
    if (sect_len > 0) {
      total_added += scoop_stackmap_registry_register_section(__start_llvm_stackmaps, sect_len);
    }
  }

#if defined(__linux__)
  // Linux：补齐 “多 image（主程序 + shared libs）” 的 best-effort 自动发现（T1504b）。
  total_added += scoop_stackmap_registry_register_all_elf_images();
#endif
#endif

  return total_added;
}

uint32_t scoop_stackmap_registry_record_count(void) {
  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);
  const uint32_t out = (uint32_t)scoop_stackmap_registry.len;
  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
  return out;
}

uint32_t scoop_stackmap_registry_lookup(uintptr_t return_address, ScoopStackmapRecordRef *out) {
  if (out == 0) {
    return 0;
  }

  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);

  // A2：无歧义命中（no best-effort nearest-match）
  //
  // 约定：
  // - 输入 `return_address` 来自 unwind/栈帧保存的 RA（通常为 “call 之后的下一条指令”）；
  // - registry 的 key 定义必须与该语义一致；在过渡期允许少量“确定性归一化”以兼容平台差异，
  //   但不允许“在窗口内挑一个最近的 record”（会导致相邻 records 误配）。
  //
  // 归一化候选（按常见差异收敛为小集合）：
  // - `ra`：理想情况（完全一致）；
  // - `ra±1`：部分 unwind 实现返回 “call 指令内部地址”（常见为 ra-1）；
  // - 注意：这里**不**做 `ra - call_len` 之类的“callsite ↔ return address”归一化：
  //   - 真实产物中 LLVM stackmap v3 的 `InstructionOffset` 在我们目标平台上对应的是
  //     **return address（call 之后的下一条指令）**；
  //   - 做 `ra - call_len` 会在“相邻 records（连续 calls）”场景下引入歧义（例如 ra-4
  //     刚好命中前一个 record 的 return address），导致 lookup 反而失败或误配；
  //   - 如果某平台 unwind 提供的是 callsite 而不是 return address，应在 platform/unwind
  //     层统一修正（见 GC-FIX Phase A3）。
  uintptr_t candidates[8];
  size_t cand_len = 0;

  // local helper：去重插入。
  #define PUSH_CAND(v) \
    do { \
      const uintptr_t _v = (uintptr_t)(v); \
      if (_v == 0) { \
        break; \
      } \
      int _dup = 0; \
      for (size_t _i = 0; _i < cand_len; _i++) { \
        if (candidates[_i] == _v) { \
          _dup = 1; \
          break; \
        } \
      } \
      if (!_dup && cand_len < (sizeof(candidates) / sizeof(candidates[0]))) { \
        candidates[cand_len++] = _v; \
      } \
    } while (0)

  PUSH_CAND(return_address);
  if (return_address > 1u) {
    PUSH_CAND(return_address - 1u);
  }
  if (return_address < (UINTPTR_MAX - 1u)) {
    PUSH_CAND(return_address + 1u);
  }

  // call_len candidates intentionally omitted (see comment above).

  // exact lookup helper
  size_t hits[8];
  size_t hit_len = 0;

  for (size_t ci = 0; ci < cand_len; ci++) {
    const uintptr_t key = candidates[ci];

    size_t left = 0;
    size_t right = scoop_stackmap_registry.len;
    while (left < right) {
      const size_t mid = left + (right - left) / 2u;
      const uintptr_t mid_ra = scoop_stackmap_registry.entries[mid].return_address;
      if (mid_ra < key) {
        left = mid + 1u;
      } else {
        right = mid;
      }
    }

    if (left < scoop_stackmap_registry.len &&
        scoop_stackmap_registry.entries[left].return_address == key) {
      if (hit_len < (sizeof(hits) / sizeof(hits[0]))) {
        hits[hit_len++] = left;
      }
    }
  }

  if (hit_len == 0) {
    (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
    return 0;
  }

  const ScoopStackmapRecordRef first = scoop_stackmap_registry.entries[hits[0]];
  for (size_t i = 1; i < hit_len; i++) {
    const ScoopStackmapRecordRef *cur = &scoop_stackmap_registry.entries[hits[i]];
    if (!scoop_stackmap_record_identity_eq(&first, cur)) {
      if (scoop_stackmap_strict_mode_enabled()) {
        (void)fprintf(stderr,
                      "[scooprt][stackmap] ambiguous lookup: ra=0x%lx hits=%zu\n",
                      (unsigned long)return_address,
                      hit_len);
        for (size_t hi = 0; hi < hit_len; hi++) {
          const ScoopStackmapRecordRef *e = &scoop_stackmap_registry.entries[hits[hi]];
          (void)fprintf(stderr,
                        "  - key=0x%lx func=0x%lx inst_off=0x%x patchpoint_id=%llu\n",
                        (unsigned long)e->return_address,
                        (unsigned long)e->function_address,
                        (unsigned)e->instruction_offset,
                        (unsigned long long)e->patchpoint_id);
        }
        abort();
      }
      (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
      return 0;
    }
  }

  *out = first;
  // API 语义：返回的 record 中 `return_address` 代表“该栈帧真实保存的 RA”。
  // 即使 registry key 与之存在可归一化差异（callsite/-1 等），也应在输出中回填为输入 RA。
  out->return_address = return_address;

  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
  return 1;

  #undef PUSH_CAND
}

void scoop_stackmap_registry_reset(void) {
  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);

  free(scoop_stackmap_registry.entries);
  scoop_stackmap_registry.entries = 0;
  scoop_stackmap_registry.len = 0;
  scoop_stackmap_registry.cap = 0;
  scoop_stackmap_registry.registered_current_process = 0;

  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
}
