#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include "../shared/graphics.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendContext.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendSurface.h"
#include "include/gpu/ganesh/d3d/GrD3DDirectContext.h"

extern "C" {
SGContext *scoop_graphics_d3d_context(IDXGIAdapter1 *adapter,
                                      ID3D12Device *device,
                                      ID3D12CommandQueue *queue) {
  try {
    GrD3DBackendContext backend;
    backend.fAdapter.retain(adapter);
    backend.fDevice.retain(device);
    backend.fQueue.retain(queue);
    auto result = std::make_unique<SGContext>();
    result->context = GrDirectContexts::MakeD3D(backend);
    return result->context ? result.release() : nullptr;
  } catch (...) {
    return nullptr;
  }
}

SGTarget *scoop_graphics_d3d_target(SGContext *context, ID3D12Resource *texture,
                                    int32_t width, int32_t height) {
  try {
    const auto desc = texture->GetDesc();
    if (desc.Width != static_cast<UINT64>(width) ||
        desc.Height != static_cast<UINT>(height) ||
        desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM ||
        desc.SampleDesc.Count != 1 ||
        desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D ||
        !(desc.Flags & D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET))
      return nullptr;
    GrD3DTextureResourceInfo info;
    info.fResource.retain(texture);
    info.fResourceState = D3D12_RESOURCE_STATE_PRESENT;
    info.fFormat = desc.Format;
    info.fLevelCount = 1;
    auto backend = GrBackendRenderTargets::MakeD3D(width, height, info);
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

int32_t scoop_graphics_d3d_drain(SGContext *context) {
  try {
    return context->context->flushAndSubmit(GrSyncCpu::kYes).fSuccess;
  } catch (...) {
    return 0;
  }
}
}
