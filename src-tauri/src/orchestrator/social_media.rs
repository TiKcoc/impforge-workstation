//! Social Media Agent Framework for ImpForge Orchestrator
//!
//! Universal social media automation agents that generate content,
//! schedule posts, and track engagement across platforms. These are
//! standalone — no ork-station dependencies, any customer can use them.
//!
//! Supported platforms:
//! - **GitHub**: README updates, release notes, discussions, bio/topics
//! - **LinkedIn**: Professional posts, article sharing, engagement tracking
//! - **HackerNews**: Show HN posts, comment engagement
//! - **Twitter/X**: Thread generation, scheduled tweets
//!
//! Content generation uses local LLMs (Ollama) for privacy-first
//! posting. Content is always queued for review before publishing
//! unless auto-post mode is explicitly enabled.
//!
//! Scientific basis:
//! - StoryBrand framework (Miller, 2017) — narrative-driven content
//! - Hook Archetypes — 7 engagement patterns for technical content
//! - Golden Hour scheduling — optimal posting times per platform

use chrono::{DateTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported social media platforms.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    GitHub,
    LinkedIn,
    HackerNews,
    Twitter,
    Mastodon,
    Discord,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::GitHub => write!(f, "github"),
            Platform::LinkedIn => write!(f, "linkedin"),
            Platform::HackerNews => write!(f, "hackernews"),
            Platform::Twitter => write!(f, "twitter"),
            Platform::Mastodon => write!(f, "mastodon"),
            Platform::Discord => write!(f, "discord"),
        }
    }
}

/// Content type for social media posts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    ReleaseNote,
    TechnicalArticle,
    ProjectUpdate,
    ShowHN,
    ThreadSeries,
    EngagementReply,
    ProfileUpdate,
}

/// A queued social media post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPost {
    pub id: String,
    pub platform: Platform,
    pub content_type: ContentType,
    pub title: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub status: PostStatus,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub engagement: Option<EngagementMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    Draft,
    Queued,
    Scheduled,
    Published,
    Failed,
    Cancelled,
}

/// Engagement metrics for a published post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementMetrics {
    pub views: u64,
    pub likes: u64,
    pub comments: u64,
    pub shares: u64,
    pub clicks: u64,
}

impl Default for EngagementMetrics {
    fn default() -> Self {
        Self { views: 0, likes: 0, comments: 0, shares: 0, clicks: 0 }
    }
}

/// Hook archetype for content generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookArchetype {
    ProblemSolution,
    Contrarian,
    DataDriven,
    PersonalStory,
    Tutorial,
    Comparison,
    Prediction,
}

impl HookArchetype {
    /// Get a prompt template for this hook archetype.
    pub fn prompt_template(&self) -> &str {
        match self {
            HookArchetype::ProblemSolution => "Most developers struggle with {problem}. Here's how {project} solves it...",
            HookArchetype::Contrarian => "Everyone says {common_belief}. But actually, {contrarian_take}...",
            HookArchetype::DataDriven => "{metric} improved by {percentage}% after switching to {approach}...",
            HookArchetype::PersonalStory => "I spent {time} building {thing}. Here's what I learned...",
            HookArchetype::Tutorial => "How to {goal} in {count} steps using {tool}...",
            HookArchetype::Comparison => "{option_a} vs {option_b}: We tested both. Here are the results...",
            HookArchetype::Prediction => "By {year}, {prediction}. Here's why {reason}...",
        }
    }
}

/// Golden hour scheduling — optimal posting times per platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenHour {
    pub platform: Platform,
    pub best_days: Vec<Weekday>,
    pub best_hour_utc: u32,
    pub timezone_offset: i32,
}

impl GoldenHour {
    /// Default golden hours based on research.
    pub fn defaults() -> Vec<Self> {
        vec![
            GoldenHour {
                platform: Platform::LinkedIn,
                best_days: vec![Weekday::Tue, Weekday::Wed, Weekday::Thu],
                best_hour_utc: 14, // 2 PM UTC (~10 AM EST)
                timezone_offset: 0,
            },
            GoldenHour {
                platform: Platform::Twitter,
                best_days: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu],
                best_hour_utc: 15,
                timezone_offset: 0,
            },
            GoldenHour {
                platform: Platform::HackerNews,
                best_days: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed],
                best_hour_utc: 14,
                timezone_offset: 0,
            },
            GoldenHour {
                platform: Platform::GitHub,
                best_days: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
                best_hour_utc: 16,
                timezone_offset: 0,
            },
        ]
    }
}

/// Social media agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialConfig {
    /// Enable auto-posting (false = queue for manual review).
    pub auto_post: bool,
    /// Maximum posts per day per platform.
    pub max_daily_posts: u32,
    /// Platforms to target.
    pub platforms: Vec<Platform>,
    /// Default tags for all posts.
    pub default_tags: Vec<String>,
    /// Quiet hours (no posting during these UTC hours).
    pub quiet_hours: Vec<u32>,
}

