import ipaddress

import http_peer
import tls_peer
import wire


def serve(connection, mode, target, contexts):
    authentication = mode == "socks-auth"
    assert wire.exact(connection, 3) == bytes((5, 1, 2 if authentication else 0))
    connection.sendall(bytes((5, 2 if authentication else 0)))
    if authentication:
        assert wire.exact(connection, 1) == b"\x01"
        username = wire.exact(connection, wire.exact(connection, 1)[0])
        password = wire.exact(connection, wire.exact(connection, 1)[0])
        assert username == "代理".encode() and password == b"secret\0"
        connection.sendall(b"\x01\0")
    head = wire.exact(connection, 4)
    assert head[:3] == b"\x05\x01\0"
    kind = head[3]
    count = {1: 4, 4: 16}.get(kind)
    if kind == 3:
        count = wire.exact(connection, 1)[0]
    assert count is not None
    address = wire.exact(connection, count)
    port = int.from_bytes(wire.exact(connection, 2), "big")
    url = http_peer.parts(target)[0]
    assert port == url.port
    if mode == "socks-local":
        assert kind in (1, 4) and ipaddress.ip_address(address).is_loopback
    elif mode == "socks-ipv6":
        assert kind == 4 and address == ipaddress.ip_address("::1").packed
    else:
        assert kind == 3 and address == url.hostname.encode("ascii"), (kind, address, url)
    if mode == "socks-deadline":
        wire.closed(connection)
        return
    if mode == "socks-reject":
        connection.sendall(b"\x05\x05\0\x01")
        wire.closed(connection)
        return
    connection.sendall(
        b"\x05\0\0\x04"
        + ipaddress.ip_address("::1").packed
        + b"\x10\xe1"
        + (http_peer.SETTINGS if mode == "socks-h2" else b"")
    )
    if mode == "socks-h2":
        http_peer.h2(connection, True)
        return
    if mode == "socks-tls":
        connection = tls_peer.TlsLayer(connection, contexts["http/1.1"])
    http_peer.request(connection, target)
    http_peer.response(connection)
    wire.closed(connection)
