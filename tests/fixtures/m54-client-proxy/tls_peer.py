import ssl
from pathlib import Path

from network_support import pem_function


def contexts(server_source, intermediate_source, directory):
    directory.mkdir(parents=True, exist_ok=True)
    chain, key = directory / "chain.pem", directory / "key.pem"
    chain.write_text(
        pem_function(Path(server_source), "serverPem")
        + pem_function(Path(intermediate_source), "intermediatePem")
    )
    key.write_text(pem_function(Path(server_source), "serverKeyPem"))
    result = {}
    for protocol in ("http/1.1", "h2"):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(chain, key)
        context.set_alpn_protocols([protocol])
        result[protocol] = context
    return result


class TlsLayer:
    def __init__(self, transport, context):
        self.transport = transport
        self.incoming = ssl.MemoryBIO()
        self.outgoing = ssl.MemoryBIO()
        self.ssl = context.wrap_bio(self.incoming, self.outgoing, server_side=True)
        self._drive(self.ssl.do_handshake)

    def _flush(self):
        while self.outgoing.pending:
            self.transport.sendall(self.outgoing.read())

    def _drive(self, operation, *arguments):
        while True:
            try:
                result = operation(*arguments)
                self._flush()
                return result
            except ssl.SSLWantReadError:
                self._flush()
                value = self.transport.recv(16384)
                if value:
                    self.incoming.write(value)
                else:
                    self.incoming.write_eof()
            except ssl.SSLWantWriteError:
                self._flush()

    def recv(self, count):
        try:
            return self._drive(self.ssl.read, count)
        except (ssl.SSLEOFError, ssl.SSLZeroReturnError):
            return b""

    def sendall(self, data):
        offset = 0
        while offset < len(data):
            offset += self._drive(self.ssl.write, data[offset:])


def untrusted(transport, context):
    try:
        connection = TlsLayer(transport, context)
    except ssl.SSLError:
        return
    try:
        value = connection.recv(1)
    except ssl.SSLError:
        return
    assert value == b"", "untrusted TLS connection sent application bytes"