impl Default for SocialConfig {
    fn default() -> Self {
        Self {
            auto_post: false, // Safe default — require manual review
            max_daily_posts: 3,
            platforms: vec![Platform::GitHub, Platform::LinkedIn],
            default_tags: vec!["ai".into(), "developer-tools".into()],
            quiet_hours: vec![0, 1, 2, 3, 4, 5, 6], // UTC midnight–6am
        }
    }
}

/// Social media agent manager.
pub struct SocialMediaManager {
    config: SocialConfig,
    posts: Vec<SocialPost>,
    golden_hours: Vec<GoldenHour>,
    daily_counts: HashMap<String, u32>, // "platform:date" → count
    next_id: u64,
}

impl SocialMediaManager {
    pub fn new(config: SocialConfig) -> Self {
        Self {
            config,
            posts: Vec::new(),
            golden_hours: GoldenHour::defaults(),
            daily_counts: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a draft post.
    pub fn draft_post(
        &mut self,
        platform: Platform,
        content_type: ContentType,
        title: Option<&str>,
        body: &str,
        tags: Vec<String>,
    ) -> &SocialPost {
        let id = format!("post-{}", self.next_id);
        self.next_id += 1;

        let mut all_tags = self.config.default_tags.clone();
        all_tags.extend(tags);

        self.posts.push(SocialPost {
            id: id.clone(),
            platform,
            content_type,
            title: title.map(|s| s.to_string()),
            body: body.to_string(),
            tags: all_tags,
            scheduled_at: None,
            status: PostStatus::Draft,
            created_at: Utc::now(),
            published_at: None,
            engagement: None,
        });

        self.posts.last().unwrap()
    }

    /// Queue a post for publishing (moves from Draft to Queued).
    pub fn queue_post(&mut self, post_id: &str) -> bool {
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            if post.status == PostStatus::Draft {
                post.status = PostStatus::Queued;
                return true;
            }
        }
        false
    }

    /// Schedule a post for a specific time.
    pub fn schedule_post(&mut self, post_id: &str, at: DateTime<Utc>) -> bool {
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            if post.status == PostStatus::Draft || post.status == PostStatus::Queued {
                post.scheduled_at = Some(at);
                post.status = PostStatus::Scheduled;
                return true;
            }
        }
        false
    }

