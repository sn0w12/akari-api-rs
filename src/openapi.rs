use utoipa::OpenApi;

use crate::models::bookmark::*;
use crate::models::chapter::*;
use crate::models::comment::*;
use crate::models::list::*;
use crate::models::manga_type::WorkFormat;
use crate::models::user::{UserProfileDetailsResponse, UserResponse, UserRole};
use crate::models::work::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        // Manga
        crate::handlers::manga::list_manga,
        crate::handlers::manga::popular_manga,
        crate::handlers::manga::search_manga,
        crate::handlers::manga::manga_ids,
        crate::handlers::manga::batch_manga,
        crate::handlers::manga::by_mal_id,
        crate::handlers::manga::by_ani_id,
        crate::handlers::manga::get_manga,
        crate::handlers::manga::manga_details,
        crate::handlers::manga::manga_chapters,
        crate::handlers::manga::global_chapter_ids,
        crate::handlers::manga::manga_recommendations,
        crate::handlers::manga::chapter_ids,
        crate::handlers::manga::chapter_detail,
        crate::handlers::manga::record_view,
        crate::handlers::manga::recently_viewed,
        crate::handlers::manga::rate_manga,
        crate::handlers::manga::get_rating,
        crate::handlers::manga::delete_rating,
        crate::handlers::manga::batch_rate,
        crate::handlers::manga::batch_by_mal,
        crate::handlers::manga::batch_by_ani,
        // Genre
        crate::handlers::genre::list_genres,
        crate::handlers::genre::manga_by_genre,
        // Author
        crate::handlers::author::list_authors,
        crate::handlers::author::manga_by_author,
        // User
        crate::handlers::user::list_users,
        crate::handlers::user::user_profile,
        crate::handlers::user::me,
        crate::handlers::user::update_profile,
        // Comments
        crate::handlers::comments::list_comments,
        crate::handlers::comments::list_comments_by_target,
        crate::handlers::comments::get_comment_replies,
        crate::handlers::comments::create_comment,
        crate::handlers::comments::update_comment,
        crate::handlers::comments::delete_comment,
        crate::handlers::comments::vote_comment,
        crate::handlers::comments::get_votes,
        crate::handlers::comments::report_comment,
        // Bookmarks
        crate::handlers::bookmarks::list_bookmarks,
        crate::handlers::bookmarks::search_bookmarks,
        crate::handlers::bookmarks::unread_count,
        crate::handlers::bookmarks::batch_upsert,
        crate::handlers::bookmarks::upsert_bookmark,
        crate::handlers::bookmarks::delete_bookmark,
        crate::handlers::bookmarks::get_bookmark,
        crate::handlers::bookmarks::reading_history,
        crate::handlers::bookmarks::reading_stats,
        // Lists
        crate::handlers::lists::list_user_lists,
        crate::handlers::lists::list_my_lists,
        crate::handlers::lists::list_ids_containing_manga,
        crate::handlers::lists::get_list,
        crate::handlers::lists::create_list,
        crate::handlers::lists::delete_list,
        crate::handlers::lists::add_entry,
        crate::handlers::lists::remove_entry,
        crate::handlers::lists::update_entry,
        // MAL
        crate::handlers::mal::token_exchange,
        crate::handlers::mal::get_manga_list,
        crate::handlers::mal::update_manga_list,
        crate::handlers::mal::me,
        crate::handlers::mal::logout,
        // AniList
        crate::handlers::anilist::me,
        crate::handlers::anilist::logout,
        crate::handlers::anilist::get_manga_list,
        crate::handlers::anilist::update_manga_list,
        // Notifications
        crate::handlers::notifications::subscribe,
        crate::handlers::notifications::website_notifications,
        crate::handlers::notifications::send_notification,
    ),
    components(
        schemas(
            MangaResponse,
            MangaDetailResponse,
            MangaSearchResponse,
            MangaIdsResponse,
            RatingResponse,
            ChapterResponse,
            ChapterNavigation,

            UserResponse,
            UserProfileDetailsResponse,
            UserRole,
            CommentResponse,
            CommentReportReason,
            CommentSortOrder,
            CommentWithRepliesResponse,
            PaginatedCommentResponse,
            CommentVoteResponse,
            BookmarkResponse,
            BookmarkDetailResponse,
            HistoryBucket,
            DayOfWeekReadCount,
            HourReadCount,
            ReadingHistoryResponse,
            ReadingHistoryTimelineEntry,
            ReadingStatsResponse,
            GenreCount,
            UserListResponse,
            UserListDetailResponse,
            ListEntryResponse,
            WorkFormat,
        )
    ),
    tags(
        (name = "manga", description = "Manga catalogue"),
        (name = "genre", description = "Genre endpoints"),
        (name = "author", description = "Author endpoints"),
        (name = "user", description = "Users & profiles"),
        (name = "comments", description = "Comments & votes"),
        (name = "bookmarks", description = "User library"),
        (name = "lists", description = "User-created lists"),
        (name = "mal", description = "MyAnimeList proxy"),
        (name = "anilist", description = "AniList proxy"),
        (name = "notifications", description = "Push notifications"),
    )
)]
pub struct ApiDoc;
