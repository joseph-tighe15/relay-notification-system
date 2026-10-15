# Relay notification system

A small Axum demo for subscribing a browser to Web Push notifications and sending a test notification.

## Run

Start the server with `cargo run`, then open `http://localhost:5000` (or the HTTPS forwarded URL provided by your development environment). Set `PORT` to use a different local port. Select **Enable Push Notifications** and allow notifications in the browser. Once enabled, use **Send Notification** to send a test push.

The server generates a VAPID key pair at startup and serves only its public key to the page. This keeps the demo self-contained, but the key pair is ephemeral: after restarting the server, select **Enable Push Notifications** again. For production, persist a server-generated VAPID key pair securely and keep it stable across restarts.
