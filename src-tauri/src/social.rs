// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Social Media Hub — Tauri Commands
//!
//! Bridges the orchestrator's SocialMediaManager to the Svelte frontend.
//! Content creation and scheduling only — no OAuth/API posting (yet).
//! Generated content goes to a review queue that users can copy-paste.

use chrono::{DateTime, Utc, Weekday};
use serde::Serialize;

use crate::neuralswarm::get_orchestrator;
use crate::orchestrator::social_media::{
    ContentType, HookArchetype, Platform, PostStatus, SocialConfig,
};

// ════════════════════════════════════════════════════════════════
// DTOs for the frontend
// ════════════════════════════════════════════════════════════════

/// Platform connection status shown in dashboard cards.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformStatus {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub last_post_date: Option<String>,
    pub post_count: u64,
    pub engagement: PlatformEngagement,
    pub golden_hour: Option<GoldenHourInfo>,
}

/// Aggregated engagement for a platform.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PlatformEngagement {
    pub views: u64,
    pub likes: u64,
    pub comments: u64,
    pub shares: u64,
}

/// Golden hour info for frontend display.
#[derive(Debug, Clone, Serialize)]
pub struct GoldenHourInfo {
    pub best_days: Vec<String>,
    pub best_hour_utc: u32,
}

/// A queued post for the frontend content queue.
#[derive(Debug, Clone, Serialize)]
pub struct QueuedPost {
    pub id: String,
    pub platform: String,
    pub content_type: String,
    pub title: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
    pub status: String,
    pub scheduled_at: Option<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}

/// Content template for the template gallery.
#[derive(Debug, Clone, Serialize)]
pub struct ContentTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content_type: String,
    pub platforms: Vec<String>,
    pub template_body: String,
    pub hook_archetype: String,
}

// ════════════════════════════════════════════════════════════════
// Helper conversions
// ════════════════════════════════════════════════════════════════

fn platform_from_str(s: &str) -> Option<Platform> {
    match s.to_lowercase().as_str() {
        "github" => Some(Platform::GitHub),
        "linkedin" => Some(Platform::LinkedIn),
        "hackernews" | "hacker_news" => Some(Platform::HackerNews),
        "twitter" | "x" => Some(Platform::Twitter),
        "mastodon" => Some(Platform::Mastodon),
        "discord" => Some(Platform::Discord),
        _ => None,
    }
}

fn content_type_from_str(s: &str) -> ContentType {
    match s.to_lowercase().as_str() {
        "release_note" | "releasenote" => ContentType::ReleaseNote,
        "technical_article" | "technicalarticle" => ContentType::TechnicalArticle,
        "show_hn" | "showhn" => ContentType::ShowHN,
        "thread_series" | "threadseries" => ContentType::ThreadSeries,
        "engagement_reply" | "engagementreply" => ContentType::EngagementReply,
        "profile_update" | "profileupdate" => ContentType::ProfileUpdate,
        _ => ContentType::ProjectUpdate,
    }
}

fn platform_display_name(p: &Platform) -> &'static str {
    match p {
        Platform::GitHub => "GitHub",
        Platform::LinkedIn => "LinkedIn",
        Platform::HackerNews => "Hacker News",
        Platform::Twitter => "Twitter / X",
        Platform::Mastodon => "Mastodon",
        Platform::Discord => "Discord",
    }
}

fn weekday_name(d: &Weekday) -> &'static str {
    match d {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

fn format_status(s: &PostStatus) -> &'static str {
    match s {
        PostStatus::Draft => "draft",
        PostStatus::Queued => "queued",
        PostStatus::Scheduled => "scheduled",
        PostStatus::Published => "published",
        PostStatus::Failed => "failed",
        PostStatus::Cancelled => "cancelled",
    }
}

fn format_content_type(ct: &ContentType) -> &'static str {
    match ct {
        ContentType::ReleaseNote => "release_note",
        ContentType::TechnicalArticle => "technical_article",
        ContentType::ProjectUpdate => "project_update",
        ContentType::ShowHN => "show_hn",
        ContentType::ThreadSeries => "thread_series",
        ContentType::EngagementReply => "engagement_reply",
        ContentType::ProfileUpdate => "profile_update",
    }
}

fn ensure_social_manager_init(
    orch: &mut crate::orchestrator::ImpForgeOrchestrator,
) {
    if orch.social_manager_mut().is_none() {
        orch.init_social_media(SocialConfig::default());
    }
}

// ════════════════════════════════════════════════════════════════
// Tauri Commands
// ════════════════════════════════════════════════════════════════

