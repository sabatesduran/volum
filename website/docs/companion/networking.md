---
title: Choose a connection network
description: Connect Volum through Tailscale, a LAN or private VPN, or trusted HTTPS.
---

# Choose a connection network

Volum connects clients directly to the canonical Desktop or `volumd` server. Tailscale remains the easiest option, but it is not required.

## Supported routes

| Route | URL | Protection | Notes |
| --- | --- | --- | --- |
| Tailscale | `http://100.x.y.z:43821` or MagicDNS | Tailscale WireGuard tunnel | Detected automatically on Desktop |
| Private LAN | `http://192.168.x.y:43821` | Your trusted local network | Do not use on guest or untrusted Wi-Fi |
| WireGuard or ZeroTier | Private `http://` IP or DNS name | The VPN tunnel | Configure the private listener IP manually |
| Twingate or another private access network | Private `http://` DNS name, or HTTPS | The private access route | HTTP names must resolve only to private addresses |
| Reverse proxy | `https://volum.example.com` | TLS certificate trusted by the device | Required for public or otherwise non-private routes |

Any trusted HTTPS origin is accepted. Plain HTTP is accepted only for loopback, private IPv4/IPv6, link-local, Tailscale, or a hostname that resolves entirely to private addresses. Volum blocks public HTTP even when a pairing payload requests it.

## Automatic Tailscale mode

1. Install and connect Tailscale on the Desktop and client.
2. Open **Settings → Companion Access**.
3. Select **Automatic**, save, and enable access.
4. Volum discovers the Desktop's Tailscale address and listens only there.

Existing QR codes with the `tailscale-http` transport remain compatible.

## Custom private or HTTPS mode

Custom mode separates two values:

- **Listen on this private IP** is an IP assigned to the Desktop, such as a LAN, WireGuard, or ZeroTier address. Volum does not accept a wildcard or public listener IP.
- **Address clients will use** is the complete origin advertised in pairing codes. It can be the same private address, a private DNS name, or an HTTPS reverse-proxy origin.

This separation lets Volum listen on `192.168.1.20` while clients use `https://volum.example.com`.

## Before a secret or token is sent

After scanning, mobile shows an editable address review screen. The client calls the unauthenticated `/identity` endpoint and checks the expected server ID and protocol before it sends the one-time pairing secret. The same identity check protects address changes made later in Settings.

Redirects are not followed. This prevents a trusted private origin from silently forwarding a pairing secret or token to another host.

## Private HTTP warning

HTTP does not provide application-level encryption. Use it only when Tailscale, a private VPN, or a trusted isolated LAN supplies the transport boundary. Mobile asks for explicit confirmation before first connecting to a non-Tailscale private HTTP address.

Use HTTPS when:

- the hostname resolves to a public address;
- traffic crosses the public internet without a private tunnel;
- the LAN cannot be trusted against interception;
- a reverse proxy already provides a certificate trusted by the client operating system.

Never expose plain TCP port `43821` to the public internet.

## Firewall and DNS checklist

- Allow only intended clients to reach the listener or reverse proxy.
- Keep Tailscale ACL, WireGuard, ZeroTier, Twingate, VLAN, and host-firewall rules as narrow as practical.
- Ensure private HTTP DNS names resolve only to private addresses from the client.
- Preserve authorization headers, request methods, range requests, ETags, and streaming uploads in a reverse proxy.
- Do not log pairing payloads, bearer tokens, filenames, searches, or notes.

Continue with [Pair a mobile device](./pair) or [Server networking](../server/networking).
