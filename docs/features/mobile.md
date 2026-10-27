# Mobile

The server implements network access, pairing, and device revocation, and the
Rust client library can pair and connect over TLS. The desktop settings and
`murvix mobile` commands described below are implemented.

**Phone apps are not shipped in this repository.** There is no iOS/Android app
or mobile SDK package here, so scanning a code into a Murvix phone UI and
connecting from a phone over SSH are planned client experiences, not currently
available installation steps.

A compatible paired client connects directly to the computer's server, without
a Murvix cloud service or account. The transport uses TLS 1.3 with the server
certificate fingerprint pinned by the pairing code. It can use a reachable
IPv4 address or DNS name on a local network, VPN such as Tailscale, or a public
network. SSH access is separately implemented for the desktop and CLI; see
[Remote servers](remote-servers.md).

## Create a pairing offer

From the desktop app:

1. Open **Settings → Mobile** and turn on **Allow mobile devices**.
2. Under **Pair a phone**, choose **Show Pairing Code**.
3. A compatible client can consume the displayed QR code or pairing link.

From a shell on a computer with `murvix`:

```bash
murvix mobile enable
murvix mobile pair
```

- A pairing offer works once and expires after five minutes. Closing desktop
  Settings or disconnecting the CLI that created it cancels it; in the
  CLI, press `Ctrl-C` to cancel. Creating another offer replaces the old one.
- The code lists addresses found by the server. If clients reach it by another
  name, put it first: `murvix mobile pair --address devbox.example.com`.
  Repeat `--address` for more; each must be a DNS name or IPv4 address without
  a port, with at most eight addresses in the resulting offer.
- Access is off by default. Enabling it listens on **all IPv4 interfaces**,
  on TCP port `7419` by default. Change it in **Settings → Mobile** or with
  `murvix mobile enable --port <port>` (1024–65535).
- `--address` only advertises an address; it does not configure DNS, routing,
  firewall rules, or port forwarding. The client must be able to reach that port.
- Desktop Mobile settings manage this computer's server. To manage another
  server through SSH, use `murvix --host devbox mobile enable` and
  `murvix --host devbox mobile pair`.

## Manage phones

| Task | Desktop | CLI |
| --- | --- | --- |
| See status and phones | **Settings → Mobile** | `murvix mobile` |
| Revoke a phone | **Revoke** next to it | `murvix mobile revoke <id>` |
| Turn access off | **Allow mobile devices** off | `murvix mobile disable` |

Revoking a phone, or turning access off, disconnects it at once.

## Access granted to paired clients

Pairing grants full terminal and project-data access, including project
mutations, session creation/input, Git, files, and uploads. The protocol blocks
paired devices from managing network access, changing server settings, stopping
the server, or using the dedicated `exec` request.

These restrictions are **not a sandbox**: a paired client can run arbitrary
commands in a terminal, with the server user's permissions. Pair only clients
you trust. Device credentials and the server TLS identity survive restarts in
the profile's private `remote.json`; the server stores device token hashes,
not the device tokens themselves. Disabling access preserves pairings for
when you enable it again.

## Troubleshooting

- On macOS, allow `murvix-server` if the firewall asks.
- Confirm that access reports **listening**, not just enabled:
  `murvix mobile` reports listener failures such as a port already in use.
- The client and server must reach each other over the configured TCP port.
  Adding `--address` does not make an otherwise unreachable server reachable.
- An invalid `remote.json` keeps access off and reports an error rather than
  silently trusting a new identity. Keep the file private: it contains the
  server's TLS private key.
