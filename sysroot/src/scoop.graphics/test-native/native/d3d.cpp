#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <cstdint>
#include <d3d12.h>
#include <dxgi1_4.h>
#include <memory>
#include <windows.h>
#include <wrl/client.h>

using Microsoft::WRL::ComPtr;

struct TestSurface {
  ComPtr<IDXGIAdapter1> adapter;
  ComPtr<ID3D12Device> device;
  ComPtr<ID3D12CommandQueue> queue;
  ComPtr<ID3D12Resource> texture;
};

extern "C" TestSurface *scoop_graphics_test_d3d_create(int32_t width,
                                                       int32_t height) {
  try {
    auto surface = std::make_unique<TestSurface>();
    ComPtr<IDXGIFactory1> factory;
    if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory))))
      return nullptr;
    for (UINT i = 0;; ++i) {
      HRESULT result = factory->EnumAdapters1(i, &surface->adapter);
      if (result == DXGI_ERROR_NOT_FOUND)
        break;
      if (FAILED(result))
        return nullptr;
      DXGI_ADAPTER_DESC1 desc;
      if (FAILED(surface->adapter->GetDesc1(&desc)))
        return nullptr;
      if (!(desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) &&
          SUCCEEDED(D3D12CreateDevice(surface->adapter.Get(),
                                      D3D_FEATURE_LEVEL_11_0,
                                      IID_PPV_ARGS(&surface->device))))
        break;
      surface->adapter.Reset();
    }
    if (!surface->device)
      return nullptr;
    D3D12_COMMAND_QUEUE_DESC queue = {};
    queue.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    if (FAILED(surface->device->CreateCommandQueue(
            &queue, IID_PPV_ARGS(&surface->queue))))
      return nullptr;
    D3D12_HEAP_PROPERTIES heap = {};
    heap.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC texture = {};
    texture.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    texture.Width = static_cast<UINT64>(width);
    texture.Height = static_cast<UINT>(height);
    texture.DepthOrArraySize = 1;
    texture.MipLevels = 1;
    texture.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    texture.SampleDesc.Count = 1;
    texture.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    if (FAILED(surface->device->CreateCommittedResource(
            &heap, D3D12_HEAP_FLAG_NONE, &texture, D3D12_RESOURCE_STATE_COMMON,
            nullptr, IID_PPV_ARGS(&surface->texture))))
      return nullptr;
    return surface.release();
  } catch (...) {
    return nullptr;
  }
}

extern "C" void *scoop_graphics_test_d3d_object(TestSurface *surface,
                                                int32_t kind) {
  switch (kind) {
  case 0:
    return surface->adapter.Get();
  case 1:
    return surface->device.Get();
  case 2:
    return surface->queue.Get();
  case 3:
    return surface->texture.Get();
  default:
    return nullptr;
  }
}

extern "C" void scoop_graphics_test_d3d_destroy(TestSurface *surface) {
  delete surface;
}
