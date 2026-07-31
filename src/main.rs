use std::net::SocketAddr;

use akari_api_rs::analytics::AnalyticsLayer;
use akari_api_rs::auth::AppState;
use akari_api_rs::config::Config;
use akari_api_rs::db::init_pool;
use akari_api_rs::handlers::{
    anilist, analytics, author, bookmarks, comments, genre, lists, mal, manga, notifications, user,
};
use akari_api_rs::middleware::mal_token_refresh::MalTokenRefreshLayer;
use akari_api_rs::middleware::rate_limit::RateLimitLayer;
use akari_api_rs::openapi::ApiDoc;
use axum::Router;
use axum::routing::{delete, get, post, put};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("akari_api_rs=info,tower_http=info")),
        )
        .init();

    let config = Config::from_cli();
    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to connect to database");
    tracing::info!("Connected to database");

    let state = AppState {
        db: pool.clone(),
        config: config.clone(),
    };

    let app = Router::new()
        .route("/v2/manga/list", get(manga::list_manga))
        .route("/v2/manga/list/popular", get(manga::popular_manga))
        .route("/v2/manga/search", get(manga::search_manga))
        .route("/v2/manga/ids", get(manga::manga_ids))
        .route("/v2/manga/batch", get(manga::batch_manga))
        .route("/v2/manga/mal/{malId}", get(manga::by_mal_id))
        .route("/v2/manga/mal/batch", post(manga::batch_by_mal))
        .route("/v2/manga/ani/{aniId}", get(manga::by_ani_id))
        .route("/v2/manga/ani/batch", post(manga::batch_by_ani))
        .route("/v2/manga/{id}", get(manga::get_manga))
        .route("/v2/manga/{id}/details", get(manga::manga_details))
        .route("/v2/manga/chapter/ids", get(manga::global_chapter_ids))
        .route("/v2/manga/{id}/chapters", get(manga::manga_chapters))
        .route(
            "/v2/manga/{id}/recommendations",
            get(manga::manga_recommendations),
        )
        .route(
            "/v2/manga/{id}/relationships",
            get(manga::get_work_relationships),
        )
        .route("/v2/manga/{id}/chapter-ids", get(manga::chapter_ids))
        .route("/v2/manga/{id}/{subId}", get(manga::chapter_detail))
        .route("/v2/manga/{id}/view", post(manga::record_view))
        .route("/v2/manga/viewed", get(manga::recently_viewed))
        .route("/v2/manga/{id}/rate", post(manga::rate_manga))
        .route("/v2/manga/{id}/rating", get(manga::get_rating))
        .route("/v2/manga/{id}/rate", delete(manga::delete_rating))
        .route("/v2/manga/rate/batch", post(manga::batch_rate))
        .route("/v2/genre/list", get(genre::list_genres))
        .route("/v2/genre/{name}", get(genre::manga_by_genre))
        .route("/v2/author/list", get(author::list_authors))
        .route("/v2/author/{name}", get(author::manga_by_author))
        .route("/v2/user", get(user::list_users))
        .route("/v2/user/{userId}", get(user::user_profile))
        .route("/v2/user/me", get(user::me))
        .route("/v2/user/profile", put(user::update_profile))
        .route(
            "/v2/comments/{commentId}/replies",
            get(comments::get_comment_replies),
        )
        .route(
            "/v2/comments/{commentId}/vote",
            post(comments::vote_comment),
        )
        .route("/v2/comments/{id}/votes", get(comments::get_votes))
        .route(
            "/v2/comments/{commentId}/report",
            post(comments::report_comment),
        )
        .route(
            "/v2/comments/{id}",
            get(comments::list_comments_by_target)
                .post(comments::create_comment)
                .put(comments::update_comment)
                .delete(comments::delete_comment),
        )
        .route("/v2/bookmarks", get(bookmarks::list_bookmarks))
        .route("/v2/bookmarks/search", get(bookmarks::search_bookmarks))
        .route("/v2/bookmarks/unread", get(bookmarks::unread_count))
        .route("/v2/bookmarks/batch", post(bookmarks::batch_upsert))
        .route("/v2/bookmarks/history", get(bookmarks::reading_history))
        .route("/v2/bookmarks/history/stats", get(bookmarks::reading_stats))
        .route(
            "/v2/bookmarks/{mangaId}",
            get(bookmarks::get_bookmark)
                .put(bookmarks::upsert_bookmark)
                .delete(bookmarks::delete_bookmark),
        )
        .route("/v2/lists/user/{userId}", get(lists::list_user_lists))
        .route("/v2/lists/user/me", get(lists::list_my_lists))
        .route(
            "/v2/lists/user/me/manga/{mangaId}",
            get(lists::list_ids_containing_manga),
        )
        .route("/v2/lists/{id}", get(lists::get_list))
        .route("/v2/lists", post(lists::create_list))
        .route("/v2/lists/{id}", delete(lists::delete_list))
        .route("/v2/lists/{id}", post(lists::add_entry))
        .route("/v2/lists/{id}/{entryId}", delete(lists::remove_entry))
        .route("/v2/lists/{id}/{entryId}", put(lists::update_entry))
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
        .route("/v2/analytics/overview", get(analytics::overview))
        .route("/v2/analytics/timeseries", get(analytics::timeseries))
        .route("/v2/analytics/top", get(analytics::top))
        .route("/v2/analytics/slowest", get(analytics::slowest))
        .route("/v2/analytics/requests", get(analytics::requests))
        .route_layer(AnalyticsLayer::new(pool.clone()))
        .merge(SwaggerUi::new("/v2/openapi").url("/v2/openapi.json", ApiDoc::openapi()))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(MalTokenRefreshLayer {
            pool: pool.clone(),
            config: config.clone(),
        })
        .layer(RateLimitLayer {
            config: config.clone(),
        })
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::COOKIE,
                    axum::http::header::ACCEPT,
                    axum::http::header::HeaderName::from_static("x-api-key"),
                    axum::http::header::HeaderName::from_static("x-forwarded-for"),
                ])
                .allow_credentials(true),
        )
        .with_state(state);

    let addr = SocketAddr::new(config.host.parse().expect("Invalid HOST"), config.port);
    tracing::info!("Starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server failed");
}
