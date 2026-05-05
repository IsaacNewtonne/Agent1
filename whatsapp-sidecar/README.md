# Agent1 WhatsApp Sidecar

Node.js sidecar service that bridges Agent1 with WhatsApp using `whatsapp-web.js`.

## Setup

```bash
cd whatsapp-sidecar
npm install
```

## Running

```bash
npm start
```

The sidecar starts on port **17372**.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PORT` | Sidecar port (default: 17372) |
| `AGENT1_PHONE` | Phone number to send notifications (e.g., +1234567890) |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/status` | Connection status |
| `GET` | `/qr` | QR code as data URL |
| `GET` | `/qrsvg` | QR code as SVG |
| `POST` | `/connect` | Start connection |
| `POST` | `/disconnect` | Disconnect |
| `POST` | `/send` | Send message |
| `POST` | `/notify` | Send formatted notification |
| `POST` | `/approve` | Send approval confirmation |
| `POST` | `/command` | Send command to Agent1 |
| `GET` | `/chats` | List chats |

## How It Works

1. Sidecar uses `whatsapp-web.js` to connect to WhatsApp Web
2. QR code is generated for initial authentication
3. Session is persisted in `.wa-sessions/` directory
4. Agent1 server communicates with sidecar via HTTP

## Usage with Agent1

The Agent1 server automatically proxies WhatsApp requests through the sidecar.
Set `AGENT1_PHONE` environment variable to receive notifications on your phone.

```bash
AGENT1_PHONE=+1234567890 npm start
```