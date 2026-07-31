use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::work::MangaResponse;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum RelationshipType {
    #[serde(rename = "prequel")]
    Prequel,
    #[serde(rename = "sequel")]
    Sequel,
    #[serde(rename = "spin_off")]
    SpinOff,
    #[serde(rename = "adaptation")]
    Adaptation,
    #[serde(rename = "alternate_version")]
    AlternateVersion,
    #[serde(rename = "side_story")]
    SideStory,
    #[serde(rename = "shared_universe")]
    SharedUniverse,
    #[serde(rename = "other")]
    Other,
}

impl From<&str> for RelationshipType {
    fn from(s: &str) -> Self {
        match s {
            "prequel" => RelationshipType::Prequel,
            "sequel" => RelationshipType::Sequel,
            "spin_off" => RelationshipType::SpinOff,
            "adaptation" => RelationshipType::Adaptation,
            "alternate_version" => RelationshipType::AlternateVersion,
            "side_story" => RelationshipType::SideStory,
            "shared_universe" => RelationshipType::SharedUniverse,
            "other" => RelationshipType::Other,
            _ => RelationshipType::Other,
        }
    }
}

impl From<String> for RelationshipType {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkRelationship {
    #[serde(rename = "relationshipType")]
    pub relationship_type: RelationshipType,
    pub manga: MangaResponse,
}
