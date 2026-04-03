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

#include "scoop_stackmap.h"

#include <pthread.h>
#include <stdlib.h>
#include <string.h>

#if defined(__APPLE__)
#include <mach-o/dyld.h>
#include <mach-o/getsect.h>
#include <mach-o/loader.h>
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
      continue;
    }
    scoop_stackmap_registry.entries[write] = scoop_stackmap_registry.entries[read];
    write++;
  }
  scoop_stackmap_registry.len = write;
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
      // 注：不同后端/指令集上 “callsite offset vs return address” 可能存在 +call_size 的差异；
      // 早期阶段我们先固化该规则，并通过测试用例（同一二进制内的内嵌 section）回归它。
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

  (void)pthread_mutex_lock(&scoop_stackmap_registry_lock);

  if (!scoop_stackmap_registry_reserve_locked(entry_len)) {
    (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
    free(entries);
    return 0;
  }

  memcpy(&scoop_stackmap_registry.entries[scoop_stackmap_registry.len],
         entries,
         entry_len * sizeof(ScoopStackmapRecordRef));
  scoop_stackmap_registry.len += entry_len;

  scoop_stackmap_registry_sort_and_dedupe_locked();

  const uint32_t out_added = (uint32_t)entry_len;
  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);

  free(entries);
  return out_added;
}

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
  const uint32_t image_count = _dyld_image_count();
  for (uint32_t i = 0; i < image_count; i++) {
    const struct mach_header *hdr = _dyld_get_image_header(i);
    if (hdr == 0) {
      continue;
    }

#if defined(__LP64__)
    unsigned long sect_size = 0;
    const uint8_t *data =
        (const uint8_t *)getsectiondata((const struct mach_header_64 *)hdr,
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
#elif defined(_WIN32)
  // Windows/COFF：后续在需要时补齐（T1504 只要求 registry+解析器的最小闭环）。
  (void)total_added;
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

  size_t left = 0;
  size_t right = scoop_stackmap_registry.len;
  while (left < right) {
    const size_t mid = left + (right - left) / 2u;
    const uintptr_t mid_ra = scoop_stackmap_registry.entries[mid].return_address;
    if (mid_ra == return_address) {
      *out = scoop_stackmap_registry.entries[mid];
      (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
      return 1;
    }
    if (mid_ra < return_address) {
      left = mid + 1u;
    } else {
      right = mid;
    }
  }

  (void)pthread_mutex_unlock(&scoop_stackmap_registry_lock);
  return 0;
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
