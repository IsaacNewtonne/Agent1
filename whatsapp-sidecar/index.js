import whatsappWeb from 'whatsapp-web.js';
import QRCode from 'qrcode';
import express from 'express';
import { createServer } from 'http';
import { rm } from 'fs/promises';

const { Client, LocalAuth, Events } = whatsappWeb;

const app = express();
const server = createServer(app);
const PORT = process.env.PORT || 17372;

app.use(express.json());

let client = null;
let qrCodeData = null;
let connectionState = 'disconnected';
let phoneNumber = null;
let lastError = null;

const sessions = new Map();

function createClient(sessionId) {
    const newClient = new Client({
        authStrategy: new LocalAuth({ clientId: sessionId, dataPath: './.wa-sessions' }),
        puppeteer: {
            args: [
                '--no-sandbox',
                '--disable-setuid-sandbox',
                '--disable-dev-shm-usage',
                '--disable-gpu',
                '--no-first-run',
                '--no-default-browser-check'
            ],
            headless: process.env.WHATSAPP_HEADLESS !== 'false',
        }
    });

    newClient.on(Events.LOADING_SCREEN, (percent, message) => {
        connectionState = 'connecting';
        console.log(`[${sessionId}] Loading ${percent}%: ${message}`);
    });

    newClient.on(Events.QR, async (qr) => {
        qrCodeData = qr;
        connectionState = 'qr_ready';
        lastError = null;
        console.log(`[${sessionId}] QR code received`);
    });

    newClient.on(Events.READY, () => {
        connectionState = 'connected';
        phoneNumber = newClient.info?.wid?.user || null;
        lastError = null;
        console.log(`[${sessionId}] WhatsApp connected: ${phoneNumber}`);
    });

    newClient.on(Events.AUTHENTICATED, () => {
        connectionState = 'connecting';
        lastError = null;
        console.log(`[${sessionId}] WhatsApp authenticated`);
    });

    newClient.on(Events.AUTHENTICATION_FAILURE, (message) => {
        connectionState = 'disconnected';
        lastError = message || 'WhatsApp authentication failed';
        console.error(`[${sessionId}] WhatsApp auth failure:`, lastError);
    });

    newClient.on(Events.DISCONNECTED, () => {
        connectionState = 'disconnected';
        phoneNumber = null;
        qrCodeData = null;
        console.log(`[${sessionId}] WhatsApp disconnected`);
    });

    newClient.on(Events.MESSAGE_CREATE, (message) => {
        if (!message.fromMe) {
            const incomingMsg = {
                id: message.id._serialized,
                from: message.from,
                fromName: message.notifyName,
                body: message.body,
                timestamp: message.timestamp,
                type: message.type,
            };
            console.log(`[${sessionId}] Incoming message:`, JSON.stringify(incomingMsg));
        }
    });

    return newClient;
}

function getMainSession() {
    if (!client) {
        client = createClient('main');
        sessions.set('main', client);
    }
    return client;
}

async function generateQrDataUrl(qr) {
    try {
        return await QRCode.toDataURL(qr, {
            width: 300,
            margin: 2,
            color: { dark: '#000000', light: '#FFFFFF' }
        });
    } catch (err) {
        console.error('QR code generation failed:', err);
        return null;
    }
}

app.get('/status', (req, res) => {
    res.json({
        state: connectionState,
        phone: phoneNumber,
        qr: qrCodeData,
        error: lastError,
    });
});

app.get('/qr', async (req, res) => {
    if (qrCodeData) {
        const dataUrl = await generateQrDataUrl(qrCodeData);
        res.json({ qr: dataUrl });
    } else {
        res.json({ qr: null });
    }
});

app.get('/qrsvg', async (req, res) => {
    if (qrCodeData) {
        try {
            const svg = await QRCode.toString(qrCodeData, { type: 'svg' });
            res.type('image/svg+xml').send(svg);
        } catch {
            res.status(500).json({ error: 'Failed to generate SVG' });
        }
    } else {
        res.status(404).json({ error: 'No QR code available' });
    }
});

app.post('/connect', async (req, res) => {
    try {
        const session = getMainSession();
        if (connectionState === 'connected') {
            return res.json({ status: 'already_connected', phone: phoneNumber });
        }
        if (connectionState === 'connecting') {
            return res.json({ status: 'connecting' });
        }
        connectionState = 'connecting';
        setTimeout(() => {
            if (connectionState === 'connecting' && !qrCodeData) {
                lastError = 'WhatsApp Web did not produce a QR code within 60 seconds. Try Reset, or set WHATSAPP_HEADLESS=false to inspect the browser window.';
                console.error(`[main] ${lastError}`);
            }
        }, 60000);
        session.initialize().catch(err => {
            console.error('Failed to initialize client:', err);
            lastError = err?.message || String(err);
            connectionState = 'disconnected';
        });
        res.json({ status: 'connecting' });
    } catch (err) {
        connectionState = 'disconnected';
        lastError = err?.message || String(err);
        res.status(500).json({ error: err.message });
    }
});

