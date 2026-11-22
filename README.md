# omamessages

omamessages is Omarchy Mobile's **Messages** app. It is laid out for a phone held
upright (480 logical pixels wide) and follows your Omarchy theme. It
behaves like AOSP's Messaging app; none of AOSP's code is used.

Messages talks to the modem through **omatel-agent**, the background service
it shares with Phone, from the
[omatel](https://github.com/derekross/omatel) repository. The agent keeps
running when the app is closed: it stores every incoming SMS and MMS, notifies you with an inline reply, and sends messages.
MMS (pictures, other attachments and group conversations) go through
mmsd-tng, which the agent talks to.
Messages reads the same stores and address book as the agent, and uses
omatel's shared crates, pinned to an omatel release tag.

## What you get

- The conversation list, with pinned conversations first, archived and
  muted conversations, unread counts, drafts, and full-text search.
- Threads with bubbles, day separators and the state of each message
  (Sending, Sent, Delivered, Not sent with a tap to retry). Older messages
  load as you scroll up. The composer counts SMS parts.
- MMS: pictures shown in their bubbles, other attachments as a file row
  that opens in the app your desktop picks; the attach button picks
  pictures (or any file) for the next message, shown above the composer
  until it goes. Group conversations show who wrote each message.
- Writing to a contact, to any number, or to several people at once (the
  + on a suggestion adds a person to a group message), blocking, and
  settings (notifications, what the lock screen shows, delivery reports,
  and mmsd-tng's MMSC, MMS proxy and MMS APN).
- Export and import of all messages as JSON (`omamessages --export FILE`,
  `--import FILE`; the format is in omatel's `docs/export-format.md`).
- `sms:`, `smsto:`, `mms:` and `mmsto:` links, with `?body=`.

Not yet: RCS (not planned). Pictures go as they are: Messages doesn't
shrink them to the carrier's size limit yet (the agent says when one is
too big).

**MMS settings.** mmsd-tng keeps the carrier's MMSC, MMS proxy and MMS APN
in `~/.mms/modemmanager/mms`. Messages' settings change them through
mmsd-tng's own D-Bus interface while it runs (they apply at once), and in
that file while it doesn't (mmsd-tng reads it when it starts). An empty
MMSC lets mmsd-tng look the carrier up; an empty proxy is "none".

## Install

On Omarchy Mobile, install the `omamessages` package; it brings in
`omatel-agent`. Start the agent once for your user:

```
systemctl --user enable --now omatel-agent
```

To build the package from a checkout, run `bin/install` on the phone, or
`bin/cross package` on an x86_64 laptop (see
[docs/development.md](docs/development.md)).

## Use

Open **Messages** from the app launcher, or:

```
omamessages                          # the conversation list
omamessages sms:+12025550100?body=hi # a new message, filled in (never sent by itself)
omamessages --compose
omamessages --export FILE            # all messages as JSON; --import FILE
```

Without the agent the app still shows its data, with a banner saying the
service isn't running.

## Where things are kept

| What | Where |
| --- | --- |
| Messages, block list | `~/.local/share/omatel/messages.db, blocked.db` |
| Contacts | `~/.local/share/contacts/<book>/*.vcf` |
| Settings | `~/.config/omatel/settings.toml` |
| Logs | `journalctl -t omamessages` (states only, never numbers or text) |

omamessages is released under the GNU General Public License, version 3 or
later. Its theme support comes from omatel's `omatel-theme`, adapted from
[gliff](https://github.com/omacom/gliff) (MIT); see NOTICE.
