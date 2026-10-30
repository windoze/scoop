#include "../shared/graphics.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendContext.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendSurface.h"
#include "include/gpu/ganesh/mtl/GrMtlDirectContext.h"

extern "C" {
SGContext *scoop_graphics_metal_context(const void *device, const void *queue) {
  try {
    GrMtlBackendContext backend;
    backend.fDevice = sk_ret_cfp(device);
    backend.fQueue = sk_ret_cfp(queue);
    auto result = std::make_unique<SGContext>();
    result->context = GrDirectContexts::MakeMetal(backend);
    return result->context ? result.release() : nullptr;
  } catch (...) {
    return nullptr;
  }
}
SGTarget *scoop_graphics_metal_target(SGContext *context, const void *texture,
                                      int32_t width, int32_t height) {
  try {
    GrMtlTextureInfo info;
    info.fTexture = sk_ret_cfp(texture);
    auto backend = GrBackendRenderTargets::MakeMtl(width, height, info);
    auto result = std::make_unique<SGTarget>();
    result->context = context->context;
    result->surface = SkSurfaces::WrapBackendRenderTarget(
        context->context.get(), backend, kTopLeft_GrSurfaceOrigin,
        kBGRA_8888_SkColorType, SkColorSpace::MakeSRGB(), nullptr);
    return result->surface ? result.release() : nullptr;
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_metal_drain(SGContext *context) {
  try {
    return context->context->flushAndSubmit(GrSyncCpu::kYes).fSuccess;
  } catch (...) {
    return 0;
  }
}
}