/// Get the status of all supported platforms.
#[tauri::command]
pub async fn social_get_platforms() -> Result<Vec<PlatformStatus>, String> {
    let all_platforms = vec![
        Platform::GitHub,
        Platform::LinkedIn,
        Platform::HackerNews,
        Platform::Twitter,
        Platform::Mastodon,
        Platform::Discord,
    ];

    let golden_hours = crate::orchestrator::social_media::GoldenHour::defaults();

    let orch = get_orchestrator();

    // Build platform statuses — if orchestrator is available, pull post counts
    let mut statuses = Vec::with_capacity(all_platforms.len());

    if let Ok(orch_ref) = orch {
        let mut orch_guard = orch_ref.lock().await;
        ensure_social_manager_init(&mut orch_guard);

        if let Some(mgr) = orch_guard.social_manager_mut() {
            for platform in &all_platforms {
                let posts: Vec<_> = mgr
                    .posts_by_status(&PostStatus::Published)
                    .into_iter()
                    .filter(|p| &p.platform == platform)
                    .collect();

                let last_post = posts
                    .iter()
                    .filter_map(|p| p.published_at)
                    .max()
                    .map(|d| d.to_rfc3339());

                let mut engagement = PlatformEngagement::default();
                for post in &posts {
                    if let Some(e) = &post.engagement {
                        engagement.views += e.views;
                        engagement.likes += e.likes;
                        engagement.comments += e.comments;
                        engagement.shares += e.shares;
                    }
                }

                let gh = golden_hours
                    .iter()
                    .find(|g| &g.platform == platform)
                    .map(|g| GoldenHourInfo {
                        best_days: g.best_days.iter().map(|d| weekday_name(d).to_string()).collect(),
                        best_hour_utc: g.best_hour_utc,
                    });

                statuses.push(PlatformStatus {
                    id: format!("{platform}"),
                    name: platform_display_name(platform).to_string(),
                    connected: false, // OAuth not yet implemented
                    last_post_date: last_post,
                    post_count: posts.len() as u64,
                    engagement,
                    golden_hour: gh,
                });
            }
        }
    }

    // Fallback: if orchestrator not available, return defaults
    if statuses.is_empty() {
        for platform in &all_platforms {
            let gh = golden_hours
                .iter()
                .find(|g| &g.platform == platform)
                .map(|g| GoldenHourInfo {
                    best_days: g.best_days.iter().map(|d| weekday_name(d).to_string()).collect(),
                    best_hour_utc: g.best_hour_utc,
                });

            statuses.push(PlatformStatus {
                id: format!("{platform}"),
                name: platform_display_name(platform).to_string(),
                connected: false,
                last_post_date: None,
                post_count: 0,
                engagement: PlatformEngagement::default(),
                golden_hour: gh,
            });
        }
    }

    Ok(statuses)
}

/// Compose and queue a new post to one or more platforms.
#[tauri::command]
pub async fn social_compose_post(
    content: String,
    platforms: Vec<String>,
    content_type: Option<String>,
    title: Option<String>,
    tags: Option<Vec<String>>,
    schedule: Option<String>,
) -> Result<Vec<QueuedPost>, String> {
    if content.trim().is_empty() {
        return Err("Post content cannot be empty".to_string());
    }
    if platforms.is_empty() {
        return Err("Select at least one platform".to_string());
    }

    let parsed_platforms: Vec<Platform> = platforms
        .iter()
        .filter_map(|s| platform_from_str(s))
        .collect();

    if parsed_platforms.is_empty() {
        return Err("No valid platforms selected".to_string());
    }

    let ct = content_type
        .as_deref()
        .map(content_type_from_str)
        .unwrap_or(ContentType::ProjectUpdate);

    let post_tags = tags.unwrap_or_default();

    let scheduled_at: Option<DateTime<Utc>> = if let Some(ref s) = schedule {
        Some(
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| format!("Invalid schedule date: {e}"))?,
        )
    } else {
        None
    };

    let orch = get_orchestrator()?;
    let mut orch_guard = orch.lock().await;
    ensure_social_manager_init(&mut orch_guard);

    let mgr = orch_guard
        .social_manager_mut()
        .ok_or("Social manager not available")?;

    let mut created_posts = Vec::new();

    for platform in &parsed_platforms {
        let post = mgr.draft_post(
            platform.clone(),
            ct.clone(),
            title.as_deref(),
            &content,
            post_tags.clone(),
        );
        let post_id = post.id.clone();

        // If scheduled, set the schedule; otherwise queue it
        if let Some(at) = scheduled_at {
            mgr.schedule_post(&post_id, at);
        } else {
            mgr.queue_post(&post_id);
        }

        // Re-fetch the post to get updated status
        if let Some(updated) = mgr.posts_by_status(&PostStatus::Queued)
            .into_iter()
            .chain(mgr.posts_by_status(&PostStatus::Scheduled))
            .chain(mgr.posts_by_status(&PostStatus::Draft))
            .find(|p| p.id == post_id)
        {
            created_posts.push(QueuedPost {
                id: updated.id.clone(),
                platform: format!("{}", updated.platform),
                content_type: format_content_type(&updated.content_type).to_string(),
                title: updated.title.clone(),
                body: updated.body.clone(),
                tags: updated.tags.clone(),
                status: format_status(&updated.status).to_string(),
                scheduled_at: updated.scheduled_at.map(|d| d.to_rfc3339()),
                created_at: updated.created_at.to_rfc3339(),
                published_at: updated.published_at.map(|d| d.to_rfc3339()),
            });
        }
    }

    Ok(created_posts)
}

