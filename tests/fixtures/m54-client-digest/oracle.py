import hashlib
import re
import unicodedata
import urllib.parse
import urllib.request


def quoted(value):
    if isinstance(value, str):
        value = value.encode("latin-1")
    return b'"' + value.replace(b"\\", b"\\\\").replace(b'"', b'\\"') + b'"'


def parameters(values):
    plain = {"algorithm", "qop", "nc", "stale", "charset", "userhash"}
    return b", ".join(
        name.encode() + b"=" + (value.encode() if name in plain else quoted(value))
        for name, value in values.items()
    )


def challenge(algorithm, qop, nonce=b'nonce,\xe9"x', **extra):
    values = {
        "realm": b"private",
        "nonce": nonce,
        "algorithm": algorithm,
        "opaque": b'opaque,\\"\xe9',
    }
    if qop:
        values["qop"] = qop
    values.update(extra)
    # The server's qop is a quoted list; other token parameters stay unquoted.
    qop_value = values.pop("qop", None)
    wire = b"Digest " + parameters(values)
    return wire + (b", qop=" + quoted(qop_value) if qop_value else b"")


def verify(header, method, target, body, algorithm, qop, nonce=b'nonce,\xe9"x', count=1, mode=""):
    scheme, raw = header.split(b" ", 1)
    assert scheme.lower() == b"digest", header
    parts = urllib.request.parse_http_list(raw.decode("latin-1"))
    values = urllib.request.parse_keqv_list(parts)
    values = {name: value.encode("latin-1") for name, value in values.items()}
    assert values["realm"] == b"private" and values["nonce"] == nonce, values
    assert values["opaque"] == b'opaque,\\"\xe9', values
    assert values["uri"] == target and values["algorithm"].decode().lower() == algorithm.lower(), (
        values
    )
    username = (
        "Jose\u0301" if mode in ("utf8", "userhash") else "café" if mode == "latin" else "Mufasa"
    )
    password = "pa\u0308ss:word" if mode in ("utf8", "userhash") else "Circle of Life"
    encoding = "utf-8" if mode in ("utf8", "userhash") else "latin-1"
    if encoding == "utf-8":
        username, password = [unicodedata.normalize("NFC", value) for value in (username, password)]
    name = algorithm.lower().removesuffix("-sess").replace("-", "")
    if name == "sha512256":
        name = "sha512_256"

    def hash_bytes(data):
        return hashlib.new(name, data).hexdigest().encode()

    if mode == "userhash":
        assert values["username"] == hash_bytes(username.encode(encoding) + b":private")
        assert values["userhash"] == b"true" and "username*" not in values
    elif mode in ("utf8", "latin"):
        assert "username" not in values
        charset, language, encoded = values["username*"].decode().split("'", 2)
        assert charset.lower() == "utf-8" and language == ""
        assert urllib.parse.unquote_to_bytes(encoded).decode("utf-8") == username
    else:
        assert values["username"] == username.encode("ascii") and "username*" not in values
    cnonce = values.get("cnonce", b"")
    if qop or algorithm.lower().endswith("-sess"):
        assert re.fullmatch(rb"[0-9a-f]{32}", cnonce), values
    if qop:
        assert values["qop"] == qop.encode() and values["nc"] == f"{count:08x}".encode(), values
    else:
        assert "qop" not in values and "nc" not in values
    for token in (b"algorithm", b"qop", b"nc"):
        assert not re.search(rb"\b" + token + rb'\s*=\s*"', raw), raw
    ha1 = hash_bytes(username.encode(encoding) + b":private:" + password.encode(encoding))
    if algorithm.lower().endswith("-sess"):
        ha1 = hash_bytes(ha1 + b":" + nonce + b":" + cnonce)

    def proof(verb, entity):
        a2 = verb + b":" + target
        if qop == "auth-int":
            a2 += b":" + hash_bytes(entity)
        middle = nonce + b":"
        if qop:
            middle += values["nc"] + b":" + cnonce + b":" + qop.encode() + b":"
        return hash_bytes(ha1 + b":" + middle + hash_bytes(a2))

    assert values["response"] == proof(method, body), values

    def response_info(entity=b"done", **extra):
        result = {"rspauth": proof(b"", entity), **extra}
        if qop:
            result.update(qop=qop, nc=values["nc"].decode(), cnonce=cnonce)
        return result

    return response_info
