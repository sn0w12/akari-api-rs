use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{Request, StatusCode, header};
use axum::Router;
use criterion::{Criterion, criterion_group, criterion_main};
use tower::ServiceExt;
use uuid::Uuid;

use akari_api_rs::app::build_app;
use akari_api_rs::auth::{AppState, AuthUser};
use akari_api_rs::config::Config;
use akari_api_rs::db::init_pool;
use akari_api_rs::handlers::*;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn db_url() -> String {
    env_or(
        "DATABASE_URL",
        "postgresql://postgres:password@localhost:5432/postgres",
    )
}

fn pool_max_connections() -> u32 {
    std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
}

fn work_id() -> Uuid {
    env_or("BENCH_WORK_ID", "019f8fd0-37cd-7ce3-bc6e-077191378515")
        .parse()
        .expect("BENCH_WORK_ID must be a valid UUID")
}

fn mal_id() -> i32 {
    env_or("BENCH_MAL_ID", "92475")
        .parse()
        .expect("BENCH_MAL_ID must be an integer")
}

fn ani_id() -> i32 {
    env_or("BENCH_ANI_ID", "95863")
        .parse()
        .expect("BENCH_ANI_ID must be an integer")
}

fn user_id() -> String {
    env_or(
        "BENCH_USER_ID",
        "sENy8w7haiU1VN3oVHkxvfihAQDuDhq8",
    )
}

fn session_token() -> String {
    std::env::var("BENCH_SESSION_COOKIE").unwrap_or_else(|_| {
        panic!(
            "BENCH_SESSION_COOKIE must be set to a valid better-auth session token \
             for the authenticated bookmark benchmarks (see plan)"
        )
    })
}

fn api_key() -> String {
    env_or("API_KEY", "bench-api-key")
}

static RT: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().expect("rt"));

static POOL: LazyLock<sqlx::PgPool> = LazyLock::new(|| {
    let _ = dotenvy::dotenv();
    let url = db_url();
    // Attach the pool to the bench runtime so connections stay alive for the
    // whole process; a pool bound to a dropped runtime cannot serve acquires.
    RT.block_on(init_pool(&url, pool_max_connections()))
        .expect("connect DB")
});

fn db() -> sqlx::PgPool {
    POOL.clone()
}

fn bench_config() -> Config {
    let mut key = [0u8; 32];
    let enc = env_or("ENCRYPTION_KEY", "0123456789abcdef0123456789abcdef");
    let bytes = enc.as_bytes();
    if bytes.len() != 32 {
        panic!("ENCRYPTION_KEY must be exactly 32 bytes for the benchmark");
    }
    key.copy_from_slice(&bytes[..32]);
    Config {
        database_url: db_url(),
        host: "127.0.0.1".into(),
        port: 3001,
        api_key: api_key(),
        encryption_key: key,
        mal_client_id: env_or("MAL_CLIENT_ID", "bench-mal-client"),
        db_max_connections: pool_max_connections(),
        vapid_subject: env_or("WEBPUSH_SUBJECT", ""),
        vapid_public_key: env_or("VAPID_PUBLIC_KEY", ""),
        vapid_private_key: env_or("VAPID_PRIVATE_KEY", ""),
    }
}

fn app_state() -> AppState {
    AppState {
        db: db(),
        config: bench_config(),
    }
}

fn bench_one<F, Fut>(c: &mut Criterion, name: &str, f: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    c.bench_function(name, |b| {
        b.to_async(&*RT).iter(|| async { f().await });
    });
}

