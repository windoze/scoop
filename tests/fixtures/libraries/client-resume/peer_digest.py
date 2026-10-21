import hashlib
import urllib.request


def hash_text(value):
    return hashlib.sha256(value.encode()).hexdigest()


def authorize(peer):
    header = peer.headers.get("Authorization")
    if header is None:
        peer.reply(
            401,
            b"",
            [
                (
                    "WWW-Authenticate",
                    'Digest realm="resume", nonce="initial", algorithm=SHA-256, qop="auth-int"',
                )
            ],
        )
        return None
    scheme, text = header.split(" ", 1)
    assert scheme.lower() == "digest"
    values = urllib.request.parse_keqv_list(urllib.request.parse_http_list(text))
    assert values["username"] == "user" and values["realm"] == "resume"
    assert values["qop"] == "auth-int" and values["uri"] == peer.path
    assert values["nonce"] in ("initial", "next")
    ha1 = hash_text("user:resume:password")
    ha2 = hash_text(f"GET:{peer.path}:{hashlib.sha256(b'').hexdigest()}")
    prefix = f"{ha1}:{values['nonce']}:{values['nc']}:{values['cnonce']}:auth-int:"
    assert values["response"] == hash_text(prefix + ha2)
    return values, prefix


def response_info(auth, path, body):
    values, prefix = auth
    ha2 = hash_text(f":{path}:{hashlib.sha256(body).hexdigest()}")
    proof = hash_text(prefix + ha2)
    return (
        "Authentication-Info",
        f'rspauth="{proof}", qop=auth-int, cnonce="{values["cnonce"]}", nc={values["nc"]}, nextnonce="next"',
    )
