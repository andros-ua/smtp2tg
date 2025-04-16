# smtp2tg

A lightweight SMTP-to-Telegram forwarder.  
Receives emails over SMTP and pushes them to a Telegram chat using your bot.

---

## 🚀 Features

- Minimal SMTP server (port `2525`)
- Sends email subject & body to Telegram
- Supports `MarkdownV2` or `HTML` formatting
- Expandable block quotes for long messages
- Low memory footprint & single binary
- Optional `--verbose` logging

---

## 🔧 Usage

```bash
smtp2tg --token <BOT_TOKEN> --chatid <CHAT_ID> [options]
```

### Required flags:
| Flag            | Description                   |
|-----------------|-------------------------------|
| `--token`, `-t` | Your Telegram bot token       |
| `--chatid`, `-c`| Your Telegram chat ID         |

### Optional flags:
| Flag              | Description                     |
|-------------------|---------------------------------|
| `--parsemode`, `-p` | `MarkdownV2` (default) or `HTML` |
| `--verbose`, `-v`   | Enable verbose logging          |
| `--help`, `-h`      | Show this help message          |

### Example:
```bash
smtp2tg -t 123456:ABC-DEF -c 987654321 -p HTML -v
```

---

## 🤖 Telegram Bot Setup

1. Talk to [@BotFather](https://t.me/BotFather)
2. Create a bot → get the token
3. Add the bot to your group or chat
4. Send a message to the group
5. Use [@userinfobot](https://t.me/userinfobot) to get the chat ID

---

## ⚙️ systemd Service

To run `smtp2tg` as a service:

```ini
# /etc/systemd/system/smtp2tg.service
[Unit]
Description=SMTP to Telegram Bridge
After=network.target

[Service]
ExecStart=/usr/local/bin/smtp2tg --token YOUR_TOKEN --chatid YOUR_CHAT_ID --parsemode HTML
Restart=on-failure
User=youruser
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

Then:
```sh
sudo systemctl daemon-reexec
sudo systemctl enable smtp2tg
sudo systemctl start smtp2tg
```

---

## 🛠 Build

```bash
cargo build --release
```

Binary will be in `target/release/smtp2tg`

---

## 📝 License

MIT