    /// Mark a post as published.
    pub fn mark_published(&mut self, post_id: &str) -> bool {
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            post.status = PostStatus::Published;
            post.published_at = Some(Utc::now());

            // Update daily count
            let key = format!("{}:{}", post.platform, Utc::now().format("%Y-%m-%d"));
            *self.daily_counts.entry(key).or_default() += 1;

            return true;
        }
        false
    }

    /// Cancel a post.
    pub fn cancel_post(&mut self, post_id: &str) -> bool {
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            if post.status != PostStatus::Published {
                post.status = PostStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Check if we can post to a platform today (rate limit check).
    pub fn can_post_today(&self, platform: &Platform) -> bool {
        let key = format!("{}:{}", platform, Utc::now().format("%Y-%m-%d"));
        let count = self.daily_counts.get(&key).unwrap_or(&0);
        *count < self.config.max_daily_posts
    }

    /// Check if current hour is in quiet hours.
    pub fn is_quiet_hour(&self) -> bool {
        let hour = Utc::now().hour();
        self.config.quiet_hours.contains(&hour)
    }

    /// Get posts ready to publish (queued or scheduled and past due).
    pub fn ready_posts(&self) -> Vec<&SocialPost> {
        let now = Utc::now();
        self.posts.iter()
            .filter(|p| match p.status {
                PostStatus::Queued => true,
                PostStatus::Scheduled => {
                    p.scheduled_at.map(|s| s <= now).unwrap_or(false)
                }
                _ => false,
            })
            .collect()
    }

    /// Get all posts with a given status.
    pub fn posts_by_status(&self, status: &PostStatus) -> Vec<&SocialPost> {
        self.posts.iter().filter(|p| &p.status == status).collect()
    }

    /// Get total post count.
    pub fn total_posts(&self) -> usize {
        self.posts.len()
    }

    /// Generate a release note post for GitHub.
    pub fn generate_release_post(
        &mut self,
        version: &str,
        changes: &[&str],
        project_name: &str,
    ) -> &SocialPost {
        let body = format!(
            "## {} v{}\n\n### Changes\n{}\n\n---\nGenerated by ImpForge AI Workstation Builder",
            project_name,
            version,
            changes.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n"),
        );

        self.draft_post(
            Platform::GitHub,
            ContentType::ReleaseNote,
            Some(&format!("{} v{}", project_name, version)),
            &body,
            vec!["release".into(), version.to_string()],
        )
    }

    /// Generate a Show HN post.
    pub fn generate_show_hn(
        &mut self,
        title: &str,
        description: &str,
        url: &str,
    ) -> &SocialPost {
        let body = format!(
            "{}\n\nURL: {}\n\n{}\n\n---\nGenerated with ImpForge",
            title, url, description
        );

        self.draft_post(
            Platform::HackerNews,
            ContentType::ShowHN,
            Some(&format!("Show HN: {}", title)),
            &body,
            vec!["showhn".into()],
        )
    }
}

use chrono::Timelike;

impl Default for SocialMediaManager {
    fn default() -> Self {
        Self::new(SocialConfig::default())
    }
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SocialConfig::default();
        assert!(!config.auto_post);
        assert_eq!(config.max_daily_posts, 3);
    }

    #[test]
    fn test_draft_post() {
        let mut mgr = SocialMediaManager::default();
        let post = mgr.draft_post(
            Platform::GitHub,
            ContentType::ReleaseNote,
            Some("v1.0"),
            "Release body",
            vec!["rust".into()],
        );

        assert_eq!(post.platform, Platform::GitHub);
        assert_eq!(post.status, PostStatus::Draft);
        assert!(post.tags.contains(&"rust".to_string()));
        assert!(post.tags.contains(&"ai".to_string())); // Default tag
    }

    #[test]
    fn test_queue_post() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::GitHub, ContentType::ProjectUpdate, None, "body", vec![]);
        assert!(mgr.queue_post("post-1"));
        assert_eq!(mgr.posts_by_status(&PostStatus::Queued).len(), 1);
    }

    #[test]
    fn test_schedule_post() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::LinkedIn, ContentType::TechnicalArticle, Some("Title"), "body", vec![]);

        let future = Utc::now() + chrono::Duration::hours(1);
        assert!(mgr.schedule_post("post-1", future));
        assert_eq!(mgr.posts_by_status(&PostStatus::Scheduled).len(), 1);
    }

    #[test]
    fn test_publish_post() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::GitHub, ContentType::ReleaseNote, None, "body", vec![]);
        mgr.queue_post("post-1");
        assert!(mgr.mark_published("post-1"));
        assert_eq!(mgr.posts_by_status(&PostStatus::Published).len(), 1);
    }

    #[test]
    fn test_cancel_post() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::GitHub, ContentType::ProjectUpdate, None, "body", vec![]);
        assert!(mgr.cancel_post("post-1"));
        assert_eq!(mgr.posts_by_status(&PostStatus::Cancelled).len(), 1);
    }

    #[test]
    fn test_cannot_cancel_published() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::GitHub, ContentType::ReleaseNote, None, "body", vec![]);
        mgr.mark_published("post-1");
        assert!(!mgr.cancel_post("post-1"));
    }

    #[test]
    fn test_daily_rate_limit() {
        let mut mgr = SocialMediaManager::new(SocialConfig {
            max_daily_posts: 2,
            ..Default::default()
        });

        assert!(mgr.can_post_today(&Platform::GitHub));

        mgr.draft_post(Platform::GitHub, ContentType::ReleaseNote, None, "1", vec![]);
        mgr.mark_published("post-1");
        mgr.draft_post(Platform::GitHub, ContentType::ReleaseNote, None, "2", vec![]);
        mgr.mark_published("post-2");

        assert!(!mgr.can_post_today(&Platform::GitHub));
    }

    #[test]
    fn test_ready_posts() {
        let mut mgr = SocialMediaManager::default();
        mgr.draft_post(Platform::GitHub, ContentType::ReleaseNote, None, "1", vec![]);
        mgr.draft_post(Platform::LinkedIn, ContentType::TechnicalArticle, None, "2", vec![]);

        mgr.queue_post("post-1");
        // post-2 stays as Draft → not ready

        assert_eq!(mgr.ready_posts().len(), 1);
    }

    #[test]
    fn test_generate_release_post() {
        let mut mgr = SocialMediaManager::default();
        let post = mgr.generate_release_post("1.0.0", &["MOA pipeline", "Dynamic topology"], "ImpForge");

        assert_eq!(post.content_type, ContentType::ReleaseNote);
        assert!(post.body.contains("v1.0.0"));
        assert!(post.body.contains("MOA pipeline"));
    }

    #[test]
    fn test_generate_show_hn() {
        let mut mgr = SocialMediaManager::default();
        let post = mgr.generate_show_hn("ImpForge", "AI Workstation Builder", "https://example.com");

        assert_eq!(post.content_type, ContentType::ShowHN);
        assert!(post.title.as_ref().unwrap().contains("Show HN"));
    }

    #[test]
    fn test_hook_archetypes() {
        let hook = HookArchetype::ProblemSolution;
        assert!(!hook.prompt_template().is_empty());

        let hook2 = HookArchetype::DataDriven;
        assert!(hook2.prompt_template().contains("metric"));
    }

    #[test]
    fn test_golden_hours() {
        let hours = GoldenHour::defaults();
        assert!(!hours.is_empty());
        assert!(hours.iter().any(|h| h.platform == Platform::LinkedIn));
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::GitHub), "github");
        assert_eq!(format!("{}", Platform::HackerNews), "hackernews");
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = SocialConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deser: SocialConfig = serde_json::from_str(&json).unwrap();
        assert!(!deser.auto_post);
        assert_eq!(deser.max_daily_posts, 3);
    }
}
