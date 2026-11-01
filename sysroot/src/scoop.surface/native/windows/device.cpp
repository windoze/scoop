#include "renderer.h"

SSUDevice::~SSUDevice() {
  if (completion)
    CloseHandle(completion);
}

HRESULT SSUDevice::initialize() {
  HRESULT result = CreateDXGIFactory2(0, IID_PPV_ARGS(&factory));
  if (FAILED(result))
    return result;
  HRESULT deviceResult = DXGI_ERROR_NOT_FOUND;
  for (UINT index = 0;; ++index) {
    ComPtr<IDXGIAdapter1> candidate;
    result = factory->EnumAdapters1(index, &candidate);
    if (result == DXGI_ERROR_NOT_FOUND)
      return deviceResult;
    if (FAILED(result))
      return result;
    DXGI_ADAPTER_DESC1 description{};
    result = candidate->GetDesc1(&description);
    if (FAILED(result))
      return result;
    if (description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)
      continue;
    deviceResult = D3D12CreateDevice(candidate.Get(), D3D_FEATURE_LEVEL_11_0,
                                     IID_PPV_ARGS(&device));
    if (SUCCEEDED(deviceResult)) {
      adapter = candidate;
      break;
    }
  }
  D3D12_COMMAND_QUEUE_DESC description{};
  description.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
  result = device->CreateCommandQueue(&description, IID_PPV_ARGS(&queue));
  if (SUCCEEDED(result))
    result =
        device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence));
  if (SUCCEEDED(result))
    result = DCompositionCreateDevice(nullptr, IID_PPV_ARGS(&composition));
  if (FAILED(result))
    return result;
  completion = CreateEventW(nullptr, FALSE, FALSE, nullptr);
  return completion ? S_OK : ssu_windows_error();
}

HRESULT SSUDevice::drain() {
  HRESULT result = device->GetDeviceRemovedReason();
  if (FAILED(result))
    return result;
  const uint64_t value = ++serial;
  result = queue->Signal(fence.Get(), value);
  if (FAILED(result))
    return result;
  if (fence->GetCompletedValue() < value) {
    result = fence->SetEventOnCompletion(value, completion);
    if (FAILED(result))
      return result;
    if (WaitForSingleObject(completion, INFINITE) != WAIT_OBJECT_0)
      return ssu_windows_error();
  }
  return device->GetDeviceRemovedReason();
}

extern "C" {
SSUResult scoop_surface_d3d_host_create() {
  auto host = std::unique_ptr<SSUDevice>(new (std::nothrow) SSUDevice);
  if (!host)
    return {nullptr, E_OUTOFMEMORY};
  HRESULT result = host->initialize();
  return FAILED(result) ? SSUResult{nullptr, result}
                        : SSUResult{host.release(), S_OK};
}

void *scoop_surface_d3d_host_object(SSUDevice *host, int32_t kind) {
  if (kind == 0)
    return host->adapter.Get();
  if (kind == 1)
    return host->device.Get();
  return host->queue.Get();
}

void scoop_surface_d3d_host_close(SSUDevice *host) { delete host; }
}
