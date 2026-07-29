use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub enum WorkFormat {
    #[serde(rename = "Manga")] Manga,
    #[serde(rename = "Manhwa")] Manhwa,
    #[serde(rename = "Manhua")] Manhua,
    #[serde(rename = "Webtoon")] Webtoon,
    #[serde(rename = "Doujinshi")] Doujinshi,
    #[serde(rename = "OneShot")] OneShot,
    #[serde(rename = "Comic")] Comic,
    #[serde(other)]
    Other,
}

impl From<&str> for WorkFormat {
    fn from(s: &str) -> Self {
        match s {
            "manga" => WorkFormat::Manga, "manhwa" => WorkFormat::Manhwa,
            "manhua" => WorkFormat::Manhua, "webtoon" => WorkFormat::Webtoon,
            "doujinshi" => WorkFormat::Doujinshi, "one_shot" => WorkFormat::OneShot,
            "comic" => WorkFormat::Comic, _ => WorkFormat::Other,
        }
    }
}

impl WorkFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkFormat::Manga => "Manga", WorkFormat::Manhwa => "Manhwa",
            WorkFormat::Manhua => "Manhua", WorkFormat::Webtoon => "Webtoon",
            WorkFormat::Doujinshi => "Doujinshi", WorkFormat::OneShot => "OneShot",
            WorkFormat::Comic => "Comic", WorkFormat::Other => "Other",
        }
    }
}
