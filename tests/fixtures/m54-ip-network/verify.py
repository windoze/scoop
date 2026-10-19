import ipaddress
import random
import subprocess
import sys


def main():
    generator = random.Random(54)
    arguments = [sys.argv[1]]
    count = 0
    for address_type, bits in ((ipaddress.IPv4Address, 32), (ipaddress.IPv6Address, 128)):
        for prefix in range(bits + 1):
            address = address_type(generator.getrandbits(bits))
            text = f"{address}/{prefix}"
            network = ipaddress.ip_network(text, strict=False)
            candidates = [
                address,
                network.network_address,
                network.broadcast_address,
                address_type(generator.getrandbits(bits)),
            ]
            if prefix:
                candidates.append(
                    address_type(int(network.network_address) ^ (1 << (bits - prefix)))
                )
            for candidate in candidates:
                arguments.extend(
                    (text, str(candidate), str(candidate in network).lower(), str(network))
                )
                count += 1
    result = subprocess.run(arguments, capture_output=True, text=True, timeout=180)
    assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
    assert result.stdout == "IP networks match independent CIDR vectors and URL socket addresses\n"
    assert not result.stderr, result.stderr
    print(f"{count} independent IPv4 and IPv6 CIDR vectors passed")


if __name__ == "__main__":
    main()
