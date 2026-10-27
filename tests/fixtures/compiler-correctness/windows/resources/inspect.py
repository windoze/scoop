"""Read embedded RCDATA through the Windows resource API."""

import ctypes
import sys
from pathlib import Path


def main():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint32]
    kernel.LoadLibraryExW.restype = ctypes.c_void_p
    kernel.FindResourceW.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = ctypes.c_void_p
    kernel.SizeofResource.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.SizeofResource.restype = ctypes.c_uint32
    kernel.LoadResource.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.LoadResource.restype = ctypes.c_void_p
    kernel.LockResource.argtypes = [ctypes.c_void_p]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [ctypes.c_void_p]
    kernel.FreeLibrary.restype = ctypes.c_int
    handle = kernel.LoadLibraryExW(str(Path(sys.argv[1]).resolve()), None, 0x22)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        resource = kernel.FindResourceW(handle, int(sys.argv[2]), 10)
        if sys.argv[3] == "-":
            assert not resource, "unexpected production resource"
            sys.stdout.buffer.write(b"resource-absent\n")
            return
        if not resource:
            raise ctypes.WinError(ctypes.get_last_error())
        length = kernel.SizeofResource(handle, resource)
        pointer = kernel.LockResource(kernel.LoadResource(handle, resource))
        if length and not pointer:
            raise ctypes.WinError(ctypes.get_last_error())
        assert ctypes.string_at(pointer, length) == Path(sys.argv[3]).read_bytes()
        sys.stdout.buffer.write(b"resource-ok\n")
    finally:
        assert kernel.FreeLibrary(handle)


if __name__ == "__main__":
    main()
