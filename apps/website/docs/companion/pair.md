---
title: Pair a mobile device
description: Pair Volum on mobile with a Desktop host or dedicated server.
---

# Pair a mobile device

Pairing approves one device and gives it a unique revocable token. The QR payload expires after five minutes and stops working after one successful pairing.

## Prepare the host

For a Desktop host:

1. Open **Settings → Mobile access**.
2. Keep **Automatic** to detect Tailscale, or choose **Custom** and enter the private listener IP plus the separate address clients will use.
3. Enable Companion Access.
4. Select **Pair a device**. Edit the advertised address before showing the QR code if this client must use another route.

For `volumd`:

1. Open the server's `VOLUM_PUBLIC_URL` in a browser.
2. Sign in as the administrator.
3. Select **Pair device**.
4. Leave the QR dialog open while connecting the phone.

The older `VOLUM_PRINT_PAIRING=true` container-log workflow remains available as an emergency fallback.

## Scan the code

1. Connect the phone or tablet to the selected private network/VPN, or confirm the HTTPS address is reachable.
2. Open Volum.
3. Select **Scan desktop code**.
4. Allow camera access when prompted.
5. Point the camera at the host's QR code.
6. Volum verifies the server identity and connects automatically over Tailscale or HTTPS.
7. If that first connection fails, review or edit the detected address and select **Verify and connect** to retry without changing the one-time secret.

Private HTTP still requires explicit confirmation that you trust the LAN or VPN to protect the connection. Public HTTP is blocked.

If camera access is unavailable, choose **Enter pairing data manually** and paste the complete payload.

## Confirm the connection

Open **Settings** on mobile and verify:

- the expected host name;
- the server address;
- a connected project count;
- the expected protocol version.

If the address later changes but the same server is available by another route, edit it in mobile Settings. Volum calls `/identity` without a token and verifies the same server ID before saving or sending authenticated requests.

## Revoke or unpair

**Revoke from the host** when a device is lost or should no longer have access. Revocation takes effect on its next request.

**Unpair on mobile** to remove the local credential and server-specific caches. Unpairing on the phone does not remove the host's device record; revoke it there as well when access should end.

## Pairing failures

Create a fresh payload if the code expired or was already used. Check the selected network route, VPN/ACL and firewall rules, DNS resolution, and port `43821` where applicable.

See [Troubleshooting](../help/troubleshooting) for detailed checks.
