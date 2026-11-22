"""Check TCP/TLS reachability from another host, pinning the receiver certificate.

Set RECEIVER_PROBE_HOST and RECEIVER_CERT_SHA256 in the environment. No driver
credentials or telemetry are sent. Host addresses are omitted from output.
"""
import hashlib
import hmac
import json
import os
import re
import socket
import ssl
import time


def main():
    host = os.environ.get('RECEIVER_PROBE_HOST', '').strip()
    pin = os.environ.get('RECEIVER_CERT_SHA256', '').strip().lower()
    port = int(os.environ.get('RECEIVER_PROBE_PORT', '7443'))
    if not host or not re.fullmatch(r'[a-f0-9]{64}', pin) or not 1 <= port <= 65535:
        raise SystemExit('Set a receiver host, valid port and SHA-256 certificate fingerprint')
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE  # The required exact fingerprint is checked below.
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    failure = None
    for _ in range(3):
        try:
            with socket.create_connection((host, port), timeout=10) as tcp:
                with context.wrap_socket(tcp, server_hostname='lmu-engineer') as tls:
                    actual = hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest()
                    if not hmac.compare_digest(actual, pin):
                        raise SystemExit('Connected endpoint has a different certificate')
                    print(json.dumps({'tcp_reachable': True, 'receiver_certificate_verified': True,
                                      'tls_version': tls.version(), 'driver_credentials_sent': False}))
                    return
        except OSError as error:
            failure = type(error).__name__
            time.sleep(2)
    raise SystemExit('Receiver connectivity check failed: ' + str(failure))


if __name__ == '__main__':
    main()
