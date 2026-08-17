use std::collections::HashSet;
use std::sync::OnceLock;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use handlebars::Handlebars;
use html_to_img::render::RenderConfig;
use html_to_img::svg::svg_data_uri;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::work::MangaResponse;

const OG_WIDTH: u32 = 1200;
const OG_HEIGHT: u32 = 630;

// Star shapes for the rating display. Blitz cannot style SVGs (no `fill`/
// `stroke`), so the stars are pre-rendered to PNG data URIs at startup.
const STAR_SIZE: u32 = 96;
const STAR_PATH: &str = "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 \
    0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 \
    0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 \
    0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 \
    0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 \
    0 0 0 1.597-1.16z";
const STAR_HALF_PATH: &str = "M12 18.338a2.1 2.1 0 0 0-.987.244L6.396 21.01a.53.53 \
    0 0 1-.77-.56l.881-5.139a2.12 2.12 0 0 0-.611-1.879L2.16 9.795a.53.53 \
    0 0 1 .294-.906l5.165-.755a2.12 2.12 0 0 0 1.597-1.16l2.309-4.679A.53.53 \
    0 0 1 12 2";

fn star_svg(paths: &[(&str, &str)]) -> String {
    let paths = paths
        .iter()
        .map(|(color, d)| format!(r#"<path stroke="{color}" d="{d}"/>"#))
        .collect::<String>();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"#
    )
}

/// Returns (full, half, empty) star PNG data URIs, rasterized once.
fn star_data_uris() -> (String, String, String) {
    static STAR_URIS: OnceLock<(String, String, String)> = OnceLock::new();
    STAR_URIS
        .get_or_init(|| {
            let full = svg_data_uri(&star_svg(&[("#f5f5f4", STAR_PATH)]), STAR_SIZE, STAR_SIZE)
                .expect("failed to rasterize full star");
            let half = svg_data_uri(
                &star_svg(&[("#737373", STAR_PATH), ("#f5f5f4", STAR_HALF_PATH)]),
                STAR_SIZE,
                STAR_SIZE,
            )
            .expect("failed to rasterize half star");
            let empty = svg_data_uri(&star_svg(&[("#737373", STAR_PATH)]), STAR_SIZE, STAR_SIZE)
                .expect("failed to rasterize empty star");
            (full, half, empty)
        })
        .clone()
}

macro_rules! register_template {
    ($handlebars:expr, $name:expr, $path:expr) => {
        $handlebars
            .register_template_string($name, include_str!($path))
            .expect(concat!("Failed to register template '", $name, "'"));
    };
}

fn handlebars() -> &'static Handlebars<'static> {
    static HANDLEBARS: OnceLock<Handlebars<'static>> = OnceLock::new();
    HANDLEBARS.get_or_init(|| {
        let mut hb = Handlebars::new();
        register_template!(hb, "manga", "../../assets/og/manga.hbs");
        register_template!(hb, "author", "../../assets/og/author.hbs");
        register_template!(hb, "general", "../../assets/og/general.hbs");
        hb
    })
}

#[derive(Serialize)]
enum Theme {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "")]
    None,
}

#[derive(Serialize)]
struct MangaOgGenres {
    pub default: Vec<String>,
    pub warning: Vec<String>,
    pub destructive: Vec<String>,
}

#[derive(Serialize)]
struct MangaOgStars {
    pub full: Vec<i8>,
    pub half: Vec<i8>,
    pub empty: Vec<i8>,
}

#[derive(Serialize)]
struct MangaOg {
    pub title: String,
    pub cover: String,
    pub authors: Vec<String>,
    pub genres: MangaOgGenres,
    pub stars: MangaOgStars,
    pub theme: Theme,
}

const WARNING_GENRES: &[&str] = &["ecchi"];
const DESTRUCTIVE_GENRES: &[&str] = &[
    "adult",
    "hentai",
    "mature",
    "shoujo_ai",
    "shounen_ai",
    "smut",
];

impl MangaOg {
    pub fn from_manga(manga: MangaResponse) -> Self {
        let warning_set: HashSet<_> = WARNING_GENRES.iter().cloned().collect();
        let destructive_set: HashSet<_> = DESTRUCTIVE_GENRES.iter().cloned().collect();

        let mut default = Vec::new();
        let mut warning = Vec::new();
        let mut destructive = Vec::new();

        for genre in &manga.genres {
            if destructive_set.contains(genre.as_str()) {
                destructive.push(genre.clone());
            } else if warning_set.contains(genre.as_str()) {
                warning.push(genre.clone());
            } else {
                default.push(genre.clone());
            }
        }

        let genres = MangaOgGenres {
            default,
            warning,
            destructive,
        };

        let stars = Self::compute_stars(manga.rating.average);

        MangaOg {
            title: manga.title,
            cover: manga.cover.url,
            authors: manga.authors,
            genres,
            stars,
            theme: Theme::Dark,
        }
    }

