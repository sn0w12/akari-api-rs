use std::sync::LazyLock;

use axum::extract::{Path, Query, State};
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;

use akari_api_rs::db::init_pool;
use akari_api_rs::handlers::*;

const WID: &str = "019f8fd0-37cd-7ce3-bc6e-077191378515";

fn wid() -> Uuid {
    WID.parse().unwrap()
}

static POOL: LazyLock<sqlx::PgPool> = LazyLock::new(|| {
    let _ = dotenvy::dotenv();
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/postgres".to_string());
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("rt");
        rt.block_on(init_pool(&url)).expect("connect DB")
    })
    .join()
    .expect("thread")
});

static RT: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().expect("rt"));

fn db() -> sqlx::PgPool {
    POOL.clone()
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

#[allow(unused_must_use)]
fn bench_handlers(c: &mut Criterion) {
    bench_one(c, "genre/list", || async {
        let _ = genre::list_genres(State(db())).await;
    });
    bench_one(c, "author/list", || async {
        let _ = author::list_authors(
            Query(author::AuthorListParams {
                page: Some(1),
                page_size: Some(20),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/list", || async {
        let _ = manga::list_manga(
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
    });
    bench_one(c, "manga/list-genre", || async {
        let _ = manga::list_manga(
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
    });
    bench_one(c, "manga/popular", || async {
        let _ = manga::popular_manga(
            Query(manga::PopularParams {
                page: Some(1),
                limit: Some(10),
                excluded_genres: None,
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/detail", || async {
        let _ = manga::manga_details(Path(wid()), State(db())).await;
    });
    bench_one(c, "manga/chapters", || async {
        let _ = manga::manga_chapters(Path(wid()), State(db())).await;
    });
    bench_one(c, "manga/recommendations", || async {
        let _ = manga::manga_recommendations(
            Path(wid()),
            Query(manga::PopularParams {
                page: Some(1),
                limit: Some(20),
                excluded_genres: None,
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/search", || async {
        let _ = manga::search_manga(
            Query(manga::SearchParams {
                query: Some("hisureba".into()),
                limit: Some(20),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/ids", || async {
        let _ = manga::manga_ids(
            Query(manga::MangaIdsParams {
                page: Some(1),
                page_size: Some(100),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/chapter-ids", || async {
        let _ = manga::chapter_ids(Path(wid()), State(db())).await;
    });
    bench_one(c, "manga/by-mal-id", || async {
        let _ = manga::by_mal_id(Path(92475), State(db())).await;
    });
    bench_one(c, "manga/by-ani-id", || async {
        let _ = manga::by_ani_id(Path(95863), State(db())).await;
    });
    bench_one(c, "manga/chapter-detail", || async {
        let _ = manga::chapter_detail(
            Path((wid(), 1.0)),
            Query(manga::ChapterDetailParams {
                scanlation_group_id: Some(0),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "manga/batch", || async {
        let _ = manga::batch_manga(
            Query(manga::BatchParams {
                ids: Some(WID.into()),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "user/profile", || async {
        let _ =
            user::user_profile(Path("sENy8w7haiU1VN3oVHkxvfihAQDuDhq8".into()), State(db())).await;
    });
    bench_one(c, "user/list", || async {
        let _ = user::list_users(
            Query(user::UserListParams {
                page: Some(1),
                page_size: Some(20),
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "comments/list", || async {
        let _ = comments::list_comments(
            Path(("work".to_string(), wid())),
            Query(comments::CommentListParams {
                page: Some(1),
                page_size: Some(20),
                sort: None,
            }),
            State(db()),
        )
        .await;
    });
    bench_one(c, "notifications/website", || async {
        let _ = notifications::website_notifications(State(db())).await;
    });
}

criterion_group!(benches, bench_handlers);
criterion_main!(benches);
