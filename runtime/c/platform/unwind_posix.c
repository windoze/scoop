// POSIX backend for Scoop C runtime platform unwind layer (v0).
//
// 说明：
// - 这里优先使用 `_Unwind_Backtrace`：它属于 Itanium unwind ABI 的一部分，
//   在我们主线 host 平台（clang toolchain）上通常无需额外链接选项；
// - 该实现仅用于 current-thread backtrace；不提供跨线程/remote unwind。

#include <setjmp.h>
#include <stdlib.h>
#include <unwind.h>

#define SCOOP_UNWIND_CTX_MAX_FRAMES 64

typedef struct ScoopPlatformUnwindFrame {
  uintptr_t sp;
  uintptr_t ra;
} ScoopPlatformUnwindFrame;

typedef struct ScoopPlatformUnwindOpaqueCtx {
  // v0：用 `setjmp` 捕获当前线程的寄存器快照，作为后续 stack walking 的起点（T1505b/T1411b）。
  //
  // 说明：
  // - `ucontext_t/getcontext` 在 macOS 上需要额外 feature macro（且已废弃），因此这里先用
  //   标准 C 的 `jmp_buf` 作为可移植的 opaque ctx；
  // - 该结构的具体解读与“从 ctx 开始逐帧 unwind”仍由后续任务 T1411b 统一在 platform 层落地。
  jmp_buf regs;

  // T1411b：为保证“可回归且无额外依赖”，POSIX v0 先在捕获时用 `_Unwind_Backtrace`
  // 采样调用栈，并在 ctx 内缓存 `(sp, ra)` 列表；后续 stack walking 从该缓存中逐帧枚举。
  //
  // 说明：
  // - `sp` 取 `_Unwind_GetCFA`（canonical frame address）；`ra` 取 `_Unwind_GetIP`；
  // - 该缓存不承诺完整性/稳定性，只用于“stackmap 查询输入”的 early-stage 回归；
  // - 后续若引入真正的 remote unwind（从寄存器快照开始），可在不改变上层调用点的前提下
  //   替换该实现细节。
  uint32_t frame_len;
  ScoopPlatformUnwindFrame frames[SCOOP_UNWIND_CTX_MAX_FRAMES];
} ScoopPlatformUnwindOpaqueCtx;

typedef struct ScoopUnwindCaptureCtx {
  uintptr_t *out_ips;
  uint32_t cap;
  uint32_t len;
  uint32_t skip;
} ScoopUnwindCaptureCtx;

static _Unwind_Reason_Code scoop_unwind_capture_cb(struct _Unwind_Context *context, void *arg) {
  if (arg == 0) {
    return _URC_END_OF_STACK;
  }

  ScoopUnwindCaptureCtx *ctx = (ScoopUnwindCaptureCtx *)arg;
  if (ctx->out_ips == 0 || ctx->cap == 0) {
    return _URC_END_OF_STACK;
  }

  // `_Unwind_GetIP` 返回的是“下一条指令地址”（实现相关）；只用于诊断/定位而非执行。
  uintptr_t ip = (uintptr_t)_Unwind_GetIP(context);
  if (ip == 0) {
    return _URC_NO_REASON;
  }

  if (ctx->skip > 0) {
    ctx->skip--;
    return _URC_NO_REASON;
  }

  if (ctx->len >= ctx->cap) {
    return _URC_END_OF_STACK;
  }

  ctx->out_ips[ctx->len] = ip;
  ctx->len++;
  return _URC_NO_REASON;
}

typedef struct ScoopUnwindCaptureFramesCtx {
  ScoopPlatformUnwindOpaqueCtx *out_ctx;
  uint32_t len;
} ScoopUnwindCaptureFramesCtx;

static _Unwind_Reason_Code scoop_unwind_capture_frames_cb(struct _Unwind_Context *context,
                                                          void *raw_arg) {
  if (raw_arg == 0 || context == 0) {
    return _URC_END_OF_STACK;
  }

  ScoopUnwindCaptureFramesCtx *arg = (ScoopUnwindCaptureFramesCtx *)raw_arg;
  if (arg->out_ctx == 0) {
    return _URC_END_OF_STACK;
  }
  if (arg->len >= SCOOP_UNWIND_CTX_MAX_FRAMES) {
    return _URC_END_OF_STACK;
  }

  // `_Unwind_GetIP` 返回的是“下一条指令地址”（实现相关）；这里作为 stackmap lookup 的 return address 输入。
  const uintptr_t ra = (uintptr_t)_Unwind_GetIP(context);
  const uintptr_t sp = (uintptr_t)_Unwind_GetCFA(context);
  if (ra == 0 || sp == 0) {
    return _URC_NO_REASON;
  }

  arg->out_ctx->frames[arg->len].sp = sp;
  arg->out_ctx->frames[arg->len].ra = ra;
  arg->len += 1;
  return _URC_NO_REASON;
}

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                                      uint32_t out_cap,
                                                                      uint32_t skip_frames) {
  if (out_ips == 0 || out_cap == 0) {
    return 0;
  }

  ScoopUnwindCaptureCtx ctx = {
      .out_ips = out_ips,
      .cap = out_cap,
      .len = 0,
      .skip = skip_frames,
  };

  (void)_Unwind_Backtrace(scoop_unwind_capture_cb, (void *)&ctx);
  return ctx.len;
}

static SCOOP_UNWIND_UNUSED void *scoop_platform_unwind_ctx_capture(void) {
  ScoopPlatformUnwindOpaqueCtx *out =
      (ScoopPlatformUnwindOpaqueCtx *)malloc(sizeof(ScoopPlatformUnwindOpaqueCtx));
  if (out == 0) {
    return 0;
  }

  out->frame_len = 0;
  for (uint32_t i = 0; i < SCOOP_UNWIND_CTX_MAX_FRAMES; i++) {
    out->frames[i].sp = 0;
    out->frames[i].ra = 0;
  }

  // `setjmp` 返回 0 表示“直接保存”，非 0 表示“从 longjmp 恢复”。
  // 我们只需要保存快照，因此忽略返回值。
  (void)setjmp(out->regs);

  // 捕获 `(sp, ra)` 列表（best-effort）。
  ScoopUnwindCaptureFramesCtx frames = {
      .out_ctx = out,
      .len = 0,
  };
  (void)_Unwind_Backtrace(scoop_unwind_capture_frames_cb, (void *)&frames);
  out->frame_len = frames.len;

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
    if (sp == 0 || ra == 0) {
      continue;
    }

    visited += 1;
    if (!visitor(sp, ra, user_data)) {
      break;
    }
  }

  return visited;
}