    /// Converts a floating-point average into three vectors of indices
    /// (0-based) for full, half, and empty stars.
    fn compute_stars(average: f64) -> MangaOgStars {
        const MAX_STARS: i8 = 5;
        // Map from 1-10 to 0-5 (e.g., 8.0 -> 4.0, 7.5 -> 3.75)
        let scaled = (average / 2.0).clamp(0.0, 5.0);
        // Round to the nearest half (0.0, 0.5, 1.0, ... 5.0)
        let rounded = (scaled * 2.0).round() / 2.0;

        let full_count = rounded.floor() as i8;
        let half = if (rounded - full_count as f64).abs() < f64::EPSILON {
            0
        } else {
            1
        };

        let full: Vec<i8> = (0..full_count).collect();

        let mut half_vec = Vec::new();
        if half == 1 {
            half_vec.push(full_count); // index of the half star (right after full ones)
        }

        let empty: Vec<i8> = (full_count + half..MAX_STARS).collect();

        MangaOgStars {
            full,
            half: half_vec,
            empty,
        }
    }
}

#[derive(Serialize)]
struct AuthorOg {
    pub title: String,
    pub covers: Vec<String>,
    pub theme: Theme,
}

#[derive(Serialize)]
struct GeneralOg {
    pub title: String,
    pub theme: Theme,
}

/// Serializes the template data, injects the bundled CSS and pre-rendered star
/// images, renders the Handlebars template to HTML, then rasterizes it to a PNG
/// image.
async fn render_template(name: &str, data: impl Serialize) -> Result<Vec<u8>, ApiError> {
    let mut context = serde_json::to_value(data)
        .map_err(|e| ApiError::internal(format!("Failed to serialize OG context: {e}")))?;
    context
        .as_object_mut()
        .ok_or_else(|| ApiError::internal("OG context is not an object"))?
        .insert("css".to_string(), json!(crate::CSS));

    let (star_full, star_half, star_empty) = star_data_uris();
    for (key, uri) in [
        ("star_full", &star_full),
        ("star_half", &star_half),
        ("star_empty", &star_empty),
    ] {
        context
            .as_object_mut()
            .ok_or_else(|| ApiError::internal("OG context is not an object"))?
            .insert(key.to_string(), json!(uri));
    }

    let html = handlebars()
        .render(name, &context)
        .map_err(|e| ApiError::internal(format!("Failed to render OG template '{name}': {e}")))?;

    let config = RenderConfig {
        width: OG_WIDTH,
        height: OG_HEIGHT,
        ..RenderConfig::default()
    };
    tokio::task::spawn_blocking(move || html_to_img::render_html_to_png(&html, &config))
        .await
        .map_err(|e| ApiError::internal(format!("OG render task failed: {e}")))?
        .map_err(|e| ApiError::internal(format!("Failed to rasterize OG image: {e}")))
}

fn png_response(png: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png")],
        Body::from(png),
    )
        .into_response()
}

/// GET /v2/og/manga/{id}
#[utoipa::path(get, path = "/v2/og/manga/{id}", tag = "og", responses(
    (status = 200, description = "Success", content_type = "image/png"),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_og(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Response, ApiError> {
    let row = sqlx::query_as::<_, crate::handlers::manga::MangaListRow>(
        crate::handlers::manga::MANGA_BY_ID_SQL,
    )
    .bind(id)
    .fetch_optional(&db)
    .await?
    .ok_or(ApiError::not_found("Manga not found"))?;

    let og = MangaOg::from_manga(row.into());
    let png = render_template("manga", og).await?;
    Ok(png_response(png))
}

/// GET /v2/og/author/{name}
#[utoipa::path(get, path = "/v2/og/author/{name}", tag = "og", responses(
    (status = 200, description = "Success", content_type = "image/png"),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn author_og(
    Path(name): Path<String>,
    State(db): State<DbPool>,
) -> Result<Response, ApiError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.authors WHERE name = $1)")
            .bind(&name)
            .fetch_one(&db)
            .await?;
    if !exists {
        return Err(ApiError::not_found("Author not found"));
    }

    let covers: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT ON (cov.work_id) cov.url \
         FROM public.covers cov \
         JOIN public.work_authors wa ON wa.work_id = cov.work_id \
         JOIN public.authors a ON a.id = wa.author_id \
         WHERE a.name = $1 AND cov.is_preferred = TRUE \
         ORDER BY cov.work_id, cov.url \
         LIMIT 6",
    )
    .bind(&name)
    .fetch_all(&db)
    .await?;

    let og = AuthorOg {
        title: name,
        covers,
        theme: Theme::Dark,
    };
    let png = render_template("author", og).await?;
    Ok(png_response(png))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct GeneralOgParams {
    pub title: String,
}

/// GET /v2/og/general
#[utoipa::path(get, path = "/v2/og/general", tag = "og", params(GeneralOgParams), responses(
    (status = 200, description = "Success", content_type = "image/png"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn general_og(Query(params): Query<GeneralOgParams>) -> Result<Response, ApiError> {
    let title = params.title.trim();
    if title.is_empty() {
        return Err(ApiError::bad_request("'title' query parameter is required"));
    }

    let og = GeneralOg {
        title: title.to_string(),
        theme: Theme::None,
    };
    let png = render_template("general", og).await?;
    Ok(png_response(png))
}
