use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::{Arc, Mutex}};
use web_push_native::{
    jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair},
    p256::PublicKey, Auth, WebPushBuilder,
};
use tower_http::services::{ServeDir, ServeFile};
use std::env;
use std::error::Error;

#[derive(Deserialize)]
struct Announcement {
    title: String,
    author: Author,
    posted_at: String,
    message: String,
}
#[derive(Deserialize)]
struct Author {
    display_name: String,
}

// 1. Structure matching the payload coming from navigator.pushManager.subscribe()
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SubscriptionPayload {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

struct AppState {
    subscription: Mutex<Option<SubscriptionPayload>>,
    vapid_key_pair: ES256KeyPair,
    vapid_public_key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().expect("Failed to read .env file");

    // 1. Configuration
    let canvas_domain = env::var("CANVAS_DOMAIN").unwrap_or_else(|_| "cuhsd.instructure.com".into());
    let access_token = env::var("ACCESS_TOKEN")
        .map_err(|_| "ACCESS_TOKEN must be set to a valid Canvas access token")?;
    let course_id = env::var("COURSE_ID").unwrap_or_else(|_| "30095".into());

    // 2. Setup the Request
    let url = format!("https://{}/api/v1/announcements", canvas_domain);

    // 3. Make the API Call
    let client = reqwest::Client::builder()
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client.get(&url)
        .bearer_auth(access_token)
        .query(&[("context_codes[]", format!("course_{}", course_id))])
        .send()
        .await?;

    // Include Canvas' error message to help diagnose authentication and permission failures.
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        let body = body.chars().take(1000).collect::<String>();
        return Err(format!("Canvas API returned {status}: {body}").into());
    }

    // Parse response
    let announcements: Vec<Announcement> = response.json().await?;

    // 4. Print the Results
    if announcements.is_empty() {
        println!("No announcements found for this course.");
    } else {
        for item in announcements {
            println!("Title: {}", item.title);
            println!("Posted by: {}", item.author.display_name);
            println!("Date: {}", item.posted_at);
            println!("Message: {}", item.message);
            println!("-----------------------------------");
        }
    }
    let vapid_key_pair = ES256KeyPair::generate();
    let vapid_public_key = Base64UrlUnpadded::encode_string(
        &vapid_key_pair
            .public_key()
            .public_key()
            .to_bytes_uncompressed(),
    );
    let shared_state = Arc::new(AppState {
        subscription: Mutex::new(None),
        vapid_key_pair,
        vapid_public_key,
    });

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static");
    let serve_dir = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(static_dir.join("index.html")));

    // Define API routes
    let app = Router::new()
        .route("/vapid-public-key", get(vapid_public_key_handler))
        .route("/subscribe", post(subscribe_handler))
        .route("/send-notification", post(send_notification_handler))
        .with_state(shared_state)
        .fallback_service(serve_dir);

    let port = std::env::var("PORT").unwrap_or_else(|_| "5000".to_string());
    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();
    println!("Axum Web Push server running on http://{address}");
    
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

async fn canvas_webhook_signup() {
    let ContextId = "30095";
    let ContextType = "course";
    let EventTypes = ["announcement_created"];
    let Format = "live-event";
    let TransportMetadata = {
        "url": "https://your-server.com/webhook-endpoint"
    };
    let TransportType = "https";
    let url = format!("https://{}/api/v1/webhooks/subscriptions", canvas_domain);
    let client = reqwest::Client::builder()
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client.get(&url)
        .bearer_auth(access_token)
        .query()
        .send()
        .await?;
}

async fn subscribe_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SubscriptionPayload>,
) -> StatusCode {
    let mut db = state.subscription.lock().unwrap();
    *db = Some(payload); // Store the subscription
    println!("Successfully captured user browser subscription profile.");
    StatusCode::CREATED
}

async fn vapid_public_key_handler(State(state): State<Arc<AppState>>) -> Json<String> {
    Json(state.vapid_public_key.clone())
}

// 3. Handler to broadcast push payloads out to the client browser via Web Push Protocol
async fn send_notification_handler(State(state): State<Arc<AppState>>) -> Result<StatusCode, (StatusCode, String)> {
    let subscription = {
        let db = state.subscription.lock().unwrap();
        match db.clone() {
            Some(sub) => sub,
            None => return Err((StatusCode::BAD_REQUEST, "No registered subscribers found".to_string())),
        }
    };

    // Unpack customer metadata strings to corresponding encryption public keys
    let client_public_key = PublicKey::from_sec1_bytes(
        &Base64UrlUnpadded::decode_vec(&subscription.keys.p256dh)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Malformed p256dh string format".to_string()))?
    ).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid SEC1 browser key format".to_string()))?;

    let client_auth = Auth::clone_from_slice(
        &Base64UrlUnpadded::decode_vec(&subscription.keys.auth)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Malformed client authorization token".to_string()))?
    );

    // Build the request infrastructure using web_push_native
    let builder = WebPushBuilder::new(
        subscription.endpoint.parse().map_err(|_| (StatusCode::BAD_REQUEST, "Invalid target gateway URL".to_string()))?,
        client_public_key,
        client_auth,
    );

    // Create JSON body data for the browser's Service Worker to parse
    let notification_json = serde_json::json!({
        "title": "Rust Notification!",
        "body": "Dispatched securely via the Axum backend framework."
    }).to_string();

    // Generate push request headers and body signature
    let push_request = builder
        .with_vapid(&state.vapid_key_pair, "mailto:admin@example.com")
        .build(notification_json.into_bytes())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Push compilation pipeline failure".to_string()))?;

    // Dispatch the payload over HTTP to the specific browser gateway (e.g., Mozilla, Google FCM)
    let client = reqwest::Client::new();
    let mut req = client.post(push_request.uri().to_string());
    
    for (name, value) in push_request.headers().iter() {
        req = req.header(name.as_str(), value.as_ref());
    }

    let response = req.body(push_request.body().to_vec())
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Network processing breakdown: {}", e)))?;

    if response.status().is_success() {
        Ok(StatusCode::OK)
    } else {
        Err((StatusCode::BAD_GATEWAY, format!("Gateway returned error code: {}", response.status())))
    }
}
