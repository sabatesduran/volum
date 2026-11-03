---
title: Server networking
description: Restrict volumd to Tailscale, another private network, or trusted HTTPS.
---

# Server networking

`volumd` authenticates every library request, but authentication does not replace transport security or firewalling.

## Recommended: Tailscale

Use the NAS host's Tailscale IP or MagicDNS origin:

```dotenv
VOLUM_PUBLIC_URL=http://volum-nas.example.ts.net:43821
```

Tailscale encrypts traffic between Tailnet devices even though the application URL uses `http://`. Volum separately requires a paired device token.

Configure the host firewall and Tailscale ACLs so only intended Tailnet devices can reach TCP port `43821`.

## Other private networks and VPNs

LAN, WireGuard, Twingate, ZeroTier, and similar private routes can use HTTP when the advertised IP is private or the hostname resolves entirely to private addresses:

```dotenv
VOLUM_PUBLIC_URL=http://10.8.0.12:43821
```

The route must be trusted to protect traffic from interception. Restrict port `43821` to the intended private interface and clients. Do not treat a bearer token as a substitute for network isolation or encryption.

## Understand the Compose port

The included Compose file maps host port `43821` to the container. Docker may publish that port on more host interfaces than your Tailnet.

Do not rely on the pairing token as the only boundary. Restrict the port with the NAS firewall, host firewall, or an explicitly scoped Docker publishing configuration.

Never forward plain HTTP port `43821` from an internet router.

## HTTPS reverse proxy

Use trusted HTTPS for any public or otherwise non-private origin. A reverse proxy must:

- terminate TLS with a certificate trusted by the client operating system;
- preserve request methods, authorization headers, range requests, and ETags;
- allow large streaming uploads within your chosen policy;
- forward to `volumd` on the private network;
- avoid logging bearer tokens, pairing payloads, filenames, searches, or notes.

Set `VOLUM_PUBLIC_URL` to the public-facing HTTPS origin, without a path:

```dotenv
VOLUM_PUBLIC_URL=https://volum.example.net
```

Desktop and mobile accept trusted HTTPS origins. Both block public HTTP. Clients verify `/identity` before sending a pairing secret and do not follow redirects.

## Unauthenticated endpoints

Only these endpoints work without a bearer token:

- `/healthz` for process health;
- `/readyz` for process and database readiness;
- `/identity` for minimal server and protocol identity;
- `/pair` for a valid, short-lived pairing session.

All `/api/v1` library endpoints require a non-revoked paired-device token.

## Test from a client

From a client on the selected route, first verify readiness:

```bash
curl --fail http://volum-nas.example.ts.net:43821/readyz
```

If this fails, check DNS, VPN or Tailscale status, ACLs, the NAS firewall, Docker port publishing, and the container log before retrying pairing.
