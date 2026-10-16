# Relay notification system

A small Axum demo for subscribing a browser to Web Push notifications and sending a test notification.

## Run

Create a `.env` file with a Canvas access token for an account that can view the course's announcements:

```text
ACCESS_TOKEN=your_canvas_access_token
CANVAS_DOMAIN=cuhsd.instructure.com
COURSE_ID=30095
```

`ACCESS_TOKEN` is required. `CANVAS_DOMAIN` and `COURSE_ID` default to the values shown above. Start the server with `cargo run`, then open `http://localhost:5000` (or the HTTPS forwarded URL provided by your development environment). Set `PORT` to use a different local port. Select **Enable Push Notifications** and allow notifications in the browser. Once enabled, use **Send Notification** to send a test push.

The Canvas API request includes the application name and version as its `User-Agent`. If the API rejects the request, the server prints Canvas' response message; a `403 Forbidden` may indicate that the token's account cannot access the course or that the token is not valid for the configured Canvas domain.

The server generates a VAPID key pair at startup and serves only its public key to the page. This keeps the demo self-contained, but the key pair is ephemeral: after restarting the server, select **Enable Push Notifications** again. For production, persist a server-generated VAPID key pair securely and keep it stable across restarts.
