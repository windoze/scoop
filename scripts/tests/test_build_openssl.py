"""Compile the crypto instructions required by the Darwin OpenSSL recipe."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import build_openssl


@unittest.skipUnless(sys.platform == "darwin", "Apple Clang target defaults")
class DarwinOpenSSLTests(unittest.TestCase):
    def test_arm64_flags_preserve_aes_and_sha_assembly(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "crypto.S"
            source.write_text(
                ".text\n.globl _crypto_probe\n_crypto_probe:\n"
                "aese v0.16b, v1.16b\nsha256h q0, q1, v2.4s\nret\n"
            )
            result = subprocess.run(
                [
                    "xcrun",
                    "clang",
                    "-arch",
                    "arm64",
                    *build_openssl.compiler_flags("aarch64-apple-darwin"),
                    "-c",
                    str(source),
                    "-o",
                    str(work / "crypto.o"),
                ],
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue((work / "crypto.o").is_file())


if __name__ == "__main__":
    unittest.main()
