use std::net::SocketAddr;

use serde_json::{Value, json};

use akari_api_rs::app::build_app;
use akari_api_rs::auth::AppState;
use akari_api_rs::config::Config;

fn fixture_work_id() -> String {
    std::env::var("BENCH_WORK_ID")
        .unwrap_or_else(|_| "019f8fd0-37cd-7ce3-bc6e-077191378515".to_string())
}

fn test_config() -> Config {
    Config::from_env()
}

async fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/postgres".to_string());
    sqlx::PgPool::connect(&url).await.unwrap()
}

async fn spawn_app() -> (reqwest::Client, SocketAddr) {
    let config = test_config();
    let pool = test_pool().await;
    let state = AppState {
        db: pool.clone(),
        config: config.clone(),
    };
    let app = build_app(state, false);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    (reqwest::Client::new(), addr)
}

#[tokio::test]
async fn test_genre_list() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/genre/list", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    assert!(body["data"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_manga_list() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/manga/list?page=1&pageSize=3", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    assert!(body["data"]["totalItems"].as_i64().unwrap_or(0) > 0);
}

#[tokio::test]
async fn test_manga_search() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/manga/search?query=hisureba", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    let items = body["data"].as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(
        items[0]["title"].as_str().unwrap().to_lowercase(),
        "hisureba"
    );
}

#[tokio::test]
async fn test_manga_search_title_priority() {
    let (client, addr) = spawn_app().await;

    // Exact primary-title match must outrank a longer title containing the query.
    let resp = client
        .get(format!(
            "http://{}/v2/manga/search?query=chainsaw&limit=20",
            addr
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    let items = body["data"].as_array().unwrap();
    assert!(
        !items.is_empty(),
        "chainsaw search must return rows (fixture)"
    );
    assert_eq!(
        items[0]["title"].as_str().unwrap(),
        "Chainsaw Man",
        "exact primary-title match must rank first"
    );

    // The duplicate discovery surface must apply the same ordering.
    let resp = client
        .get(format!(
            "http://{}/v2/manga/list?sortBy=search&query=chainsaw&page=1&pageSize=20",
            addr
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    let items = body["data"]["items"].as_array().unwrap();
    assert!(
        !items.is_empty(),
        "chainsaw list search must return rows (fixture)"
    );
    assert_eq!(
        items[0]["title"].as_str().unwrap(),
        "Chainsaw Man",
        "list search must rank exact primary-title match first"
    );

    // An all-whitespace query is treated as no query: successful empty data.
    let resp = client
        .get(format!("http://{}/v2/manga/search?query=%20%20", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    assert!(
        body["data"].as_array().unwrap().is_empty(),
        "all-whitespace query must return empty data"
    );
}

#[tokio::test]
async fn test_manga_ids() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/manga/ids?page=1&pageSize=2", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"]["totalItems"].as_i64().unwrap_or(0) > 0);
}

#[tokio::test]
async fn test_author_list() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/author/list?page=1&pageSize=2", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"]["items"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_auth_required() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!("http://{}/v2/user/me", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let resp = client
        .get(format!("http://{}/v2/bookmarks/unread", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_manga_detail_fixture() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!(
            "http://{}/v2/manga/{}/details",
            addr,
            fixture_work_id()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    assert_eq!(body["data"]["id"], fixture_work_id());
    assert!(body["data"]["chapters"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_chapter_detail_fixture() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!(
            "http://{}/v2/manga/{}/1?scanlatorId=0",
            addr,
            fixture_work_id()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Success");
    assert_eq!(body["data"]["number"], 1.0);
}

#[tokio::test]
async fn test_invalid_manga_404() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!(
            "http://{}/v2/manga/00000000-0000-0000-0000-000000000000/details",
            addr
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Error");
}

#[tokio::test]
async fn test_invalid_chapter_404() {
    let (client, addr) = spawn_app().await;
    let resp = client
        .get(format!(
            "http://{}/v2/manga/{}/999999",
            addr,
            fixture_work_id()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Error");
}

#[tokio::test]
async fn test_batch_over_limit_400() {
    let (client, addr) = spawn_app().await;
    let over_limit: Vec<i32> = (1..=51).collect();

    let resp = client
        .post(format!("http://{}/v2/manga/mal/batch", addr))
        .json(&json!({ "malIds": over_limit }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    let resp = client
        .post(format!("http://{}/v2/manga/ani/batch", addr))
        .json(&json!({ "aniIds": over_limit }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_analytics_non_admin_403() {
    let (client, addr) = spawn_app().await;

    // Unauthenticated analytics request is rejected with 401.
    let resp = client
        .get(format!("http://{}/v2/analytics/overview", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // A valid non-admin session is rejected with 403.
    let resp = client
        .get(format!("http://{}/v2/analytics/overview", addr))
        .header(
            "Cookie",
            "better-auth.session_token=REGULAR_USER_SESSION_TOKEN",
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
