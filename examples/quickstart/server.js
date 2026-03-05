import express from "express";
import { readFileSync } from "fs";
import { createHmac, randomUUID } from "crypto";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const app = express();
const PORT = 3000;

// Load the shared secret (same one browsertapd uses)
function loadSecret() {
  if (process.env.BROWSERTAP_SECRET) {
    return Buffer.from(process.env.BROWSERTAP_SECRET, "hex");
  }
  const home = process.env.HOME || "/tmp";
  const hex = readFileSync(join(home, ".browsertap", "secret.key"), "utf8").trim();
  return Buffer.from(hex, "hex");
}

const secret = loadSecret();

// Serve static HTML
app.get("/", (_req, res) => {
  res.sendFile(join(__dirname, "index.html"));
});

// Serve the browsertap runtime SDK from the workspace
app.use(
  "/@browsertap/runtime",
  express.static(join(__dirname, "..", "..", "runtime", "browser", "dist"))
);

// Handshake endpoint: mints HMAC-SHA256 session tokens
app.post("/api/browsertap/handshake", (_req, res) => {
  const sessionId = randomUUID();
  const payload = {
    token_id: randomUUID(),
    scope: "session",
    subject: "quickstart",
    session_id: sessionId,
    issued_at: new Date().toISOString(),
    expires_at: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  };

  const encoded = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const sig = createHmac("sha256", secret).update(encoded).digest("base64url");

  res.json({
    sessionId,
    sessionToken: `${encoded}.${sig}`,
    socketUrl: "wss://127.0.0.1:4455/bridge",
    expiresAt: Math.floor(Date.now() / 1000) + 300,
  });
});

app.listen(PORT, () => {
  console.log(`quickstart app listening on http://localhost:${PORT}`);
  console.log("");
  console.log("Next steps:");
  console.log("  1. Make sure browsertapd is running:  browsertapd");
  console.log("  2. Open http://localhost:3000 in your browser");
  console.log("  3. Click 'Connect to browsertap'");
  console.log("  4. From another terminal:");
  console.log("     browsertap sessions");
  console.log('     browsertap run-js <codename> "document.title"');
  console.log("     browsertap screenshot <codename> -o page.jpg");
});