/// Generate post content using AI (Ollama) based on topic and style.
#[tauri::command]
pub async fn social_ai_generate(
    topic: String,
    platform: String,
    style: String,
) -> Result<String, String> {
    if topic.trim().is_empty() {
        return Err("Topic cannot be empty".to_string());
    }

    let platform_name = platform_from_str(&platform)
        .map(|p| platform_display_name(&p).to_string())
        .unwrap_or_else(|| platform.clone());

    // Map style to hook archetype prompt
    let hook_hint = match style.to_lowercase().as_str() {
        "problem_solution" => HookArchetype::ProblemSolution.prompt_template(),
        "contrarian" => HookArchetype::Contrarian.prompt_template(),
        "data_driven" => HookArchetype::DataDriven.prompt_template(),
        "personal_story" => HookArchetype::PersonalStory.prompt_template(),
        "tutorial" => HookArchetype::Tutorial.prompt_template(),
        "comparison" => HookArchetype::Comparison.prompt_template(),
        "prediction" => HookArchetype::Prediction.prompt_template(),
        _ => "Write an engaging post about {topic}...",
    };

    let system_prompt = format!(
        "You are a social media content creator for tech products. \
         Write a post for {platform_name} about the following topic. \
         Use the StoryBrand framework: identify the hero (developer), \
         their problem, and position the product as the guide. \
         Style hint: {hook_hint}\n\
         Keep it concise and platform-appropriate. \
         For {platform_name}, use the right tone and length.\n\
         Do NOT use markdown formatting unless the platform supports it.\n\
         Do NOT include hashtags unless asked."
    );

    let prompt = format!("Write a {platform_name} post about: {topic}");

    // Try Ollama first (offline-first)
    let ollama_url = std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let payload = serde_json::json!({
        "model": "dolphin3:8b",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt}
        ],
        "stream": false,
        "options": {
            "temperature": 0.8,
            "num_predict": 512
        }
    });

    let resp = client
        .post(format!("{ollama_url}/api/chat"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                "Ollama is not running. Start it with: ollama serve".to_string()
            } else {
                format!("Ollama request failed: {e}")
            }
        })?;

    if !resp.status().is_success() {
        return Err(format!(
            "Ollama returned HTTP {}. Ensure a model is available (ollama pull dolphin3:8b)",
            resp.status()
        ));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let generated = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("(AI generation returned empty content)")
        .to_string();

    Ok(generated)
}

/// Get all posts in the queue (all statuses except published).
#[tauri::command]
pub async fn social_get_queue() -> Result<Vec<QueuedPost>, String> {
    let orch = get_orchestrator();

    if let Ok(orch_ref) = orch {
        let mut orch_guard = orch_ref.lock().await;
        ensure_social_manager_init(&mut orch_guard);

        if let Some(mgr) = orch_guard.social_manager_mut() {
            let statuses = [
                PostStatus::Draft,
                PostStatus::Queued,
                PostStatus::Scheduled,
                PostStatus::Published,
                PostStatus::Failed,
            ];

            let mut posts = Vec::new();
            for status in &statuses {
                for post in mgr.posts_by_status(status) {
                    posts.push(QueuedPost {
                        id: post.id.clone(),
                        platform: format!("{}", post.platform),
                        content_type: format_content_type(&post.content_type).to_string(),
                        title: post.title.clone(),
                        body: post.body.clone(),
                        tags: post.tags.clone(),
                        status: format_status(&post.status).to_string(),
                        scheduled_at: post.scheduled_at.map(|d| d.to_rfc3339()),
                        created_at: post.created_at.to_rfc3339(),
                        published_at: post.published_at.map(|d| d.to_rfc3339()),
                    });
                }
            }

            // Sort by created_at descending
            posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            return Ok(posts);
        }
    }

    Ok(vec![])
}

