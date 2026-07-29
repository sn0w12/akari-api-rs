use std::net::SocketAddr;

use axum::Router;
use axum::routing::{delete, get, post, put};
use serde_json::Value;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use akari_api_rs::auth::AppState;
use akari_api_rs::config::Config;
use akari_api_rs::handlers::{
    anilist, author, bookmarks, comments, genre, lists, mal, manga, notifications, user,
};
use akari_api_rs::middleware::mal_token_refresh::MalTokenRefreshLayer;
use akari_api_rs::middleware::rate_limit::RateLimitLayer;

fn build_app(state: AppState, pool: sqlx::PgPool, config: Config) -> Router {
    Router::new()
        .route("/v2/manga/list", get(manga::list_manga))
        .route("/v2/manga/popular", get(manga::popular_manga))
        .route("/v2/manga/search", get(manga::search_manga))
        .route("/v2/manga/ids", get(manga::manga_ids))
        .route("/v2/manga/batch", get(manga::batch_manga))
        .route("/v2/manga/mal-id/{mal_id}", get(manga::by_mal_id))
        .route("/v2/manga/ani-id/{ani_id}", get(manga::by_ani_id))
        .route("/v2/manga/{id}/details", get(manga::manga_details))
        .route("/v2/manga/{id}/chapters", get(manga::manga_chapters))
        .route(
            "/v2/manga/{id}/recommendations",
            get(manga::manga_recommendations),
        )
        .route("/v2/manga/{id}/chapter-ids", get(manga::chapter_ids))
        .route("/v2/manga/{id}/{sub_id}", get(manga::chapter_detail))
        .route("/v2/manga/{id}/view", post(manga::record_view))
        .route("/v2/manga/viewed", get(manga::recently_viewed))
        .route("/v2/manga/{id}/rate", post(manga::rate_manga))
        .route("/v2/manga/{id}/rating", get(manga::get_rating))
        .route("/v2/manga/{id}/rate", delete(manga::delete_rating))
        .route("/v2/manga/rate/batch", post(manga::batch_rate))
        .route("/v2/genre/list", get(genre::list_genres))
        .route("/v2/author/list", get(author::list_authors))
        .route("/v2/user/list", get(user::list_users))
        .route("/v2/user/{id}/profile", get(user::user_profile))
        .route("/v2/user/me", get(user::me))
        .route("/v2/user/profile", put(user::update_profile))
        .route(
            "/v2/comment/list/{target_type}/{target_id}",
            get(comments::list_comments),
        )
        .route(
            "/v2/comments/{id}",
            post(comments::create_comment)
                .put(comments::update_comment)
                .delete(comments::delete_comment),
        )
        .route("/v2/comments/{id}/vote", post(comments::vote_comment))
        .route("/v2/comments/{target_id}/votes", get(comments::get_votes))
        .route("/v2/comments/{id}/report", post(comments::report_comment))
        .route("/v2/bookmarks", get(bookmarks::list_bookmarks))
        .route("/v2/bookmarks/search", get(bookmarks::search_bookmarks))
        .route("/v2/bookmarks/unread", get(bookmarks::unread_count))
        .route("/v2/bookmarks/batch", post(bookmarks::batch_upsert))
        .route("/v2/bookmarks/history", get(bookmarks::reading_history))
        .route("/v2/bookmarks/history/stats", get(bookmarks::reading_stats))
        .route(
            "/v2/bookmarks/{manga_id}",
            get(bookmarks::get_bookmark)
                .put(bookmarks::upsert_bookmark)
                .delete(bookmarks::delete_bookmark),
        )
        .route("/v2/lists/user/{user_id}", get(lists::list_user_lists))
        .route("/v2/lists/user/me", get(lists::list_my_lists))
        .route(
            "/v2/lists/user/me/manga/{manga_id}",
            get(lists::list_ids_containing_manga),
        )
        .route("/v2/lists/{id}", get(lists::get_list))
        .route("/v2/lists", post(lists::create_list))
        .route("/v2/lists/{id}", delete(lists::delete_list))
        .route("/v2/lists/{id}", post(lists::add_entry))
        .route("/v2/lists/{id}/{entry_id}", delete(lists::remove_entry))
        .route("/v2/lists/{id}/{entry_id}", put(lists::update_entry))
        .route("/v2/mal/token", post(mal::token_exchange))
        .route("/v2/mal/mangalist", get(mal::get_manga_list))
        .route("/v2/mal/mangalist", post(mal::update_manga_list))
        .route("/v2/mal/me", get(mal::me))
        .route("/v2/mal/logout", post(mal::logout))
        .route("/v2/ani/me", get(anilist::me))
        .route("/v2/ani/logout", post(anilist::logout))
        .route("/v2/ani/mangalist", get(anilist::get_manga_list))
        .route("/v2/ani/mangalist", post(anilist::update_manga_list))
        .route(
            "/v2/notifications/subscribe",
            post(notifications::subscribe),
        )
        .route(
            "/v2/notifications/website",
            get(notifications::website_notifications),
        )
        .route(
            "/v2/notifications/send",
            post(notifications::send_notification),
        )
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(MalTokenRefreshLayer {
            pool,
            config: config.clone(),
        })
        .layer(RateLimitLayer { config })
        .with_state(state)
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
    let app = build_app(state, pool, config);

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
    let items = body["data"]["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(
        items[0]["title"].as_str().unwrap().to_lowercase(),
        "hisureba"
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
