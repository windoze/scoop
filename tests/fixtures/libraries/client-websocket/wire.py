import struct
import zlib


def exact(source, size):
    data = source.read(size)
    if len(data) != size:
        raise EOFError("short WebSocket frame")
    return data


def frame(opcode, data, compressed=False):
    size = len(data)
    head = bytes([128 | (64 if compressed else 0) | opcode])
    if size < 126:
        return head + bytes([size]) + data
    if size <= 65535:
        return head + b"\x7e" + struct.pack("!H", size) + data
    return head + b"\x7f" + struct.pack("!Q", size) + data


def read_frame(source):
    first, second = exact(source, 2)
    assert second & 128, "client frame must be masked"
    size = second & 127
    if size == 126:
        size = struct.unpack("!H", exact(source, 2))[0]
    elif size == 127:
        size = struct.unpack("!Q", exact(source, 8))[0]
    mask = exact(source, 4)
    data = exact(source, size)
    return first, bytes(value ^ mask[index % 4] for index, value in enumerate(data))


def message(source):
    chunks = []
    first, data = read_frame(source)
    opcode, compressed = first & 15, bool(first & 64)
    assert opcode in (1, 2)
    chunks.append(data)
    while not first & 128:
        first, data = read_frame(source)
        assert first & 15 == 0 and first & 64 == 0
        chunks.append(data)
    payload = b"".join(chunks)
    if compressed:
        payload = zlib.decompressobj(-15).decompress(payload + b"\x00\x00\xff\xff")
    return opcode, payload, compressed


def compressed_frame(text):
    encoder = zlib.compressobj(wbits=-15)
    data = encoder.compress(text) + encoder.flush(zlib.Z_SYNC_FLUSH)
    assert data.endswith(b"\x00\x00\xff\xff")
    return frame(1, data[:-4], True)
