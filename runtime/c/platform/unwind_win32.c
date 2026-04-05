// Windows backend for Scoop C runtime platform unwind layer (v0).
//
// 目标（对应 GC-FIX-TODO A3 / TODO T1411b）：
// - 为 GC stack walking 提供“可回归、可解释”的帧序列输入（至少 x86_64）；
// - ctx capture/frames walk 只服务于“从被 park 的线程捕获到的 ctx 枚举帧信息”；
// - 仍保持内部链接（static），避免引入新的导出符号。
//
// 实现说明：
// - `capture_ips`：使用 `RtlCaptureStackBackTrace` 采样 instruction pointers（仅诊断用途）。
// - `ctx_capture/ctx_walk_frames`：使用 `RtlCaptureContext` + `RtlVirtualUnwind` 逐帧展开，
//   输出 `(sp, ra, fp)`：
//   - `sp`：CONTEXT.Rsp（视为 CFA）；
//   - `ra`：CONTEXT.Rip（作为 stackmap registry lookup 的 return address 输入）；
//   - `fp`：CONTEXT.Rbp（可能为 0；是否需要由 stackmap locations 决定）。

#include <stdlib.h>

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#if defined(_M_X64) || defined(__x86_64__)
#define SCOOP_UNWIND_WIN64_SUPPORTED 1
#else
#define SCOOP_UNWIND_WIN64_SUPPORTED 0
#endif

#define SCOOP_UNWIND_CTX_MAX_FRAMES 64

typedef struct ScoopPlatformUnwindFrame {
  uintptr_t sp;
  uintptr_t ra;
  uintptr_t fp;
} ScoopPlatformUnwindFrame;

typedef struct ScoopPlatformUnwindOpaqueCtx {
  uint32_t frame_len;
  ScoopPlatformUnwindFrame frames[SCOOP_UNWIND_CTX_MAX_FRAMES];
} ScoopPlatformUnwindOpaqueCtx;

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                                      uint32_t out_cap,
                                                                      uint32_t skip_frames) {
  if (out_ips == 0 || out_cap == 0) {
    return 0;
  }

  // `RtlCaptureStackBackTrace` 返回每一帧的 instruction pointer（实现相关，不承诺稳定性）。
  //
  // 备注：
  // - MS 文档中该 API 的上层名字是 `CaptureStackBackTrace`；
  // - 这里使用 Rtl 前缀可避免额外包含/链接差异（两者位于 Kernel32/NTDLL 相关实现中）。
  void *tmp[SCOOP_UNWIND_CTX_MAX_FRAMES];
  const uint16_t cap = (out_cap > SCOOP_UNWIND_CTX_MAX_FRAMES) ? (uint16_t)SCOOP_UNWIND_CTX_MAX_FRAMES
                                                               : (uint16_t)out_cap;
  const uint16_t skip = (skip_frames > UINT16_MAX) ? UINT16_MAX : (uint16_t)skip_frames;
  const USHORT n = RtlCaptureStackBackTrace(skip, cap, tmp, 0);
  const uint32_t out_n = (n > cap) ? cap : (uint32_t)n;
  for (uint32_t i = 0; i < out_n; i++) {
    out_ips[i] = (uintptr_t)tmp[i];
  }
  return out_n;
}

static SCOOP_UNWIND_UNUSED void *scoop_platform_unwind_ctx_capture(void) {
  if (!SCOOP_UNWIND_WIN64_SUPPORTED) {
    return 0;
  }

  ScoopPlatformUnwindOpaqueCtx *out =
      (ScoopPlatformUnwindOpaqueCtx *)malloc(sizeof(ScoopPlatformUnwindOpaqueCtx));
  if (out == 0) {
    return 0;
  }

  out->frame_len = 0;
  for (uint32_t i = 0; i < SCOOP_UNWIND_CTX_MAX_FRAMES; i++) {
    out->frames[i].sp = 0;
    out->frames[i].ra = 0;
    out->frames[i].fp = 0;
  }

  CONTEXT ctx;
  RtlCaptureContext(&ctx);

  // `RtlVirtualUnwind` 要求 ContextFlags 至少包含 control registers。
  ctx.ContextFlags = CONTEXT_ALL;

  uintptr_t last_sp = 0;

  for (uint32_t i = 0; i < SCOOP_UNWIND_CTX_MAX_FRAMES; i++) {
    const uintptr_t sp = (uintptr_t)ctx.Rsp;
    const uintptr_t ra = (uintptr_t)ctx.Rip;
    const uintptr_t fp = (uintptr_t)ctx.Rbp;

    if (sp == 0 || ra == 0) {
      break;
    }

    // 健壮性：对齐 POSIX 的回归约束，期望 outer frames 的 CFA 单调不减。
    if (out->frame_len > 0 && sp < last_sp) {
      break;
    }
    last_sp = sp;

    out->frames[out->frame_len].sp = sp;
    out->frames[out->frame_len].ra = ra;
    out->frames[out->frame_len].fp = fp;
    out->frame_len += 1;

    DWORD64 image_base = 0;
    PRUNTIME_FUNCTION runtime_fn = RtlLookupFunctionEntry((DWORD64)ctx.Rip, &image_base, 0);
    if (runtime_fn == 0) {
      // leaf：没有 unwind info；按常见 x64 ABI 直接从栈上弹出 return address。
      //
      // 说明：
      // - 这里的 `Rsp` 是当前帧的栈顶；其首个 qword 通常是返回地址；
      // - 若栈不可读会触发异常；作为 v0，我们选择 best-effort（上层还有 capability gating）。
      const uintptr_t cur_sp = (uintptr_t)ctx.Rsp;
      if (cur_sp == 0) {
        break;
      }
      const uintptr_t ret = *(const uintptr_t *)cur_sp;
      if (ret == 0) {
        break;
      }
      ctx.Rip = (DWORD64)ret;
      ctx.Rsp = (DWORD64)(cur_sp + (uintptr_t)sizeof(void *));
      continue;
    }

    PVOID handler_data = 0;
    DWORD64 establisher_frame = 0;
    (void)RtlVirtualUnwind(UNWIND_FLAG_NHANDLER,
                           image_base,
                           (DWORD64)ctx.Rip,
                           runtime_fn,
                           &ctx,
                           &handler_data,
                           &establisher_frame,
                           0);

    if (ctx.Rip == 0) {
      break;
    }
  }

  return (void *)out;
}

static SCOOP_UNWIND_UNUSED void scoop_platform_unwind_ctx_destroy(void *ctx) {
  if (ctx == 0) {
    return;
  }

  free(ctx);
}

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_ctx_walk_frames(
    void *ctx,
    uint32_t skip_frames,
    ScoopPlatformUnwindFrameVisitor visitor,
    void *user_data) {
  if (ctx == 0 || visitor == 0) {
    return 0;
  }

  ScoopPlatformUnwindOpaqueCtx *opaque = (ScoopPlatformUnwindOpaqueCtx *)ctx;
  uint32_t visited = 0;

  for (uint32_t i = 0; i < opaque->frame_len; i++) {
    if (skip_frames > 0) {
      skip_frames--;
      continue;
    }

    const uintptr_t sp = opaque->frames[i].sp;
    const uintptr_t ra = opaque->frames[i].ra;
    const uintptr_t fp = opaque->frames[i].fp;
    if (sp == 0 || ra == 0) {
      continue;
    }

    visited += 1;
    if (!visitor(sp, ra, fp, user_data)) {
      break;
    }
  }

  return visited;
}
