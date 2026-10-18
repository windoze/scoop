# TLS system trust test support

This ordinary test Cone supplies isolated platform anchors, native work gates,
and short-lived certificates signed in memory by the existing test CA. The
server templates keep their SAN and usage extensions. Valid test leaves run
from yesterday through seven days from now; expired leaves end yesterday.
Production TLS continues to check the current time and the platform's validity
policy, and these helpers never modify machine keychains or trust settings.

The fixed M51 certificates intentionally span multiple years and remain
unchanged. They are unsuitable as positive SecTrust TLS policy inputs because
macOS limits the validity period even when an anchor is supplied explicitly.
