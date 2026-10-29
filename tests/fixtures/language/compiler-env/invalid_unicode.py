import os
import sys

os.environb[b"SCOOP_ENV_INVALID_UNICODE"] = b"\xff"
os.execv(sys.argv[1], sys.argv[1:])