/// Get pre-built content templates.
#[tauri::command]
pub async fn social_get_templates() -> Result<Vec<ContentTemplate>, String> {
    Ok(vec![
        ContentTemplate {
            id: "release-announcement".into(),
            name: "Release Announcement".into(),
            description: "Announce a new version with changelog highlights".into(),
            content_type: "release_note".into(),
            platforms: vec!["github".into(), "twitter".into(), "linkedin".into()],
            template_body: concat!(
                "We just shipped {project} v{version}!\n\n",
                "Highlights:\n",
                "- {feature_1}\n",
                "- {feature_2}\n",
                "- {feature_3}\n\n",
                "Try it out: {url}\n\n",
                "#opensource #devtools"
            ).into(),
            hook_archetype: "problem_solution".into(),
        },
        ContentTemplate {
            id: "technical-article".into(),
            name: "Technical Article Promotion".into(),
            description: "Share a blog post or technical deep-dive".into(),
            content_type: "technical_article".into(),
            platforms: vec!["linkedin".into(), "twitter".into(), "hackernews".into()],
            template_body: concat!(
                "I wrote about {topic} — here's what I learned:\n\n",
                "Key takeaway: {takeaway}\n\n",
                "Read the full article: {url}\n\n",
                "What's your experience with {topic}?"
            ).into(),
            hook_archetype: "personal_story".into(),
        },
        ContentTemplate {
            id: "show-hn".into(),
            name: "Show HN Post".into(),
            description: "Launch on Hacker News with a compelling Show HN".into(),
            content_type: "show_hn".into(),
            platforms: vec!["hackernews".into()],
            template_body: concat!(
                "Show HN: {project} — {tagline}\n\n",
                "{description}\n\n",
                "What it does:\n",
                "- {feature_1}\n",
                "- {feature_2}\n\n",
                "Tech stack: {tech_stack}\n\n",
                "Live demo: {url}\n",
                "Source: {repo_url}"
            ).into(),
            hook_archetype: "problem_solution".into(),
        },
        ContentTemplate {
            id: "project-update".into(),
            name: "Project Update".into(),
            description: "Share progress on your project with the community".into(),
            content_type: "project_update".into(),
            platforms: vec!["twitter".into(), "mastodon".into(), "discord".into()],
            template_body: concat!(
                "Progress update on {project}:\n\n",
                "This week:\n",
                "- {done_1}\n",
                "- {done_2}\n\n",
                "Next up: {next}\n\n",
                "Building in public — follow along!"
            ).into(),
            hook_archetype: "personal_story".into(),
        },
        ContentTemplate {
            id: "weekly-roundup".into(),
            name: "Weekly Dev Roundup".into(),
            description: "Curated list of interesting finds from the week".into(),
            content_type: "thread_series".into(),
            platforms: vec!["twitter".into(), "linkedin".into(), "mastodon".into()],
            template_body: concat!(
                "Weekly Dev Roundup ({date}):\n\n",
                "1. {item_1} — {desc_1}\n",
                "2. {item_2} — {desc_2}\n",
                "3. {item_3} — {desc_3}\n",
                "4. {item_4} — {desc_4}\n",
                "5. {item_5} — {desc_5}\n\n",
                "What did you discover this week?"
            ).into(),
            hook_archetype: "data_driven".into(),
        },
        ContentTemplate {
            id: "comparison-post".into(),
            name: "Tool Comparison".into(),
            description: "Compare tools, frameworks, or approaches".into(),
            content_type: "technical_article".into(),
            platforms: vec!["linkedin".into(), "twitter".into(), "hackernews".into()],
            template_body: concat!(
                "{tool_a} vs {tool_b}: Which one should you use?\n\n",
                "I tested both. Here's what I found:\n\n",
                "{tool_a}:\n",
                "+ {pro_a}\n",
                "- {con_a}\n\n",
                "{tool_b}:\n",
                "+ {pro_b}\n",
                "- {con_b}\n\n",
                "My verdict: {verdict}"
            ).into(),
            hook_archetype: "comparison".into(),
        },
    ])
}

/// Cancel a queued or scheduled post.
#[tauri::command]
pub async fn social_cancel_post(post_id: String) -> Result<bool, String> {
    let orch = get_orchestrator()?;
    let mut orch_guard = orch.lock().await;
    ensure_social_manager_init(&mut orch_guard);

    let mgr = orch_guard
        .social_manager_mut()
        .ok_or("Social manager not available")?;

    Ok(mgr.cancel_post(&post_id))
}

/// Mark a post as published (manual confirmation after copy-paste).
#[tauri::command]
pub async fn social_mark_published(post_id: String) -> Result<bool, String> {
    let orch = get_orchestrator()?;
    let mut orch_guard = orch.lock().await;
    ensure_social_manager_init(&mut orch_guard);

    let mgr = orch_guard
        .social_manager_mut()
        .ok_or("Social manager not available")?;

    Ok(mgr.mark_published(&post_id))
}
