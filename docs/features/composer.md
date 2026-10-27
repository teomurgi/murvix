# Composer

The composer is a text box for writing long prompts and commands, with files,
images, and dictation. It sends to the active terminal, or to every visible
terminal in the project. Handy for AI agents.

## Use it

| Action | macOS | Linux |
| --- | --- | --- |
| Open or close | `Cmd+I` | `Ctrl+Shift+I` |
| Send and press Return | `Cmd+Return` | `Ctrl+Return` |
| Send without Return | `Cmd+Shift+Return` | `Ctrl+Shift+Return` |
| Close | `Esc` | `Esc` |

- Sending first clears the terminal's prompt with `Ctrl+U`, in programs that
  support it.
- With text selected, only the selection is sent.
- **Send to All Split Panes** in the **…** menu sends to every pane you can see
  in the project. If one of them isn't a live terminal, such as an extension
  tab, nothing is sent.
- A failed send keeps the draft. Some terminals may already have received it,
  so check before retrying.
- Each project keeps its own draft, saved across restarts.

## Files and images

Attach with **Attach File**, drag and drop, or paste.

- Files are inserted as quoted paths.
- Images appear as `[Image N]`. On macOS, by default they are put on the
  clipboard and sent with `Ctrl+V` for compatible AI tools to read. Set
  **Settings → Composer → Image submission** to paste file paths instead.
  Linux uses saved image paths; clipboard-image submission is unavailable.
- For [remote](remote-servers.md) terminals, attachments are uploaded and the
  remote path is sent.

## Layout

The composer opens as a panel on the right or bottom, or as a floating box
that closes after sending. Switch from its **…** menu, which also has
**Clear After Sending**, **Clear on Close**, and **Reset Composer Size**.
`Cmd+=` and `Cmd+-` on macOS, or `Ctrl+=` and `Ctrl+-` on Linux, change its
font size.

## Dictation

`Cmd+Shift+I` on macOS or `Ctrl+Alt+I` on Linux starts dictation. Inside the
composer, it types into the draft. Elsewhere, the recording overlay sends the
transcript to the active terminal when you press `Return`; `Space` pauses and
`Esc` cancels.

On macOS, dictation needs an on-device speech language, plus microphone and
speech recognition access. Linux uses local `parec` audio capture and
`whisper-cli` 1.9.5 or newer. It requires a readable multilingual GGML voice
model and the Silero VAD model at
`$XDG_DATA_HOME/murvix/voice/ggml-silero-v6.2.0.bin` (by default,
`~/.local/share/murvix/voice/ggml-silero-v6.2.0.bin`). The default voice model is
`ggml-base.bin` in the same directory. Pick the language, Linux **Offline voice
model**, and **Send after recording** in **Settings → Composer**.