fn bench_handlers(c: &mut Criterion) {
    // Force runtime + pool initialization on the main thread before any
    // async bench task touches them (block_on inside an async task panics).
    let _ = &*RT;
    let _ = &*POOL;

    bench_one(c, "genre/list", || async {
        let r = genre::list_genres(State(db())).await;
        assert!(r.is_ok(), "genre/list must succeed");
        assert!(
            !r.unwrap().0.data.is_empty(),
            "genre/list must return genres (fixture)"
        );
    });
    bench_one(c, "author/list", || async {
        let r = author::list_authors(
            Query(author::AuthorListParams {
                page: Some(1),
                page_size: Some(20),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "author/list must succeed");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "author/list must return authors (fixture)"
        );
    });
    bench_one(c, "manga/list", || async {
        let r = manga::list_manga(
            Query(manga::MangaListParams {
                page: Some(1),
                page_size: Some(20),
                sort_by: Some("latest".into()),
                query: None,
                genres: None,
                excluded_genres: None,
                authors: None,
                types: None,
                excluded_types: None,
                status: None,
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/list must succeed");
        let data = r.unwrap().0.data;
        assert!(!data.items.is_empty(), "manga/list must return rows (fixture)");
        assert!(data.total_items > 0, "manga/list totalItems must be positive");
    });
    bench_one(c, "manga/list-genre", || async {
        let r = manga::list_manga(
            Query(manga::MangaListParams {
                page: Some(1),
                page_size: Some(20),
                sort_by: Some("latest".into()),
                query: None,
                genres: Some(vec!["Action".into()]),
                excluded_genres: None,
                authors: None,
                types: None,
                excluded_types: None,
                status: None,
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/list-genre must succeed");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "manga/list-genre must return rows (fixture)"
        );
    });
    bench_one(c, "manga/popular", || async {
        let r = manga::popular_manga(
            Query(manga::PopularParams {
                page: Some(1),
                limit: Some(10),
                excluded_genres: None,
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/popular must succeed");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "manga/popular must return rows (fixture)"
        );
    });
    bench_one(c, "manga/detail", || async {
        let r = manga::manga_details(Path(work_id()), State(db())).await;
        assert!(r.is_ok(), "manga/detail must succeed (fixture work)");
        assert!(
            !r.unwrap().0.data.chapters.is_empty(),
            "manga/detail must return chapters (fixture)"
        );
    });
    bench_one(c, "manga/chapters", || async {
        let r = manga::manga_chapters(Path(work_id()), State(db())).await;
        assert!(r.is_ok(), "manga/chapters must succeed");
        assert!(
            !r.unwrap().0.data.chapters.is_empty(),
            "manga/chapters must return chapters (fixture)"
        );
    });
    bench_one(c, "manga/recommendations", || async {
        let r = manga::manga_recommendations(
            Path(work_id()),
            Query(manga::PopularParams {
                page: Some(1),
                limit: Some(20),
                excluded_genres: None,
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/recommendations must succeed");
        assert!(
            !r.unwrap().0.data.is_empty(),
            "manga/recommendations must return rows (fixture)"
        );
    });
    bench_one(c, "manga/search", || async {
        let r = manga::search_manga(
            Query(manga::SearchParams {
                query: Some("hisureba".into()),
                limit: Some(20),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/search must succeed");
        assert_eq!(
            r.unwrap().0.data[0].title.to_lowercase(),
            "hisureba",
            "manga/search must rank hisureba first (fixture)"
        );
    });
    bench_one(c, "manga/ids", || async {
        let r = manga::manga_ids(
            Query(manga::MangaIdsParams {
                page: Some(1),
                page_size: Some(100),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/ids must succeed");
        assert!(
            r.unwrap().0.data.total_items > 0,
            "manga/ids totalItems must be positive"
        );
    });
    bench_one(c, "manga/chapter-ids", || async {
        let r = manga::chapter_ids(Path(work_id()), State(db())).await;
        assert!(r.is_ok(), "manga/chapter-ids must succeed");
        assert!(
            !r.unwrap().0.data.items[0].chapter_ids.is_empty(),
            "manga/chapter-ids must return chapter ids (fixture)"
        );
    });
    bench_one(c, "manga/by-mal-id", || async {
        let r = manga::by_mal_id(Path(mal_id()), State(db())).await;
        assert!(r.is_ok(), "manga/by-mal-id must succeed (fixture)");
        assert_eq!(
            r.unwrap().0.data.id,
            work_id(),
            "manga/by-mal-id must map to the fixture work"
        );
    });
    bench_one(c, "manga/by-ani-id", || async {
        let r = manga::by_ani_id(Path(ani_id()), State(db())).await;
        assert!(r.is_ok(), "manga/by-ani-id must succeed (fixture)");
        assert_eq!(
            r.unwrap().0.data.id,
            work_id(),
            "manga/by-ani-id must map to the fixture work"
        );
    });
    bench_one(c, "manga/chapter-detail", || async {
        let r = manga::chapter_detail(
            Path((work_id(), 1.0)),
            Query(manga::ChapterDetailParams {
                scanlation_group_id: Some(0),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/chapter-detail must succeed (fixture)");
        assert_eq!(
            r.unwrap().0.data.number,
            1.0,
            "manga/chapter-detail must return the fixture chapter"
        );
    });
    bench_one(c, "manga/batch", || async {
        let r = manga::batch_manga(
            Query(manga::BatchParams {
                ids: Some(work_id().to_string()),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "manga/batch must succeed");
        assert_eq!(
            r.unwrap().0.data.items.len(),
            1,
            "manga/batch must return the fixture work"
        );
    });
    bench_one(c, "user/profile", || async {
        let r = user::user_profile(Path(user_id()), State(db())).await;
        assert!(r.is_ok(), "user/profile must succeed (fixture user)");
    });
    bench_one(c, "user/list", || async {
        let r = user::list_users(
            Query(user::UserListParams {
                page: Some(1),
                page_size: Some(20),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "user/list must succeed");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "user/list must return users (fixture)"
        );
    });
    bench_one(c, "comments/list", || async {
        let r = comments::list_comments(
            Path(("work".to_string(), work_id())),
            Query(comments::CommentListParams {
                page: Some(1),
                page_size: Some(20),
                sort: None,
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "comments/list must succeed");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "comments/list must return comments (fixture)"
        );
    });
    bench_one(c, "notifications/website", || async {
        let r = notifications::website_notifications(State(db())).await;
        assert!(r.is_ok(), "notifications/website must succeed");
        assert!(
            !r.unwrap().0.data.is_empty(),
            "notifications/website must return seeded notifications (fixture)"
        );
    });
    bench_one(c, "bookmarks/list", || async {
        let user = AuthUser {
            id: user_id(),
            username: "bench".into(),
            display_name: None,
            role: None,
            banned: None,
        };
        let r = bookmarks::list_bookmarks(
            user,
            Query(bookmarks::BookmarkListParams {
                page: Some(1),
                page_size: Some(20),
            }),
            State(db()),
        )
        .await;
        assert!(r.is_ok(), "bookmarks/list must succeed (fixture user)");
        assert!(
            !r.unwrap().0.data.items.is_empty(),
            "bookmarks/list must return bookmarks (fixture)"
        );
    });
}

fn http_request(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(uri).header("X-API-Key", api_key());
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, format!("better-auth.session_token={}", c));
    }
    builder.body(Body::empty()).expect("request body")
}

fn bench_http_single<F>(c: &mut Criterion, name: &str, router: &Router, make_req: F, expect: u16)
where
    F: Fn() -> Request<Body>,
{
    c.bench_function(name, |b| {
        b.to_async(&*RT).iter(|| {
            let router = router.clone();
            let req = make_req();
            async move {
                let resp = router.oneshot(req).await.expect("oneshot");
                assert_eq!(resp.status().as_u16(), expect, "{} status", name);
                let _ = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .expect("consume body");
            }
        });
    });
}

fn bench_http_concurrency<F>(
    c: &mut Criterion,
    name: &str,
    router: &Router,
    make_req: F,
    concurrency: usize,
    expect: u16,
    samples: &Arc<Mutex<Vec<f64>>>,
) where
    F: Fn() -> Request<Body>,
{
    c.bench_function(name, |b| {
        b.to_async(&*RT).iter(|| {
            let router = router.clone();
            let reqs: Vec<Request<Body>> = (0..concurrency).map(|_| make_req()).collect();
            async move {
                let futs = reqs.into_iter().map(|req| {
                    let router = router.clone();
                    async move {
                        let t0 = std::time::Instant::now();
                        let resp = router.oneshot(req).await.expect("oneshot");
                        assert_eq!(resp.status().as_u16(), expect, "{} status", name);
                        let _ = axum::body::to_bytes(resp.into_body(), usize::MAX)
                            .await
                            .expect("consume body");
                        samples
                            .lock()
                            .unwrap()
                            .push(t0.elapsed().as_secs_f64() * 1000.0);
                    }
                });
                futures::future::join_all(futs).await;
            }
        });
    });
}

fn report_stats(label: &str, samples: &Arc<Mutex<Vec<f64>>>) {
    let mut v: Vec<f64> = samples.lock().unwrap().clone();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite latency"));
    if v.is_empty() {
        println!("{}: no samples collected", label);
        return;
    }
    let median = v[v.len() / 2];
    let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
    println!(
        "=== {}: n={} median={:.3}ms p95={:.3}ms",
        label,
        v.len(),
        median,
        p95
    );
}

fn bench_http(c: &mut Criterion) {
    let _ = &*RT;
    let _ = &*POOL;
    // Build inside the bench runtime so the analytics collector (if any) has a
    // reactor; the no-analytics router is the measurement group.
    let router = RT.block_on(async { build_app(app_state(), false) });

    // One-time wire-contract verification (not timed).
    RT.block_on(async {
        let resp = router
            .clone()
            .oneshot(http_request(
                "/v2/manga/list?page=1&pageSize=20&sortBy=latest",
                None,
            ))
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK, "http manga-list status");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["result"], "Success", "http manga-list envelope");
        assert_eq!(v["status"], 200, "http manga-list status field");

        let resp = router
            .clone()
            .oneshot(http_request("/v2/manga/search?query=hisureba", None))
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK, "http manga-search status");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(
            v["data"][0]["title"].as_str().unwrap().to_lowercase(),
            "hisureba",
            "http manga-search must rank hisureba first"
        );

        let resp = router
            .clone()
            .oneshot(http_request(
                &format!("/v2/manga/{}/chapter-ids", work_id()),
                None,
            ))
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK, "http chapter-ids status");

        let resp = router
            .clone()
            .oneshot(http_request("/v2/user/me", None))
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "http user-me 401");
    });

    bench_http_single(
        c,
        "http/manga-list",
        &router,
        || http_request("/v2/manga/list?page=1&pageSize=20&sortBy=latest", None),
        200,
    );
    bench_http_single(
        c,
        "http/manga-search",
        &router,
        || http_request("/v2/manga/search?query=hisureba", None),
        200,
    );
    bench_http_single(
        c,
        "http/manga-chapter-ids",
        &router,
        || http_request(&format!("/v2/manga/{}/chapter-ids", work_id()), None),
        200,
    );
    bench_http_single(
        c,
        "http/user-me",
        &router,
        || http_request("/v2/user/me", None),
        401,
    );
    bench_http_single(
        c,
        "http/bookmarks",
        &router,
        || http_request("/v2/bookmarks", Some(&session_token())),
        200,
    );

    // Concurrency series for the main manga-list path (per-request latency).
    let samples: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));
    let make_req = || http_request("/v2/manga/list?page=1&pageSize=20&sortBy=latest", None);
    for &n in &[1usize, 8, 32, 64] {
        let s = samples.clone();
        bench_http_concurrency(
            c,
            &format!("http/manga-list/c{}", n),
            &router,
            &make_req,
            n,
            200,
            &s,
        );
    }
    report_stats("http/manga-list concurrency (per-request latency)", &samples);
}

fn bench_http_prod(c: &mut Criterion) {
    let _ = &*RT;
    let _ = &*POOL;
    let router = RT.block_on(async { build_app(app_state(), true) });
    bench_http_single(
        c,
        "http-prod/manga-list",
        &router,
        || http_request("/v2/manga/list?page=1&pageSize=20&sortBy=latest", None),
        200,
    );
}

fn criterion() -> Criterion {
    Criterion::default()
        .sample_size(50)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
}

criterion_group! {
    name = benches;
    config = criterion();
    targets = bench_handlers, bench_http, bench_http_prod
}
criterion_main!(benches);
