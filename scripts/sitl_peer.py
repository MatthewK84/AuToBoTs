"""Bind the PX4 SITL onboard port so the host can connect and heartbeat."""

import socket
import time

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("127.0.0.1", 14540))
sock.settimeout(0.5)
deadline = time.time() + 20
while time.time() < deadline:
    try:
        sock.recvfrom(2048)
    except TimeoutError:
        pass