app.post('/disconnect', async (req, res) => {
    try {
        if (client) {
            await client.destroy();
            client = null;
            sessions.delete('main');
        }
        connectionState = 'disconnected';
        phoneNumber = null;
        qrCodeData = null;
        lastError = null;
        res.json({ status: 'disconnected' });
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/reset', async (req, res) => {
    try {
        if (client) {
            await client.destroy().catch(() => {});
            client = null;
            sessions.delete('main');
        }
        await rm('./.wa-sessions', { recursive: true, force: true });
        connectionState = 'disconnected';
        phoneNumber = null;
        qrCodeData = null;
        lastError = null;
        res.json({ status: 'reset' });
    } catch (err) {
        lastError = err?.message || String(err);
        res.status(500).json({ error: lastError });
    }
});

app.post('/send', async (req, res) => {
    const { to, text } = req.body;

    if (!to || !text) {
        return res.status(400).json({ error: 'Missing "to" or "text" field' });
    }

    if (connectionState !== 'connected') {
        return res.status(503).json({ error: 'WhatsApp not connected' });
    }

    try {
        const session = getMainSession();
        const chatId = to.includes('@c.us') ? to : `${to}@c.us`;
        const message = await session.sendMessage(chatId, text);
        res.json({
            success: true,
            messageId: message.id._serialized,
        });
    } catch (err) {
        console.error('Send failed:', err);
        res.status(500).json({ error: err.message });
    }
});

app.get('/messages/:chatId', async (req, res) => {
    const { chatId } = req.params;
    const limit = parseInt(req.query.limit) || 50;

    if (connectionState !== 'connected') {
        return res.status(503).json({ error: 'WhatsApp not connected' });
    }

    try {
        const session = getMainSession();
        const chat = await session.getChatById(chatId);
        const messages = await chat.fetchMessages({ limit });
        res.json({
            messages: messages.map(m => ({
                id: m.id._serialized,
                from: m.from,
                fromMe: m.fromMe,
                body: m.body,
                timestamp: m.timestamp,
                type: m.type,
            }))
        });
    } catch (err) {
        console.error('Failed to fetch messages:', err);
        res.status(500).json({ error: err.message });
    }
});

app.get('/chats', async (req, res) => {
    if (connectionState !== 'connected') {
        return res.status(503).json({ error: 'WhatsApp not connected' });
    }

    try {
        const session = getMainSession();
        const chats = await session.getChats();
        res.json({
            chats: chats.slice(0, 50).map(c => ({
                id: c.id._serialized,
                name: c.name,
                isGroup: c.isGroup,
                unreadCount: c.unreadCount,
            }))
        });
    } catch (err) {
        console.error('Failed to fetch chats:', err);
        res.status(500).json({ error: err.message });
    }
});

app.post('/approve', async (req, res) => {
    const { message } = req.body;
    if (!message) {
        return res.status(400).json({ error: 'Missing message' });
    }
    const session = getMainSession();
    const chatId = process.env.AGENT1_PHONE || null;
    if (chatId) {
        try {
            await session.sendMessage(`${chatId}@c.us`, `✅ Approved: ${message}`);
            res.json({ success: true });
        } catch (err) {
            res.status(500).json({ error: err.message });
        }
    } else {
        res.json({ success: true, note: 'No AGENT1_PHONE configured' });
    }
});

app.post('/notify', async (req, res) => {
    const { title, body } = req.body;
    const fullText = `*${title}*\n${body}`;
    const targetPhone = process.env.AGENT1_PHONE;

    if (!targetPhone) {
        return res.status(400).json({ error: 'AGENT1_PHONE not configured' });
    }

    if (connectionState !== 'connected') {
        return res.status(503).json({ error: 'WhatsApp not connected' });
    }

    try {
        const session = getMainSession();
        await session.sendMessage(`${targetPhone}@c.us`, fullText);
        res.json({ success: true });
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/command', async (req, res) => {
    const { text } = req.body;
    if (!text) {
        return res.status(400).json({ error: 'Missing text' });
    }
    console.log(`[COMMAND] ${text}`);
    res.json({ received: true, command: text });
});

server.listen(PORT, () => {
    console.log(`Agent1 WhatsApp sidecar running on port ${PORT}`);
});

process.on('SIGTERM', async () => {
    console.log('Shutting down...');
    if (client) {
        await client.destroy();
    }
    server.close();
    process.exit(0);
});
