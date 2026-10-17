# M51 test certificates

Fixed PEM inputs generated with the OpenSSL 3.6.5 CLI. The root is valid from
2024-01-01 through 2036-01-01, the intermediate through 2035-01-01, and the
server through 2034-01-01. The server SAN contains localhost, 127.0.0.1 and ::1.
The expired leaf ends on 2025-01-01; the client leaf only permits clientAuth.
The included private keys are test inputs. This ordinary Cone only depends on
core and is used through test-dependencies by crypto and TLS consumers.
