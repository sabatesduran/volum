---
title: Troubleshooting
description: Diagnose pairing, network, offline, upload, mirror, and server problems.
---

# Troubleshooting

Start with the canonical host. If it is not healthy and reachable, client retries and new pairing codes will not help.

## A client cannot reach the host

1. Confirm Volum Desktop or the `volumd` container is running.
2. Open the readiness URL from the client network.
3. Verify the selected LAN, VPN, Tailscale, or HTTPS route is active on both devices.
4. Check VPN/ACL, reverse-proxy, and host-firewall rules for TCP port `43821` where applicable.
5. Confirm the configured URL is reachable by the client, not `0.0.0.0` or a container hostname.

```bash
curl --fail http://volum-host.example.ts.net:43821/readyz
```

Plain HTTP is accepted only when the address is private or the hostname resolves entirely to private IPs. Use HTTPS for public names or routes.

## Desktop Companion Access says it needs attention

- In Automatic mode, confirm Tailscale is installed, signed in, and connected.
- In Custom mode, confirm the listener IP belongs to this computer and is private, and confirm the advertised URL is reachable by clients.
- Turn Companion Access off and on after the selected network interface becomes available.
- Check whether another process is already using port `43821`.
- Review the error shown in **Settings → Mobile access**.
- Restart Volum after a major network-interface change.

## Pairing is rejected

- Create a fresh payload; pairing data expires after five minutes.
- Make sure the previous payload was not already used successfully.
- Copy the complete payload without adding spaces or line breaks.
- Check that the host and client clocks are reasonably synchronized.
- Confirm the client reached the expected `/identity` response.
- If the address was edited, confirm it is the same server identity. Volum refuses to send the secret to a different server.

Repeatedly retrying an expired payload does not extend it.

## A paired device was revoked

Revocation is immediate. Create a new pairing payload on the host and pair again only if the device should regain access.

If a mobile device was intentionally retired, unpair it locally to remove cached credentials and offline content.

## The Desktop server disappeared after closing Volum

Without menu-bar or system-tray mode, closing or quitting Volum stops the embedded server. Enable [menu-bar operation](../desktop/menu-bar) if the window should close while Companion Access continues.

After a reboot, open Volum again. Automatic start at login is not currently part of the public workflow.

## An edit is unavailable on mobile

Check **Allow organization changes** on the canonical Desktop or `VOLUM_ORGANIZATION_WRITES` on `volumd`. Edits require a live connection and are not queued offline.

A revision conflict means another client changed the record first. Refresh, review the current value, and retry intentionally.

Photo uploads use a separate permission. Check **Allow photo uploads** on Desktop or `VOLUM_PHOTO_UPLOADS` on `volumd`; organization changes can remain disabled.

## A file upload is unavailable

- Confirm the canonical library has managed storage.
- Enable organization writes.
- Use STL, 3MF, OBJ, STEP, or STP.
- Keep the app in the foreground until completion.
- Check the 500 MB mobile file limit and available server storage.
- Review the host log for validation or import failures.

## Mirrored files are missing offline

Only completed, verified mirror files appear offline. Reconnect and run **Sync now** from **Settings → Remote library**.

Look under `Conflicts/` when a local file and server revision changed independently. Volum preserves conflicts rather than choosing one silently.

## A NAS migration is interrupted

- Keep the Desktop library and NAS staging directories unchanged.
- Confirm the same `volumd` server is healthy and reachable.
- Return to **Settings → Advanced → Library hosting → Move this library to a NAS**.
- Select **Resume migration**. Verified chunks and complete files are not sent again.

If activation has not started and you no longer want to continue, select **Abandon migration**. Volum first verifies the original NAS identity and requires that NAS to confirm cancellation before it restores local writes and indexing. If the NAS cannot be verified or reached, leave the migration paused and restore connectivity rather than pairing another server.

If the NAS says it already contains projects, stop. Volum deliberately refuses to merge it with the Desktop database. Use a fresh empty server or preserve and reset the NAS through its documented recovery process.

If activation started, `volumd` exits so Docker can restart it with the verified database. Check `docker compose ps` and logs. Do not switch the Desktop back to local while activation state is uncertain.

## Docker readiness fails

```bash
docker compose ps
docker compose logs --tail=200 volumd
```

Check:

- ownership and write access for `/config` and `/library`;
- free space, including temporary parsing space;
- a valid `VOLUM_PUBLIC_URL` origin;
- whether port `43821` is already allocated;
- whether a migration failed against an unsupported or damaged database.

Do not manually repair SQLite before preserving the failed data and a coordinated backup.

## Get help safely

Include the Volum version, client platform, server mode, and exact non-sensitive error. Do not publish pairing payloads, bearer tokens, private filenames, notes, or library paths.

On a local Desktop, **Settings → Advanced → Troubleshooting → Export diagnostics** creates a redacted report suitable for inspection before sharing. Review it and remove anything you do not want to disclose.

For exploitable security issues, follow [Report a security issue](./security).
