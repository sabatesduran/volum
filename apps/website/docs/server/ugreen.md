---
title: Install on a UGREEN NAS
description: Run volumd with UGREEN Container Manager and pair a phone from the browser.
---

# Install on a UGREEN NAS

UGREEN Container Manager can run the published `volumd` image on supported Intel and ARM NAS models. Interface labels vary slightly between UGOS releases, but the container settings are the same.

## Create persistent folders

Create two NAS folders:

```text
/docker/volum/config
/docker/volum/library
```

The container runs as UID and GID `1000:1000`. Grant that identity read/write access, or configure the container to run as the owner of both folders.

## Create the project

In **Container Manager**, create a Compose project with the following file. Replace the example NAS paths and IP address before starting it.

```yaml
services:
  volumd:
    image: ghcr.io/sabatesduran/volum-server:latest
    container_name: volumd
    restart: unless-stopped
    ports:
      - "43821:43821"
    environment:
      VOLUM_BIND: "0.0.0.0:43821"
      VOLUM_PUBLIC_URL: "http://192.168.1.50:43821"
      VOLUM_ADMIN_PASSWORD: ""
      VOLUM_ORGANIZATION_WRITES: "true"
      VOLUM_PHOTO_UPLOADS: "true"
      VOLUM_PRINT_PAIRING: "false"
      VOLUM_WEB_ENABLED: "true"
    volumes:
      - /docker/volum/config:/config
      - /docker/volum/library:/library
    read_only: true
    tmpfs:
      - /tmp:size=256m,mode=1777
    security_opt:
      - no-new-privileges:true
    cap_drop:
      - ALL
```

Replace `192.168.1.50` with the NAS address your phone can reach and fill the blank administrator password with a private value of at least 12 characters before deploying. A Tailscale MagicDNS name or trusted HTTPS reverse-proxy origin may be used instead.

## Pair the phone

1. Start the container and wait for its health state to become healthy.
2. Open `http://192.168.1.50:43821` in a browser.
3. Sign in with `VOLUM_ADMIN_PASSWORD`.
4. Select **Pair device**.
5. In Volum on the phone, select **Scan desktop code** and scan the QR.
6. Review the address and select **Verify and connect**.

The new phone appears immediately under **Paired devices**. The QR expires after five minutes and works only once.

## After setup

Once `/config/web-admin.json` exists and is included in your persistent mount, you may remove `VOLUM_ADMIN_PASSWORD` from the container environment and redeploy. The saved Argon2id hash continues to work.

Changing the environment value later resets the administrator password and revokes browser administrator sessions. It does not revoke paired phones or Desktop clients.

Do not forward plain port `43821` from your internet router. Use Tailscale, another private VPN, or a trusted HTTPS reverse proxy for access away from home.
