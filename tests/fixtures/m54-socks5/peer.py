import contextlib
import socket
import sys


def read_exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        if not part:
            raise EOFError("incomplete SOCKS5 request")
        result.extend(part)
    return bytes(result)


def expect_closed(connection):
    with contextlib.suppress(ConnectionResetError):
        assert connection.recv(1) == b""


def authenticate(connection, enabled, rejected):
    greeting = read_exact(connection, 2)
    assert greeting == b"\x05\x01"
    method = b"\x02" if enabled else b"\x00"
    assert read_exact(connection, 1) == method
    for byte in b"\x05" + method:
        connection.sendall(bytes([byte]))
    if not enabled:
        return True
    assert read_exact(connection, 1) == b"\x01"
    username = read_exact(connection, read_exact(connection, 1)[0])
    password = read_exact(connection, read_exact(connection, 1)[0])
    assert username == "用户".encode()
    assert password == b"\x00\xffp"
    connection.sendall(b"\x01\x09" if rejected else b"\x01\x00")
    if rejected:
        expect_closed(connection)
        return False
    return True


def read_destination(connection):
    version, command, reserved, kind = read_exact(connection, 4)
    assert (version, command, reserved) == (5, 1, 0)
    count = {1: 4, 4: 16}.get(kind)
    if kind == 3:
        count = read_exact(connection, 1)[0]
    assert count is not None and count > 0
    address = read_exact(connection, count)
    port = int.from_bytes(read_exact(connection, 2), "big")
    return kind, address, port


def exchange(connection, scenario):
    enabled = scenario in ("local", "auth-reject")
    if not authenticate(connection, enabled, scenario == "auth-reject"):
        return
    received = read_destination(connection)
    if scenario == "local":
        assert received == (1, socket.inet_pton(socket.AF_INET, "127.0.0.1"), 80)
        bound = b"\x01" + socket.inet_pton(socket.AF_INET, "192.0.2.1")
    elif scenario == "ipv6":
        assert received == (4, socket.inet_pton(socket.AF_INET6, "::1"), 8443)
        bound = b"\x04" + socket.inet_pton(socket.AF_INET6, "2001:db8::1")
    else:
        assert received == (3, b"example.invalid", 443)
        bound = b"\x03\x0bproxy-bound"
    status = b"\x05" if scenario == "connect-reject" else b"\x00"
    reply = b"\x05" + status + b"\x00" + bound + (3210).to_bytes(2, "big")
    if scenario == "connect-reject":
        connection.sendall(reply)
        expect_closed(connection)
        return
    connection.sendall(reply + b"prefix")
    assert read_exact(connection, 4) == b"PING"
    connection.sendall(b"PONG")
    connection.shutdown(socket.SHUT_WR)
    expect_closed(connection)


def main():
    scenario = sys.argv[1]
    assert scenario in ("domain", "local", "ipv6", "auth-reject", "connect-reject")
    with socket.socket() as listener:
        listener.settimeout(30)
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        print(listener.getsockname()[1], flush=True)
        connection, _ = listener.accept()
        with connection:
            connection.settimeout(30)
            exchange(connection, scenario)
    print("accepted " + scenario, flush=True)


if __name__ == "__main__":
    main()
