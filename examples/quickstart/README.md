# browsertap quickstart

A minimal example showing the full browsertap workflow: daemon + web app + CLI.

## Prerequisites

- Rust 1.75+ (to build browsertap)
- Node.js 18+

## Steps

### 1. Build browsertap (from repo root)

```bash
cd ../..
cargo build --release
```

### 2. Start the daemon

```bash
./target/release/browsertapd
# => browsertapd listening on https://127.0.0.1:4455
```

### 3. Build the browser runtime SDK

```bash
cd runtime/browser
npm install --include=dev
npm run build
cd ../..
```

### 4. Start the quickstart app

```bash
cd examples/quickstart
npm install
npm start
# => quickstart app listening on http://localhost:3000
```

### 5. Connect your browser

Open http://localhost:3000 and click **Connect to browsertap**.

### 6. Control from CLI

```bash
# List sessions
./target/release/browsertap sessions

# Run JavaScript
./target/release/browsertap run-js <codename> "document.title"

# Take a screenshot
./target/release/browsertap screenshot <codename> -o page.jpg

# View console logs
./target/release/browsertap console <codename>

# Discover selectors
./target/release/browsertap selectors <codename>
```

Replace `<codename>` with the codename shown in the browser (e.g., `iron-falcon`).
