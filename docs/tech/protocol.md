# Protocol

How apps and the server talk. The exact messages live in `crates/murvix-protocol`.

## Connection

- The connection is a reliable byte stream: a Unix socket on the same computer,
  SSH to `murvix stdio` on another computer, or TLS for
  [paired devices](../features/mobile.md).
- Traffic is split into channels. Channel 0 carries control messages, and each
  attached session gets its own channel for screen updates and input.
- Every frame starts with a small header: length, protocol version, channel, and
  message kind. Messages are CBOR with numbered fields. Screen rows use postcard.

```mermaid
sequenceDiagram
    participant App
    participant Server
    App->>Server: Hello, with the versions I speak
    Server-->>App: Hello reply, with supported versions, build identity, and features
    App->>Server: Attach to a session
    Server-->>App: Screen and recent history, on a new channel
    loop While attached
        Server-->>App: Screen frame with changed rows
        App->>Server: Ack
    end
```

## Requests and replies

- Every request carries an ID and gets exactly one reply, in any order.
- A bad request, such as an unknown session or an invalid path, gets an error
  reply and the connection stays open.
- A request the server doesn't know gets an "unsupported" reply. That is how an
  older server answers a newer app.
- Malformed traffic closes the connection.
- For shared state, such as projects, sessions, and AI activity, the server
  sends a short "changed" notice and apps read the new state again.

## Screen updates

- A row is a list of style runs: text, its style, and its cell widths. Apps need
  no Unicode width tables.
- Frames carry only the visible rows that changed. A resize sends the whole
  screen.
- Attach replies include recent history. Further history is not streamed with
  screen updates; apps page through it and search it on request.

## Flow control

Each attached session has at most one frame in flight to each app. While the
app is busy, newer output merges into a single waiting frame.

```mermaid
sequenceDiagram
    participant Server
    participant App as Slow app
    Server->>App: Frame 1
    Note over Server: Frames 2, 3, and 4 merge into one
    App-->>Server: Ack frame 1
    Server->>App: Frames 2 to 4, merged
```

Pending screen data stays bounded, and a slow app jumps straight to the latest
screen. Control messages are selected before waiting screen frames; they cannot
preempt a write already in progress. Wire compression is planned but not built
yet.

## Versions

Two builds can talk when they share a protocol version. This build supports only
V2; V1 peers must update. The handshake exchanges supported-version lists rather
than selecting a different wire version. Only a breaking change bumps the
version. Fields and enum values have permanent numbers that are never reused.
Unknown optional fields are skipped; open server enums retain unrecognized
values. Unknown request variants get an "unsupported" reply, unreadable replies
fail their request, and unknown event variants can be skipped. Screen frames
remain strict because dropping one would break acknowledgements.

| Change | Bumps the version? |
| --- | --- |
| Add an optional field or one with a default | No |
| Add an enum value | No |
| Add a request | No. Older servers reply "unsupported" |
| Add something the server must act on | No. Add it as a new request or feature, since older servers ignore unknown fields |
| Remove something | No. Stop using it and keep its number reserved |
| Change a type or meaning, make a field required, or change screen rows or the header | Yes |

Encoded samples of each version are kept in
`crates/murvix-protocol/tests/fixtures/v<N>` and are only ever added to. CI
checks that the current build reads its version's samples. The cross-build
helper currently selects legacy beta tags with matching fixtures, not stable
release tags, and skips that check when none match. It therefore does not
establish compatibility with the latest stable release. Saved user data must
never be lost when a format changes.
