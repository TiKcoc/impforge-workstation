//! ImpForge Style Engine — BenikUI-Inspired Deep Sub-Component Customization
//!
//! Every widget is decomposed into independently styleable sub-components:
//! container, bars, text labels, value displays, borders, glow effects.
//! Each sub-element has its own font, color, position offset, animation,
//! glow intensity, and number format configuration.
//!
//! Architecture inspired by BenikUI (WoW addon):
//! - Fractal customization: every detail is independently configurable
//! - Position offsets: sub-elements can be moved relative to their parent
//! - Font system: face, size, weight, outline, shadow per text element
//! - Glow/aura effects: neon borders, text glow, bar shimmer
//! - Number formatting: percent, current/max, abbreviated (1.2K), animated
//! - Animation presets: fade, pulse, flash-on-change, slide
//! - Dynamic backgrounds: gradients, patterns, animated effects
//!
//! References:
//! - BenikUI (WoW addon — fractal sub-component customization, GPL → clean-room MIT)
//! - ElvUI profile system (GPL → clean-room MIT reimplementation)
//! - Grafana panel styling (Apache-2.0 reference)
//! - CSS Custom Properties Level 2 (W3C spec)
//!
//! License: MIT (all original code)

use serde::{Deserialize, Serialize};

// ============================================================================
// TEXT STYLE — Font face, size, weight, outline, shadow, position offset
// ============================================================================

/// Available font families — 20+ bundled via Fontsource (SIL OFL / MIT)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontFamily {
    /// Default system UI font (Inter Variable)
    System,
    /// Monospace for numbers/code (JetBrains Mono Variable)
    Mono,
    /// Display font for headings (Space Grotesk Variable)
    Display,
    /// Geometric sans-serif (clean, modern)
    Geometric,
    /// Rounded, friendly UI font
    Rounded,
    /// Condensed/narrow for dense layouts
    Condensed,
    /// Handwriting/script (casual)
    Handwriting,
    /// Pixel/retro monospace
    Pixel,
    /// Serif for editorial/document feel
    Serif,
    /// Slab-serif (strong, techy)
    SlabSerif,
    /// Futuristic/sci-fi (Orbitron, Rajdhani style)
    Futuristic,
    /// Gaming/aggressive (Teko, Bungee style)
    Gaming,
    /// Tabular numbers optimized (same-width digits)
    TabularNums,
    /// Custom font name (user-bundled or system)
    Custom(String),
}

impl Default for FontFamily {
    fn default() -> Self {
        Self::System
    }
}

/// Text outline/stroke style (BenikUI-style text borders) — 8 variations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TextOutline {
    None,
    /// 1px subtle outline
    Thin,
    /// 2px visible outline
    Medium,
    /// 3px bold outline
    Thick,
    /// Colored outline (uses glow color)
    ColoredThin,
    /// Double outline (inner white + outer colored)
    DoubleOutline,
    /// Shadow-based outline (softer, via text-shadow)
    ShadowOutline,
    /// Embossed/engraved 3D text effect
    Embossed,
}

impl Default for TextOutline {
    fn default() -> Self {
        Self::None
    }
}

/// Number display format (BenikUI health/mana bar patterns) — 12 formats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NumberFormat {
    /// Raw number: "12345"
    Raw,
    /// Abbreviated: "12.3K", "1.5M"
    Abbreviated,
    /// Percentage: "85%"
    Percent,
    /// Current / Max: "8500 / 10000"
    CurrentMax,
    /// Current / Max + Percent: "8500 / 10000 (85%)"
    CurrentMaxPercent,
    /// Hidden
    Hidden,
    /// Deficit: "-1500" (max - current, for damage/deficit)
    Deficit,
    /// Bytes/data size: "1.5 GB", "256 MB"
    Bytes,
    /// Duration: "2h 35m", "45s"
    Duration,
    /// Scientific: "1.23e4"
    Scientific,
    /// Compact with unit: "12.3K req/s"
    CompactWithUnit,
    /// Locale-formatted: "12,345" or "12.345" depending on locale
    Locale,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self::Raw
    }
}

/// Complete text styling for any text sub-element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font family
    pub font_family: FontFamily,
    /// Font size in pixels (8-72)
    pub font_size: f32,
    /// Font weight (100-900, 400=normal, 700=bold)
    pub font_weight: u16,
    /// Text color (hex with alpha: #RRGGBBAA or #RRGGBB)
    pub color: String,
    /// Text outline/stroke
    pub outline: TextOutline,
    /// Text shadow (CSS text-shadow format: "2px 2px 4px #000000")
    pub shadow: Option<String>,
    /// Position offset from natural position (x, y) in pixels
    pub offset: (f32, f32),
    /// Number display format (for numeric values)
    pub number_format: NumberFormat,
    /// Letter spacing in pixels (-2.0 to 10.0)
    pub letter_spacing: f32,
    /// Text transform: none, uppercase, lowercase, capitalize
    pub text_transform: String,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Whether to show this text element
    pub visible: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: FontFamily::System,
            font_size: 13.0,
            font_weight: 400,
            color: "#e8e8ed".into(),
            outline: TextOutline::None,
            shadow: None,
            offset: (0.0, 0.0),
            number_format: NumberFormat::Raw,
            letter_spacing: 0.0,
            text_transform: "none".into(),
            opacity: 1.0,
            visible: true,
        }
    }
}

// ============================================================================
// BAR STYLE — Progress bars, health bars, usage meters
// ============================================================================

/// Bar fill direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BarFillDirection {
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

impl Default for BarFillDirection {
    fn default() -> Self {
        Self::LeftToRight
    }
}

/// Bar texture/pattern — 20 variations (BenikUI-level customization)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BarTexture {
    /// Solid flat fill
    Flat,
    /// Subtle gradient overlay
    Gradient,
    /// Striped pattern (like ElvUI diagonal stripes)
    Striped,
    /// Glossy/glass effect (top-half highlight)
    Glossy,
    /// Minimalist thin bar
    Minimalist,
    /// Textured noise overlay
    Noise,
    /// Horizontal thin lines
    Lined,
    /// Pixel/retro blocky look
    Pixelated,
    /// Checkerboard micro-pattern
    Checkerboard,
    /// Brushed metal effect
    BrushedMetal,
    /// Diamond crosshatch pattern
    Diamond,
    /// Honeycomb hexagonal pattern
    Honeycomb,
    /// Circuit board trace pattern
    Circuit,
    /// Wave/ripple effect
    Wave,
    /// Frosted glass (blur + opacity)
    Frosted,
    /// Carbon fiber weave
    CarbonFiber,
    /// Scanline retro CRT effect
    Scanline,
    /// Neon edge glow (fill + glowing edge)
    NeonEdge,
    /// Dual-tone split gradient
    DualTone,
    /// Animated shimmer sweep
    Shimmer,
}

impl Default for BarTexture {
    fn default() -> Self {
        Self::Flat
    }
}

/// Color threshold — changes bar color based on value (BenikUI health bar pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorThreshold {
    /// Value threshold (0.0 to 1.0, e.g. 0.25 = 25%)
    pub threshold: f32,
    /// Color to use when value is at or below this threshold
    pub color: String,
}

/// Complete bar styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarStyle {
    /// Primary bar fill color
    pub color: String,
    /// Background (empty) color
    pub background_color: String,
    /// Bar texture/pattern
    pub texture: BarTexture,
    /// Fill direction
    pub fill_direction: BarFillDirection,
    /// Corner radius in pixels
    pub border_radius: f32,
    /// Bar height in pixels (for horizontal bars)
    pub height: f32,
    /// Whether bar has a gradient overlay
    pub gradient: bool,
    /// Gradient end color (if gradient=true)
    pub gradient_end_color: Option<String>,
    /// Color thresholds (sorted by threshold, descending)
    /// e.g. [{ threshold: 0.25, color: "#FF3366" }, { threshold: 0.5, color: "#FFCC00" }]
    pub color_thresholds: Vec<ColorThreshold>,
    /// Whether to animate value changes
    pub animate_changes: bool,
    /// Animation duration in ms
    pub animation_duration_ms: u32,
    /// Spark/shimmer effect on the fill edge
    pub spark_effect: bool,
    /// Position offset from natural position (x, y)
    pub offset: (f32, f32),
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Whether to show this bar
    pub visible: bool,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            color: "#00FF66".into(),
            background_color: "#1a1a24".into(),
            texture: BarTexture::Flat,
            fill_direction: BarFillDirection::LeftToRight,
            border_radius: 4.0,
            height: 16.0,
            gradient: false,
            gradient_end_color: None,
            color_thresholds: vec![
                ColorThreshold { threshold: 0.25, color: "#FF3366".into() },
                ColorThreshold { threshold: 0.50, color: "#FFCC00".into() },
            ],
            animate_changes: true,
            animation_duration_ms: 300,
            spark_effect: false,
            offset: (0.0, 0.0),
            opacity: 1.0,
            visible: true,
        }
    }
}

// ============================================================================
// BORDER STYLE — Frame borders with neon glow options
// ============================================================================

/// Border pattern style — 15 variations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BorderPattern {
    Solid,
    Dashed,
    Dotted,
    Double,
    /// No border
    None,
    /// 3D ridge effect
    Ridge,
    /// 3D groove effect
    Groove,
    /// 3D inset (sunken)
    Inset,
    /// 3D outset (raised)
    Outset,
    /// Neon glow border (rendered via box-shadow)
    NeonGlow,
    /// Gradient border (via background-clip)
    GradientBorder,
    /// Animated dashed (marching ants)
    MarchingAnts,
    /// Corner-only brackets [ ]
    Corners,
    /// Rounded pill shape (large radius)
    Pill,
    /// Pixelated retro border
    PixelBorder,
}

impl Default for BorderPattern {
    fn default() -> Self {
        Self::Solid
    }
}

/// Complete border styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderStyle {
    /// Border pattern
    pub pattern: BorderPattern,
    /// Border width in pixels
    pub width: f32,
    /// Border color
    pub color: String,
    /// Corner radius in pixels
    pub radius: f32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Whether to show this border
    pub visible: bool,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            pattern: BorderPattern::Solid,
            width: 1.0,
            color: "#2a2a3a".into(),
            radius: 8.0,
            opacity: 1.0,
            visible: true,
        }
    }
}

// ============================================================================
// GLOW STYLE — Neon aura and light effects (BenikUI signature look)
// ============================================================================

/// Glow/aura effect type — 15 variations (neon, fire, frost, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GlowType {
    /// No glow
    None,
    /// Box shadow glow (around container)
    BoxGlow,
    /// Text shadow glow (around text)
    TextGlow,
    /// Inner glow (inset shadow)
    InnerGlow,
    /// Combined outer + inner
    DualGlow,
    /// Multi-layer neon (white core + colored outer, like real neon tubes)
    NeonMultiLayer,
    /// Soft ambient glow (large blur, low opacity)
    AmbientGlow,
    /// Sharp edge glow (small blur, high intensity)
    EdgeGlow,
    /// Pulsing ring (animated border-radius glow)
    PulsingRing,
    /// Fire/ember effect (orange-red gradient glow)
    FireGlow,
    /// Frost/ice effect (cyan-white crystalline glow)
    FrostGlow,
    /// Electric/lightning (flickering, multiple colors)
    ElectricGlow,
    /// Holographic (rainbow shift glow)
    HolographicGlow,
    /// Drop shadow only (directional, not glow)
    DropShadow,
    /// Neon underline (glow on bottom edge only)
    NeonUnderline,
}

impl Default for GlowType {
    fn default() -> Self {
        Self::None
    }
}

/// Complete glow/aura styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlowStyle {
    /// Glow type
    pub glow_type: GlowType,
    /// Glow color
    pub color: String,
    /// Glow intensity/spread in pixels (0 = off, 1-5 = subtle, 5-20 = medium, 20+ = strong)
    pub intensity: f32,
    /// Blur radius in pixels
    pub blur: f32,
    /// Whether glow pulses/animates
    pub animated: bool,
    /// Pulse speed in ms (if animated)
    pub pulse_duration_ms: u32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
}

impl Default for GlowStyle {
    fn default() -> Self {
        Self {
            glow_type: GlowType::None,
            color: "#00FF66".into(),
            intensity: 8.0,
            blur: 16.0,
            animated: false,
            pulse_duration_ms: 2000,
            opacity: 0.3,
        }
    }
}

// ============================================================================
// ANIMATION CONFIG — Transitions, entrance effects, value change animations
// ============================================================================

/// Animation preset type — 20 variations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnimationType {
    /// No animation
    None,
    /// Fade in/out
    Fade,
    /// Scale up/down
    Scale,
    /// Slide from direction
    SlideIn,
    /// Pulse on value change
    PulseOnChange,
    /// Flash bright then settle (for alerts)
    Flash,
    /// Smooth counting number animation
    CountUp,
    /// Continuous subtle breathing
    Breathe,
    /// Bounce entrance (overshoot + settle)
    Bounce,
    /// Elastic spring (wobble + settle)
    Elastic,
    /// Flip/rotate 3D entrance
    Flip,
    /// Typewriter text reveal
    Typewriter,
    /// Shake/vibrate (error/alert)
    Shake,
    /// Blur in (from blurry to sharp)
    BlurIn,
    /// Glitch effect (RGB split + jitter)
    Glitch,
    /// Matrix/rain digital effect
    MatrixRain,
    /// Ripple/wave propagation
    Ripple,
    /// Morph/transform shape
    Morph,
    /// Stagger children sequentially
    StaggerChildren,
    /// Heartbeat (double-pulse like ECG)
    Heartbeat,
}

impl Default for AnimationType {
    fn default() -> Self {
        Self::None
    }
}

/// Easing function for animations — 15 curves
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Spring physics (overshoot + settle)
    Spring,
    /// Bounce at end (ball drop)
    BounceOut,
    /// Elastic wobble
    ElasticOut,
    /// Back/overshoot then return
    BackOut,
    /// Smooth step (S-curve, no overshoot)
    SmoothStep,
    /// Exponential acceleration
    ExpoIn,
    /// Exponential deceleration
    ExpoOut,
    /// Circular arc
    CircOut,
    /// Sine wave
    SineInOut,
    /// Steps (discrete jumps, like clock)
    Steps,
    /// Custom cubic-bezier (user-defined)
    CustomBezier,
}

impl Default for Easing {
    fn default() -> Self {
        Self::EaseOut
    }
}

/// Complete animation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Animation type
    pub animation_type: AnimationType,
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Easing function
    pub easing: Easing,
    /// Delay before animation starts (ms)
    pub delay_ms: u32,
    /// Whether animation repeats
    pub repeat: bool,
    /// Respect prefers-reduced-motion (WCAG 2.3.3)
    pub respect_reduced_motion: bool,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            animation_type: AnimationType::None,
            duration_ms: 300,
            easing: Easing::EaseOut,
            delay_ms: 0,
            repeat: false,
            respect_reduced_motion: true, // WCAG compliance by default
        }
    }
}

// ============================================================================
// BACKGROUND STYLE — Dynamic backgrounds, gradients, patterns
// ============================================================================

/// Background type — 20 variations (BenikUI-level background options)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackgroundType {
    /// Solid color
    Solid,
    /// Linear gradient
    LinearGradient,
    /// Radial gradient
    RadialGradient,
    /// Conic gradient (sweep/pie)
    ConicGradient,
    /// Subtle noise/texture pattern
    Pattern,
    /// Transparent
    Transparent,
    /// Glassmorphism (blur + semi-transparent)
    Glass,
    /// Mesh gradient (multi-point color blend)
    MeshGradient,
    /// Animated gradient (color cycling)
    AnimatedGradient,
    /// Striped diagonal background
    DiagonalStripes,
    /// Dot grid pattern
    DotGrid,
    /// Cross-hatch pattern
    Crosshatch,
    /// Hexagonal honeycomb
    Hexagons,
    /// Carbon fiber texture
    CarbonFiber,
    /// Circuit board lines
    CircuitBoard,
    /// Star field (scattered dots)
    Starfield,
    /// Topographic contour lines
    Topographic,
    /// Gradient mesh with noise
    NoiseGradient,
    /// Repeating wave/sine pattern
    Waves,
    /// Image/texture URL
    Image,
}

impl Default for BackgroundType {
    fn default() -> Self {
        Self::Solid
    }
}

/// Complete background styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStyle {
    /// Background type
    pub bg_type: BackgroundType,
    /// Primary color
    pub color: String,
    /// Secondary color (for gradients)
    pub color_end: Option<String>,
    /// Gradient angle in degrees (for linear gradient)
    pub gradient_angle: f32,
    /// Pattern name (if bg_type == Pattern)
    pub pattern: Option<String>,
    /// Whether background animates (gradient shift, etc.)
    pub animated: bool,
    /// Animation duration in ms
    pub animation_duration_ms: u32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Blur/backdrop-filter in pixels (glassmorphism)
    pub backdrop_blur: f32,
}

impl Default for BackgroundStyle {
    fn default() -> Self {
        Self {
            bg_type: BackgroundType::Solid,
            color: "#13131a".into(),
            color_end: None,
            gradient_angle: 180.0,
            pattern: None,
            animated: false,
            animation_duration_ms: 5000,
            opacity: 1.0,
            backdrop_blur: 0.0,
        }
    }
}

// ============================================================================
// COMPONENT STYLE — The root style object for any widget sub-component
// ============================================================================

/// A complete style configuration for a single sub-component.
/// This is the "atom" of the BenikUI-style customization system.
/// Every widget is composed of multiple ComponentStyles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStyle {
    /// Unique identifier: "widget-id.sub-component-name"
    /// e.g. "system-stats.cpu-bar", "system-stats.title-text"
    pub id: String,
    /// Human-readable label for the settings UI
    pub label: String,
    /// Parent component ID (for hierarchy)
    pub parent_id: Option<String>,
    /// Position offset from parent (x, y) in pixels
    pub offset: (f32, f32),
    /// Size override (width, height) — None means auto/inherit
    pub size: Option<(f32, f32)>,
    /// Z-index override (for layering)
    pub z_index: i32,
    /// Background style
    pub background: BackgroundStyle,
    /// Border style
    pub border: BorderStyle,
    /// Glow/aura effect
    pub glow: GlowStyle,
    /// Text style (for text sub-components)
    pub text: Option<TextStyle>,
    /// Bar style (for progress/meter sub-components)
    pub bar: Option<BarStyle>,
    /// Animation configuration
    pub animation: AnimationConfig,
    /// Padding (top, right, bottom, left) in pixels
    pub padding: (f32, f32, f32, f32),
    /// Whether this component is visible
    pub visible: bool,
}

impl Default for ComponentStyle {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            parent_id: None,
            offset: (0.0, 0.0),
            size: None,
            z_index: 0,
            background: BackgroundStyle::default(),
            border: BorderStyle::default(),
            glow: GlowStyle::default(),
            text: None,
            bar: None,
            animation: AnimationConfig::default(),
            padding: (4.0, 8.0, 4.0, 8.0),
            visible: true,
        }
    }
}

// ============================================================================
// WIDGET STYLE MAP — All sub-component styles for a widget instance
// ============================================================================

/// Complete style configuration for one widget instance.
/// Maps sub-component IDs to their styles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetStyleMap {
    /// Widget ID this style applies to
    pub widget_id: String,
    /// All sub-component styles (keyed by sub-component name)
    pub components: Vec<ComponentStyle>,
}

/// Get default sub-component styles for a widget type.
/// This defines the "skeleton" of each widget — what sub-components exist.
pub fn default_widget_styles(widget_id: &str) -> WidgetStyleMap {
    match widget_id {
        "system-stats" => system_stats_default_styles(),
        "agent-pool" => agent_pool_default_styles(),
        "quick-chat" => quick_chat_default_styles(),
        "docker-overview" => generic_card_styles(widget_id, "Docker Overview"),
        "github-feed" => generic_card_styles(widget_id, "GitHub Feed"),
        "browser-sessions" => generic_card_styles(widget_id, "Browser Sessions"),
        "network-waterfall" => generic_card_styles(widget_id, "Network Waterfall"),
        "model-status" => model_status_default_styles(),
        "eval-pipeline" => generic_card_styles(widget_id, "Eval Pipeline"),
        "news-ticker" => generic_card_styles(widget_id, "News Ticker"),
        "workflow-status" => generic_card_styles(widget_id, "Workflow Status"),
        "console-output" => generic_card_styles(widget_id, "Console Output"),
        _ => generic_card_styles(widget_id, widget_id),
    }
}

/// System Stats widget: CPU bar, RAM bar, GPU bar, temperature text, title
fn system_stats_default_styles() -> WidgetStyleMap {
    WidgetStyleMap {
        widget_id: "system-stats".into(),
        components: vec![
            // Container
            ComponentStyle {
                id: "system-stats.container".into(),
                label: "Container".into(),
                background: BackgroundStyle {
                    color: "#13131a".into(),
                    opacity: 0.95,
                    backdrop_blur: 8.0,
                    ..Default::default()
                },
                border: BorderStyle {
                    color: "#2a2a3a".into(),
                    radius: 8.0,
                    ..Default::default()
                },
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            // Title text
            ComponentStyle {
                id: "system-stats.title".into(),
                label: "Title".into(),
                parent_id: Some("system-stats.container".into()),
                text: Some(TextStyle {
                    font_size: 11.0,
                    font_weight: 600,
                    color: "#a0a0b0".into(),
                    text_transform: "uppercase".into(),
                    letter_spacing: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            // CPU bar
            ComponentStyle {
                id: "system-stats.cpu-bar".into(),
                label: "CPU Bar".into(),
                parent_id: Some("system-stats.container".into()),
                bar: Some(BarStyle {
                    color: "#00FF66".into(),
                    height: 14.0,
                    border_radius: 3.0,
                    color_thresholds: vec![
                        ColorThreshold { threshold: 0.90, color: "#FF3366".into() },
                        ColorThreshold { threshold: 0.70, color: "#FFCC00".into() },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            // CPU label
            ComponentStyle {
                id: "system-stats.cpu-label".into(),
                label: "CPU Label".into(),
                parent_id: Some("system-stats.cpu-bar".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono,
                    font_size: 10.0,
                    font_weight: 500,
                    color: "#e8e8ed".into(),
                    number_format: NumberFormat::Percent,
                    outline: TextOutline::Thin,
                    ..Default::default()
                }),
                ..Default::default()
            },
            // RAM bar
            ComponentStyle {
                id: "system-stats.ram-bar".into(),
                label: "RAM Bar".into(),
                parent_id: Some("system-stats.container".into()),
                bar: Some(BarStyle {
                    color: "#00CCFF".into(),
                    height: 14.0,
                    border_radius: 3.0,
                    color_thresholds: vec![
                        ColorThreshold { threshold: 0.90, color: "#FF3366".into() },
                        ColorThreshold { threshold: 0.70, color: "#FFCC00".into() },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            // RAM label
            ComponentStyle {
                id: "system-stats.ram-label".into(),
                label: "RAM Label".into(),
                parent_id: Some("system-stats.ram-bar".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono,
                    font_size: 10.0,
                    font_weight: 500,
                    color: "#e8e8ed".into(),
                    number_format: NumberFormat::CurrentMaxPercent,
                    outline: TextOutline::Thin,
                    ..Default::default()
                }),
                ..Default::default()
            },
            // GPU bar
            ComponentStyle {
                id: "system-stats.gpu-bar".into(),
                label: "GPU Bar".into(),
                parent_id: Some("system-stats.container".into()),
                bar: Some(BarStyle {
                    color: "#9933FF".into(),
                    height: 14.0,
                    border_radius: 3.0,
                    color_thresholds: vec![
                        ColorThreshold { threshold: 0.90, color: "#FF3366".into() },
                        ColorThreshold { threshold: 0.70, color: "#FFCC00".into() },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            // GPU label
            ComponentStyle {
                id: "system-stats.gpu-label".into(),
                label: "GPU Label".into(),
                parent_id: Some("system-stats.gpu-bar".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono,
                    font_size: 10.0,
                    font_weight: 500,
                    color: "#e8e8ed".into(),
                    number_format: NumberFormat::Percent,
                    outline: TextOutline::Thin,
                    ..Default::default()
                }),
                ..Default::default()
            },
            // Temperature text
            ComponentStyle {
                id: "system-stats.temp-text".into(),
                label: "Temperature".into(),
                parent_id: Some("system-stats.container".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono,
                    font_size: 18.0,
                    font_weight: 700,
                    color: "#FFCC00".into(),
                    number_format: NumberFormat::Raw,
                    ..Default::default()
                }),
                glow: GlowStyle {
                    glow_type: GlowType::TextGlow,
                    color: "#FFCC00".into(),
                    intensity: 4.0,
                    blur: 8.0,
                    opacity: 0.2,
                    ..Default::default()
                },
                ..Default::default()
            },
        ],
    }
}

/// Agent Pool widget
fn agent_pool_default_styles() -> WidgetStyleMap {
    WidgetStyleMap {
        widget_id: "agent-pool".into(),
        components: vec![
            ComponentStyle {
                id: "agent-pool.container".into(),
                label: "Container".into(),
                background: BackgroundStyle {
                    color: "#13131a".into(),
                    opacity: 0.95,
                    ..Default::default()
                },
                border: BorderStyle { color: "#2a2a3a".into(), radius: 8.0, ..Default::default() },
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            ComponentStyle {
                id: "agent-pool.title".into(),
                label: "Title".into(),
                parent_id: Some("agent-pool.container".into()),
                text: Some(TextStyle {
                    font_size: 11.0, font_weight: 600, color: "#a0a0b0".into(),
                    text_transform: "uppercase".into(), letter_spacing: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ComponentStyle {
                id: "agent-pool.agent-count".into(),
                label: "Agent Count".into(),
                parent_id: Some("agent-pool.container".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono, font_size: 24.0, font_weight: 700,
                    color: "#00FF66".into(), number_format: NumberFormat::Raw,
                    ..Default::default()
                }),
                glow: GlowStyle {
                    glow_type: GlowType::TextGlow, color: "#00FF66".into(),
                    intensity: 6.0, blur: 12.0, opacity: 0.25,
                    ..Default::default()
                },
                animation: AnimationConfig {
                    animation_type: AnimationType::CountUp, duration_ms: 500,
                    ..Default::default()
                },
                ..Default::default()
            },
            ComponentStyle {
                id: "agent-pool.status-indicator".into(),
                label: "Status Indicator".into(),
                parent_id: Some("agent-pool.container".into()),
                glow: GlowStyle {
                    glow_type: GlowType::BoxGlow, color: "#00FF66".into(),
                    intensity: 4.0, blur: 8.0, animated: true, pulse_duration_ms: 2000,
                    opacity: 0.3,
                },
                ..Default::default()
            },
        ],
    }
}

/// Quick Chat widget
fn quick_chat_default_styles() -> WidgetStyleMap {
    WidgetStyleMap {
        widget_id: "quick-chat".into(),
        components: vec![
            ComponentStyle {
                id: "quick-chat.container".into(),
                label: "Container".into(),
                background: BackgroundStyle {
                    color: "#0d0d12".into(), opacity: 0.98,
                    backdrop_blur: 12.0,
                    ..Default::default()
                },
                border: BorderStyle { color: "#2a2a3a".into(), radius: 10.0, ..Default::default() },
                padding: (0.0, 0.0, 0.0, 0.0),
                ..Default::default()
            },
            ComponentStyle {
                id: "quick-chat.header".into(),
                label: "Header".into(),
                parent_id: Some("quick-chat.container".into()),
                background: BackgroundStyle {
                    bg_type: BackgroundType::LinearGradient,
                    color: "#1a1a24".into(),
                    color_end: Some("#13131a".into()),
                    gradient_angle: 180.0,
                    ..Default::default()
                },
                text: Some(TextStyle {
                    font_size: 12.0, font_weight: 600, color: "#e8e8ed".into(),
                    ..Default::default()
                }),
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            ComponentStyle {
                id: "quick-chat.messages".into(),
                label: "Message Area".into(),
                parent_id: Some("quick-chat.container".into()),
                text: Some(TextStyle {
                    font_size: 13.0, color: "#e8e8ed".into(),
                    ..Default::default()
                }),
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            ComponentStyle {
                id: "quick-chat.input".into(),
                label: "Input Field".into(),
                parent_id: Some("quick-chat.container".into()),
                background: BackgroundStyle { color: "#1a1a24".into(), ..Default::default() },
                border: BorderStyle { color: "#2a2a3a".into(), radius: 6.0, ..Default::default() },
                text: Some(TextStyle {
                    font_size: 13.0, color: "#e8e8ed".into(),
                    ..Default::default()
                }),
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
        ],
    }
}

/// Model Status widget
fn model_status_default_styles() -> WidgetStyleMap {
    WidgetStyleMap {
        widget_id: "model-status".into(),
        components: vec![
            ComponentStyle {
                id: "model-status.container".into(),
                label: "Container".into(),
                background: BackgroundStyle { color: "#13131a".into(), opacity: 0.95, ..Default::default() },
                border: BorderStyle { color: "#2a2a3a".into(), radius: 8.0, ..Default::default() },
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            ComponentStyle {
                id: "model-status.title".into(),
                label: "Title".into(),
                parent_id: Some("model-status.container".into()),
                text: Some(TextStyle {
                    font_size: 11.0, font_weight: 600, color: "#a0a0b0".into(),
                    text_transform: "uppercase".into(), letter_spacing: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ComponentStyle {
                id: "model-status.model-name".into(),
                label: "Model Name".into(),
                parent_id: Some("model-status.container".into()),
                text: Some(TextStyle {
                    font_family: FontFamily::Mono, font_size: 14.0, font_weight: 600,
                    color: "#00CCFF".into(), ..Default::default()
                }),
                ..Default::default()
            },
            ComponentStyle {
                id: "model-status.health-dot".into(),
                label: "Health Indicator".into(),
                parent_id: Some("model-status.container".into()),
                glow: GlowStyle {
                    glow_type: GlowType::BoxGlow, color: "#00FF66".into(),
                    intensity: 3.0, blur: 6.0, animated: true, pulse_duration_ms: 3000,
                    opacity: 0.4,
                },
                ..Default::default()
            },
        ],
    }
}

/// Generic card-style widget defaults
fn generic_card_styles(widget_id: &str, label: &str) -> WidgetStyleMap {
    WidgetStyleMap {
        widget_id: widget_id.into(),
        components: vec![
            ComponentStyle {
                id: format!("{widget_id}.container"),
                label: "Container".into(),
                background: BackgroundStyle { color: "#13131a".into(), opacity: 0.95, ..Default::default() },
                border: BorderStyle { color: "#2a2a3a".into(), radius: 8.0, ..Default::default() },
                padding: (8.0, 12.0, 8.0, 12.0),
                ..Default::default()
            },
            ComponentStyle {
                id: format!("{widget_id}.title"),
                label: "Title".into(),
                parent_id: Some(format!("{widget_id}.container")),
                text: Some(TextStyle {
                    font_size: 11.0, font_weight: 600, color: "#a0a0b0".into(),
                    text_transform: "uppercase".into(), letter_spacing: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ComponentStyle {
                id: format!("{widget_id}.content"),
                label: format!("{label} Content"),
                parent_id: Some(format!("{widget_id}.container")),
                text: Some(TextStyle {
                    font_size: 13.0, color: "#e8e8ed".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ],
    }
}

// ============================================================================
// FONT REGISTRY — Available fonts for the style system
// ============================================================================

/// Available font entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontEntry {
    pub name: String,
    pub family: String,
    pub category: String, // "sans-serif", "monospace", "display"
    pub is_variable: bool,
    pub bundled: bool,
}

/// Get list of available fonts (bundled + system fonts)
/// All fonts are SIL Open Font License or Apache-2.0 — safe for commercial use
pub fn available_fonts() -> Vec<FontEntry> {
    vec![
        // === BUNDLED (shipped with ImpForge via Fontsource, offline-first) ===
        FontEntry { name: "Inter".into(), family: "'Inter Variable', 'Inter', system-ui, sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: true },
        FontEntry { name: "JetBrains Mono".into(), family: "'JetBrains Mono Variable', 'JetBrains Mono', monospace".into(), category: "monospace".into(), is_variable: true, bundled: true },
        FontEntry { name: "Space Grotesk".into(), family: "'Space Grotesk Variable', 'Space Grotesk', sans-serif".into(), category: "display".into(), is_variable: true, bundled: true },
        FontEntry { name: "System UI".into(), family: "system-ui, -apple-system, sans-serif".into(), category: "sans-serif".into(), is_variable: false, bundled: true },

        // === GEOMETRIC (clean, modern sans-serif) ===
        FontEntry { name: "Outfit".into(), family: "'Outfit Variable', 'Outfit', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Plus Jakarta Sans".into(), family: "'Plus Jakarta Sans Variable', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "DM Sans".into(), family: "'DM Sans Variable', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Nunito".into(), family: "'Nunito Variable', 'Nunito', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },

        // === MONOSPACE (code/numbers) ===
        FontEntry { name: "Fira Code".into(), family: "'Fira Code Variable', 'Fira Code', monospace".into(), category: "monospace".into(), is_variable: true, bundled: false },
        FontEntry { name: "Source Code Pro".into(), family: "'Source Code Pro Variable', monospace".into(), category: "monospace".into(), is_variable: true, bundled: false },
        FontEntry { name: "IBM Plex Mono".into(), family: "'IBM Plex Mono', monospace".into(), category: "monospace".into(), is_variable: false, bundled: false },
        FontEntry { name: "Inconsolata".into(), family: "'Inconsolata Variable', monospace".into(), category: "monospace".into(), is_variable: true, bundled: false },

        // === DISPLAY / HEADING ===
        FontEntry { name: "Orbitron".into(), family: "'Orbitron Variable', 'Orbitron', sans-serif".into(), category: "display".into(), is_variable: true, bundled: false },
        FontEntry { name: "Rajdhani".into(), family: "'Rajdhani', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Chakra Petch".into(), family: "'Chakra Petch', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Audiowide".into(), family: "'Audiowide', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Russo One".into(), family: "'Russo One', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },

        // === ROUNDED ===
        FontEntry { name: "Quicksand".into(), family: "'Quicksand Variable', 'Quicksand', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Comfortaa".into(), family: "'Comfortaa Variable', 'Comfortaa', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },

        // === CONDENSED ===
        FontEntry { name: "Barlow Condensed".into(), family: "'Barlow Condensed', sans-serif".into(), category: "sans-serif".into(), is_variable: false, bundled: false },
        FontEntry { name: "Oswald".into(), family: "'Oswald Variable', 'Oswald', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Roboto Condensed".into(), family: "'Roboto Condensed Variable', sans-serif".into(), category: "sans-serif".into(), is_variable: true, bundled: false },

        // === PIXEL / RETRO ===
        FontEntry { name: "Share Tech Mono".into(), family: "'Share Tech Mono', monospace".into(), category: "monospace".into(), is_variable: false, bundled: false },
        FontEntry { name: "Press Start 2P".into(), family: "'Press Start 2P', monospace".into(), category: "monospace".into(), is_variable: false, bundled: false },
        FontEntry { name: "VT323".into(), family: "'VT323', monospace".into(), category: "monospace".into(), is_variable: false, bundled: false },
        FontEntry { name: "Silkscreen".into(), family: "'Silkscreen', monospace".into(), category: "monospace".into(), is_variable: false, bundled: false },

        // === SERIF ===
        FontEntry { name: "Playfair Display".into(), family: "'Playfair Display Variable', serif".into(), category: "serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Lora".into(), family: "'Lora Variable', serif".into(), category: "serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Merriweather".into(), family: "'Merriweather', serif".into(), category: "serif".into(), is_variable: false, bundled: false },

        // === SLAB SERIF ===
        FontEntry { name: "Roboto Slab".into(), family: "'Roboto Slab Variable', serif".into(), category: "serif".into(), is_variable: true, bundled: false },
        FontEntry { name: "Zilla Slab".into(), family: "'Zilla Slab', serif".into(), category: "serif".into(), is_variable: false, bundled: false },

        // === HANDWRITING ===
        FontEntry { name: "Caveat".into(), family: "'Caveat Variable', cursive".into(), category: "handwriting".into(), is_variable: true, bundled: false },
        FontEntry { name: "Patrick Hand".into(), family: "'Patrick Hand', cursive".into(), category: "handwriting".into(), is_variable: false, bundled: false },

        // === FUTURISTIC / GAMING ===
        FontEntry { name: "Exo 2".into(), family: "'Exo 2 Variable', sans-serif".into(), category: "display".into(), is_variable: true, bundled: false },
        FontEntry { name: "Teko".into(), family: "'Teko', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Play".into(), family: "'Play', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Michroma".into(), family: "'Michroma', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
        FontEntry { name: "Titillium Web".into(), family: "'Titillium Web', sans-serif".into(), category: "display".into(), is_variable: false, bundled: false },
    ]
}

// ============================================================================
// GRAPH STYLE — Chart/graph display configuration
// ============================================================================

/// Graph/chart type — D3/Vega-Lite inspired taxonomy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphType {
    /// Mini sparkline (compact trend indicator)
    Sparkline,
    /// Area chart with fill
    Area,
    /// Line chart (time series)
    Line,
    /// Bar chart (vertical)
    BarChart,
    /// Radial/gauge meter
    Gauge,
    /// Donut/pie chart
    Donut,
    /// Stacked area (composition over time)
    StackedArea,
    /// Horizontal bar chart
    HorizontalBar,
    /// Grouped bar chart (multi-series comparison)
    GroupedBar,
    /// Radar/spider chart (multivariate comparison)
    Radar,
    /// Heatmap (2D density/intensity)
    Heatmap,
    /// Treemap (hierarchical proportions)
    TreeMap,
    /// Scatter plot (correlation)
    Scatter,
    /// Waterfall chart (cumulative changes)
    Waterfall,
    /// Funnel chart (progressive reduction)
    Funnel,
    /// Candlestick (OHLC financial)
    Candlestick,
    /// Sankey diagram (flow/allocation)
    Sankey,
    /// Bubble chart (3-variable scatter)
    Bubble,
    /// Step line (discrete changes)
    StepLine,
    /// Polar area (angular comparison)
    PolarArea,
}

impl Default for GraphType {
    fn default() -> Self {
        Self::Sparkline
    }
}

/// Graph display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStyle {
    /// Graph type
    pub graph_type: GraphType,
    /// Line/fill color
    pub color: String,
    /// Fill opacity (for area charts)
    pub fill_opacity: f32,
    /// Line width in pixels
    pub line_width: f32,
    /// Whether to show data points
    pub show_points: bool,
    /// Whether to show grid lines
    pub show_grid: bool,
    /// Grid line color
    pub grid_color: String,
    /// Whether to show axis labels
    pub show_labels: bool,
    /// Number of data points to display
    pub data_points: u32,
    /// Whether chart animates new data points
    pub animate: bool,
    /// Smooth curves (bezier) vs angular (linear)
    pub smooth: bool,
}

impl Default for GraphStyle {
    fn default() -> Self {
        Self {
            graph_type: GraphType::Sparkline,
            color: "#00FF66".into(),
            fill_opacity: 0.15,
            line_width: 2.0,
            show_points: false,
            show_grid: false,
            grid_color: "#1a1a24".into(),
            show_labels: false,
            data_points: 30,
            animate: true,
            smooth: true,
        }
    }
}

// ============================================================================
// THEME PRESETS — Complete visual configuration snapshots (BenikUI-style)
// ============================================================================

/// Pre-built theme presets — one-click complete UI restyling
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThemePreset {
    /// Default ImpForge dark theme (neon green accents)
    ImpForgeDefault,
    /// Deep space cyberpunk (magenta/cyan neon, dark backgrounds)
    Cyberpunk,
    /// Ice blue / frost white, frosted glass
    Arctic,
    /// Warm orange/red embers, dark charcoal
    Ember,
    /// Purple/gold luxury feel
    Imperial,
    /// Full green terminal retro (Matrix-style)
    Matrix,
    /// Navy/gold professional look
    Corporate,
    /// Pink/purple gradient vapor
    Synthwave,
    /// Earth tones, warm greens, muted
    Forest,
    /// White/light gray, dark text (light mode)
    Daylight,
    /// Deep ocean blue, aqua accents
    DeepSea,
    /// Red/black aggressive gamer aesthetic
    Crimson,
    /// Pastel colors, rounded, friendly
    Candy,
    /// Pure grayscale, no color accents
    Monochrome,
    /// Solar amber/orange, warm dark
    SolarFlare,
    /// High contrast accessibility theme
    HighContrast,
    /// Minimal borders, subtle shadows, clean
    Minimal,
    /// Retro pixel art colors (8-bit palette)
    Retro8Bit,
    /// Holographic rainbow iridescence
    Holographic,
    /// Custom user-defined preset (stored in SQLite)
    Custom(String),
}

impl Default for ThemePreset {
    fn default() -> Self {
        Self::ImpForgeDefault
    }
}

/// Theme color palette definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePalette {
    /// Primary accent color
    pub accent: String,
    /// Secondary accent
    pub accent_secondary: String,
    /// Background primary
    pub bg_primary: String,
    /// Background secondary
    pub bg_secondary: String,
    /// Background tertiary
    pub bg_tertiary: String,
    /// Border color
    pub border: String,
    /// Text primary
    pub text_primary: String,
    /// Text secondary
    pub text_secondary: String,
    /// Text muted
    pub text_muted: String,
    /// Success/healthy indicator
    pub status_success: String,
    /// Warning indicator
    pub status_warning: String,
    /// Danger/error indicator
    pub status_danger: String,
    /// Glow/neon color
    pub neon: String,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            accent: "#00FF66".into(),
            accent_secondary: "#7B61FF".into(),
            bg_primary: "#0a0a12".into(),
            bg_secondary: "#12121e".into(),
            bg_tertiary: "#1a1a2e".into(),
            border: "#2a2a3e".into(),
            text_primary: "#e8e8ed".into(),
            text_secondary: "#a0a0b4".into(),
            text_muted: "#606078".into(),
            status_success: "#00FF66".into(),
            status_warning: "#FFCC00".into(),
            status_danger: "#FF3366".into(),
            neon: "#00FF66".into(),
        }
    }
}

/// Get the color palette for a theme preset
pub fn theme_palette(preset: &ThemePreset) -> ThemePalette {
    match preset {
        ThemePreset::ImpForgeDefault => ThemePalette::default(),
        ThemePreset::Cyberpunk => ThemePalette {
            accent: "#FF00FF".into(),
            accent_secondary: "#00FFFF".into(),
            bg_primary: "#0d0221".into(),
            bg_secondary: "#150530".into(),
            bg_tertiary: "#1a0a3e".into(),
            border: "#3d1a6e".into(),
            text_primary: "#f0e6ff".into(),
            text_secondary: "#b088d0".into(),
            text_muted: "#6a3d8f".into(),
            status_success: "#00FFFF".into(),
            status_warning: "#FFD700".into(),
            status_danger: "#FF1493".into(),
            neon: "#FF00FF".into(),
        },
        ThemePreset::Arctic => ThemePalette {
            accent: "#88DDFF".into(),
            accent_secondary: "#AAEEFF".into(),
            bg_primary: "#0a1520".into(),
            bg_secondary: "#101e2e".into(),
            bg_tertiary: "#162838".into(),
            border: "#2a4a60".into(),
            text_primary: "#e0f0ff".into(),
            text_secondary: "#8ab8d8".into(),
            text_muted: "#4a7898".into(),
            status_success: "#66EEBB".into(),
            status_warning: "#FFD488".into(),
            status_danger: "#FF7788".into(),
            neon: "#88DDFF".into(),
        },
        ThemePreset::Ember => ThemePalette {
            accent: "#FF6B35".into(),
            accent_secondary: "#FFB347".into(),
            bg_primary: "#1a0e08".into(),
            bg_secondary: "#241810".into(),
            bg_tertiary: "#2e2018".into(),
            border: "#4a3020".into(),
            text_primary: "#ffe8d8".into(),
            text_secondary: "#c09070".into(),
            text_muted: "#785840".into(),
            status_success: "#88CC44".into(),
            status_warning: "#FFB347".into(),
            status_danger: "#FF4444".into(),
            neon: "#FF6B35".into(),
        },
        ThemePreset::Matrix => ThemePalette {
            accent: "#00FF41".into(),
            accent_secondary: "#008F11".into(),
            bg_primary: "#000000".into(),
            bg_secondary: "#0a0a0a".into(),
            bg_tertiary: "#111111".into(),
            border: "#003B00".into(),
            text_primary: "#00FF41".into(),
            text_secondary: "#00AA2A".into(),
            text_muted: "#006615".into(),
            status_success: "#00FF41".into(),
            status_warning: "#88FF00".into(),
            status_danger: "#FF0000".into(),
            neon: "#00FF41".into(),
        },
        ThemePreset::Synthwave => ThemePalette {
            accent: "#FF71CE".into(),
            accent_secondary: "#B967FF".into(),
            bg_primary: "#1a1028".into(),
            bg_secondary: "#241838".into(),
            bg_tertiary: "#2e2048".into(),
            border: "#4a3068".into(),
            text_primary: "#ffe0f0".into(),
            text_secondary: "#c090c0".into(),
            text_muted: "#886088".into(),
            status_success: "#01CDFE".into(),
            status_warning: "#FFFB96".into(),
            status_danger: "#FF6B6B".into(),
            neon: "#FF71CE".into(),
        },
        ThemePreset::HighContrast => ThemePalette {
            accent: "#FFFF00".into(),
            accent_secondary: "#00FFFF".into(),
            bg_primary: "#000000".into(),
            bg_secondary: "#111111".into(),
            bg_tertiary: "#1a1a1a".into(),
            border: "#FFFFFF".into(),
            text_primary: "#FFFFFF".into(),
            text_secondary: "#EEEEEE".into(),
            text_muted: "#AAAAAA".into(),
            status_success: "#00FF00".into(),
            status_warning: "#FFFF00".into(),
            status_danger: "#FF0000".into(),
            neon: "#FFFF00".into(),
        },
        // All other presets use default as base (customers can customize)
        _ => ThemePalette::default(),
    }
}

// ============================================================================
// SQLITE PERSISTENCE
// ============================================================================

fn get_style_db() -> Result<rusqlite::Connection, String> {
    let data_dir = dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("impforge");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Dir create error: {e}"))?;
    let db_path = data_dir.join("styles.db");
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("DB open error: {e}"))?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;
        PRAGMA cache_size = -8000;
        PRAGMA temp_store = MEMORY;",
    ).map_err(|e| format!("PRAGMA error: {e}"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS widget_styles (
            widget_id TEXT NOT NULL,
            profile_id TEXT NOT NULL DEFAULT 'default',
            data TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (widget_id, profile_id)
        );
        CREATE TABLE IF NOT EXISTS style_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS graph_configs (
            widget_id TEXT NOT NULL,
            component_id TEXT NOT NULL,
            profile_id TEXT NOT NULL DEFAULT 'default',
            data TEXT NOT NULL,
            PRIMARY KEY (widget_id, component_id, profile_id)
        );",
    ).map_err(|e| format!("Schema error: {e}"))?;

    // Ensure default profile exists
    conn.execute(
        "INSERT OR IGNORE INTO style_profiles (id, name, description) VALUES ('default', 'Default', 'Default style profile')",
        [],
    ).map_err(|e| format!("Default profile error: {e}"))?;

    Ok(conn)
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Get all sub-component styles for a widget (merged: saved overrides on top of defaults)
#[tauri::command]
pub async fn style_get_widget(widget_id: String, profile_id: Option<String>) -> Result<WidgetStyleMap, String> {
    let profile = profile_id.unwrap_or_else(|| "default".into());
    let defaults = default_widget_styles(&widget_id);

    // Try to load saved overrides
    if let Ok(conn) = get_style_db() {
        if let Ok(data) = conn.query_row(
            "SELECT data FROM widget_styles WHERE widget_id = ?1 AND profile_id = ?2",
            [&widget_id, &profile],
            |row| row.get::<_, String>(0),
        ) {
            if let Ok(saved) = serde_json::from_str::<WidgetStyleMap>(&data) {
                return Ok(saved);
            }
        }
    }

    Ok(defaults)
}

/// Save sub-component styles for a widget
#[tauri::command]
pub async fn style_save_widget(styles: WidgetStyleMap, profile_id: Option<String>) -> Result<String, String> {
    let profile = profile_id.unwrap_or_else(|| "default".into());
    let conn = get_style_db()?;
    let data = serde_json::to_string(&styles).map_err(|e| format!("Serialize error: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO widget_styles (widget_id, profile_id, data, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
        [&styles.widget_id, &profile, &data],
    ).map_err(|e| format!("Save error: {e}"))?;

    Ok(format!("Styles saved for widget '{}'", styles.widget_id))
}

/// Reset widget styles to defaults
#[tauri::command]
pub async fn style_reset_widget(widget_id: String, profile_id: Option<String>) -> Result<WidgetStyleMap, String> {
    let profile = profile_id.unwrap_or_else(|| "default".into());
    if let Ok(conn) = get_style_db() {
        let _ = conn.execute(
            "DELETE FROM widget_styles WHERE widget_id = ?1 AND profile_id = ?2",
            [&widget_id, &profile],
        );
    }
    Ok(default_widget_styles(&widget_id))
}

/// Get all available widget style defaults (for the style editor catalog)
#[tauri::command]
pub async fn style_list_defaults() -> Result<Vec<WidgetStyleMap>, String> {
    let widget_ids = vec![
        "system-stats", "agent-pool", "quick-chat", "docker-overview",
        "github-feed", "browser-sessions", "network-waterfall",
        "model-status", "eval-pipeline", "news-ticker", "workflow-status",
        "console-output",
    ];
    Ok(widget_ids.into_iter().map(|id| default_widget_styles(id)).collect())
}

/// List available fonts
#[tauri::command]
pub async fn style_list_fonts() -> Result<Vec<FontEntry>, String> {
    Ok(available_fonts())
}

/// Save a graph configuration for a widget sub-component
#[tauri::command]
pub async fn style_save_graph(widget_id: String, component_id: String, graph: GraphStyle, profile_id: Option<String>) -> Result<String, String> {
    let profile = profile_id.unwrap_or_else(|| "default".into());
    let conn = get_style_db()?;
    let data = serde_json::to_string(&graph).map_err(|e| format!("Serialize error: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO graph_configs (widget_id, component_id, profile_id, data) VALUES (?1, ?2, ?3, ?4)",
        [&widget_id, &component_id, &profile, &data],
    ).map_err(|e| format!("Save error: {e}"))?;
    Ok("Graph config saved".into())
}

/// Get graph configuration for a widget sub-component
#[tauri::command]
pub async fn style_get_graph(widget_id: String, component_id: String, profile_id: Option<String>) -> Result<GraphStyle, String> {
    let profile = profile_id.unwrap_or_else(|| "default".into());
    if let Ok(conn) = get_style_db() {
        if let Ok(data) = conn.query_row(
            "SELECT data FROM graph_configs WHERE widget_id = ?1 AND component_id = ?2 AND profile_id = ?3",
            [&widget_id, &component_id, &profile],
            |row| row.get::<_, String>(0),
        ) {
            if let Ok(graph) = serde_json::from_str::<GraphStyle>(&data) {
                return Ok(graph);
            }
        }
    }
    Ok(GraphStyle::default())
}

/// List style profiles
#[tauri::command]
pub async fn style_list_profiles() -> Result<Vec<serde_json::Value>, String> {
    let conn = get_style_db()?;
    let mut stmt = conn.prepare("SELECT id, name, description, created_at, updated_at FROM style_profiles ORDER BY name")
        .map_err(|e| format!("Query error: {e}"))?;
    let profiles: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, Option<String>>(2)?,
            "created_at": row.get::<_, String>(3)?,
            "updated_at": row.get::<_, String>(4)?,
        }))
    }).map_err(|e| format!("Query error: {e}"))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(profiles)
}

/// Create a new style profile
#[tauri::command]
pub async fn style_create_profile(id: String, name: String, description: Option<String>) -> Result<String, String> {
    let conn = get_style_db()?;
    conn.execute(
        "INSERT INTO style_profiles (id, name, description) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, name, description],
    ).map_err(|e| format!("Create profile error: {e}"))?;
    Ok(format!("Profile '{name}' created"))
}

/// Delete a style profile (and all its widget styles)
#[tauri::command]
pub async fn style_delete_profile(profile_id: String) -> Result<String, String> {
    if profile_id == "default" {
        return Err("Cannot delete default profile".into());
    }
    let conn = get_style_db()?;
    conn.execute("DELETE FROM widget_styles WHERE profile_id = ?1", [&profile_id])
        .map_err(|e| format!("Delete styles error: {e}"))?;
    conn.execute("DELETE FROM graph_configs WHERE profile_id = ?1", [&profile_id])
        .map_err(|e| format!("Delete graphs error: {e}"))?;
    conn.execute("DELETE FROM style_profiles WHERE id = ?1", [&profile_id])
        .map_err(|e| format!("Delete profile error: {e}"))?;
    Ok(format!("Profile '{profile_id}' deleted"))
}

// ============================================================================
// THEME PRESET COMMANDS — Expose theme presets to frontend
// ============================================================================

/// Get the color palette for a given theme preset name
#[tauri::command]
pub async fn style_get_theme_palette(preset_name: String) -> Result<ThemePalette, String> {
    let preset = match preset_name.as_str() {
        "default" | "impforge" => ThemePreset::ImpForgeDefault,
        "cyberpunk" => ThemePreset::Cyberpunk,
        "arctic" => ThemePreset::Arctic,
        "ember" => ThemePreset::Ember,
        "imperial" => ThemePreset::Imperial,
        "matrix" => ThemePreset::Matrix,
        "corporate" => ThemePreset::Corporate,
        "synthwave" => ThemePreset::Synthwave,
        "forest" => ThemePreset::Forest,
        "daylight" => ThemePreset::Daylight,
        "deepsea" => ThemePreset::DeepSea,
        "crimson" => ThemePreset::Crimson,
        "candy" => ThemePreset::Candy,
        "monochrome" => ThemePreset::Monochrome,
        "solarflare" => ThemePreset::SolarFlare,
        "highcontrast" => ThemePreset::HighContrast,
        "minimal" => ThemePreset::Minimal,
        "retro8bit" => ThemePreset::Retro8Bit,
        "holographic" => ThemePreset::Holographic,
        other => ThemePreset::Custom(other.to_string()),
    };
    Ok(theme_palette(&preset))
}

/// List all available theme preset names
#[tauri::command]
pub async fn style_list_theme_presets() -> Result<Vec<String>, String> {
    Ok(vec![
        "default", "cyberpunk", "arctic", "ember", "imperial", "matrix",
        "corporate", "synthwave", "forest", "daylight", "deepsea", "crimson",
        "candy", "monochrome", "solarflare", "highcontrast", "minimal",
        "retro8bit", "holographic",
    ].into_iter().map(String::from).collect())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_style_default() {
        let ts = TextStyle::default();
        assert_eq!(ts.font_size, 13.0);
        assert_eq!(ts.font_weight, 400);
        assert_eq!(ts.color, "#e8e8ed");
        assert!(ts.visible);
        assert_eq!(ts.opacity, 1.0);
        assert_eq!(ts.offset, (0.0, 0.0));
    }

    #[test]
    fn test_bar_style_default() {
        let bs = BarStyle::default();
        assert_eq!(bs.color, "#00FF66");
        assert_eq!(bs.height, 16.0);
        assert_eq!(bs.color_thresholds.len(), 2);
        assert!(bs.animate_changes);
        assert!(bs.visible);
    }

    #[test]
    fn test_bar_color_thresholds_sorted() {
        let bs = BarStyle::default();
        // Lower thresholds should use warning/danger colors
        assert_eq!(bs.color_thresholds[0].color, "#FF3366"); // danger at 25%
        assert_eq!(bs.color_thresholds[1].color, "#FFCC00"); // warning at 50%
    }

    #[test]
    fn test_glow_style_default() {
        let gs = GlowStyle::default();
        assert_eq!(gs.glow_type, GlowType::None);
        assert_eq!(gs.color, "#00FF66");
        assert!(!gs.animated);
    }

    #[test]
    fn test_animation_config_default() {
        let ac = AnimationConfig::default();
        assert_eq!(ac.animation_type, AnimationType::None);
        assert!(ac.respect_reduced_motion); // WCAG compliance
    }

    #[test]
    fn test_background_style_default() {
        let bg = BackgroundStyle::default();
        assert_eq!(bg.bg_type, BackgroundType::Solid);
        assert_eq!(bg.opacity, 1.0);
        assert_eq!(bg.backdrop_blur, 0.0);
    }

    #[test]
    fn test_component_style_serialization_roundtrip() {
        let cs = ComponentStyle {
            id: "test.container".into(),
            label: "Test Container".into(),
            text: Some(TextStyle {
                font_size: 16.0,
                color: "#FF0000".into(),
                number_format: NumberFormat::Percent,
                ..Default::default()
            }),
            glow: GlowStyle {
                glow_type: GlowType::BoxGlow,
                color: "#00FF66".into(),
                intensity: 12.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&cs).unwrap();
        let parsed: ComponentStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test.container");
        assert_eq!(parsed.text.unwrap().font_size, 16.0);
        assert_eq!(parsed.glow.glow_type, GlowType::BoxGlow);
    }

    #[test]
    fn test_widget_style_map_serialization() {
        let wsm = default_widget_styles("system-stats");
        assert_eq!(wsm.widget_id, "system-stats");
        assert!(wsm.components.len() >= 8); // container + title + 3 bars + 3 labels + temp
        let json = serde_json::to_string(&wsm).unwrap();
        let parsed: WidgetStyleMap = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.widget_id, "system-stats");
        assert_eq!(parsed.components.len(), wsm.components.len());
    }

    #[test]
    fn test_all_widgets_have_default_styles() {
        let ids = vec![
            "system-stats", "agent-pool", "quick-chat", "docker-overview",
            "github-feed", "browser-sessions", "network-waterfall",
            "model-status", "eval-pipeline", "news-ticker",
        ];
        for id in ids {
            let styles = default_widget_styles(id);
            assert_eq!(styles.widget_id, id);
            assert!(!styles.components.is_empty(), "Widget '{id}' has no default styles");
            // Every widget must have a container
            assert!(
                styles.components.iter().any(|c| c.id.ends_with(".container")),
                "Widget '{id}' missing container component"
            );
        }
    }

    #[test]
    fn test_system_stats_has_all_bars() {
        let styles = default_widget_styles("system-stats");
        let bar_ids: Vec<_> = styles.components.iter()
            .filter(|c| c.bar.is_some())
            .map(|c| c.id.as_str())
            .collect();
        assert!(bar_ids.contains(&"system-stats.cpu-bar"));
        assert!(bar_ids.contains(&"system-stats.ram-bar"));
        assert!(bar_ids.contains(&"system-stats.gpu-bar"));
    }

    #[test]
    fn test_parent_child_hierarchy() {
        let styles = default_widget_styles("system-stats");
        for comp in &styles.components {
            if let Some(ref parent) = comp.parent_id {
                assert!(
                    styles.components.iter().any(|c| c.id == *parent),
                    "Component '{}' references non-existent parent '{}'", comp.id, parent
                );
            }
        }
    }

    #[test]
    fn test_number_format_variants() {
        let formats = vec![
            NumberFormat::Raw,
            NumberFormat::Abbreviated,
            NumberFormat::Percent,
            NumberFormat::CurrentMax,
            NumberFormat::CurrentMaxPercent,
            NumberFormat::Hidden,
            NumberFormat::Deficit,
            NumberFormat::Bytes,
            NumberFormat::Duration,
            NumberFormat::Scientific,
            NumberFormat::CompactWithUnit,
            NumberFormat::Locale,
        ];
        for fmt in formats {
            let json = serde_json::to_string(&fmt).unwrap();
            let parsed: NumberFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, fmt);
        }
    }

    #[test]
    fn test_font_registry() {
        let fonts = available_fonts();
        assert!(fonts.len() >= 5);
        assert!(fonts.iter().any(|f| f.name == "Inter"));
        assert!(fonts.iter().any(|f| f.name == "JetBrains Mono"));
        // At least some must be bundled
        assert!(fonts.iter().any(|f| f.bundled));
    }

    #[test]
    fn test_graph_style_default() {
        let gs = GraphStyle::default();
        assert_eq!(gs.graph_type, GraphType::Sparkline);
        assert!(gs.animate);
        assert!(gs.smooth);
        assert_eq!(gs.data_points, 30);
    }

    #[test]
    fn test_graph_style_serialization() {
        let gs = GraphStyle {
            graph_type: GraphType::Area,
            color: "#FF3366".into(),
            fill_opacity: 0.3,
            show_grid: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&gs).unwrap();
        let parsed: GraphStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.graph_type, GraphType::Area);
        assert_eq!(parsed.color, "#FF3366");
        assert!(parsed.show_grid);
    }

    #[test]
    fn test_generic_card_styles() {
        let styles = generic_card_styles("custom-widget", "My Custom");
        assert_eq!(styles.widget_id, "custom-widget");
        assert_eq!(styles.components.len(), 3); // container, title, content
        assert_eq!(styles.components[0].id, "custom-widget.container");
    }

    #[test]
    fn test_wcag_animation_compliance() {
        // All default animations must respect reduced motion
        let ids = vec!["system-stats", "agent-pool", "quick-chat", "model-status"];
        for id in ids {
            let styles = default_widget_styles(id);
            for comp in &styles.components {
                assert!(
                    comp.animation.respect_reduced_motion,
                    "Component '{}' doesn't respect prefers-reduced-motion", comp.id
                );
            }
        }
    }

    #[test]
    fn test_border_pattern_serialization() {
        let patterns = vec![
            BorderPattern::Solid,
            BorderPattern::Dashed,
            BorderPattern::Dotted,
            BorderPattern::Double,
            BorderPattern::None,
            BorderPattern::Ridge,
            BorderPattern::Groove,
            BorderPattern::Inset,
            BorderPattern::Outset,
            BorderPattern::NeonGlow,
            BorderPattern::GradientBorder,
            BorderPattern::MarchingAnts,
            BorderPattern::Corners,
            BorderPattern::Pill,
            BorderPattern::PixelBorder,
        ];
        for p in patterns {
            let json = serde_json::to_string(&p).unwrap();
            let parsed: BorderPattern = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, p);
        }
    }

    #[test]
    fn test_easing_function_serialization() {
        let easings = vec![
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::Spring,
            Easing::BounceOut,
            Easing::ElasticOut,
            Easing::BackOut,
            Easing::SmoothStep,
            Easing::ExpoIn,
            Easing::ExpoOut,
            Easing::CircOut,
            Easing::SineInOut,
            Easing::Steps,
            Easing::CustomBezier,
        ];
        for e in easings {
            let json = serde_json::to_string(&e).unwrap();
            let parsed: Easing = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, e);
        }
    }

    #[test]
    fn test_bar_texture_all_variants() {
        let textures = vec![
            BarTexture::Flat, BarTexture::Gradient, BarTexture::Striped,
            BarTexture::Glossy, BarTexture::Minimalist, BarTexture::Noise,
            BarTexture::Lined, BarTexture::Pixelated, BarTexture::Checkerboard,
            BarTexture::BrushedMetal, BarTexture::Diamond, BarTexture::Honeycomb,
            BarTexture::Circuit, BarTexture::Wave, BarTexture::Frosted,
            BarTexture::CarbonFiber, BarTexture::Scanline, BarTexture::NeonEdge,
            BarTexture::DualTone, BarTexture::Shimmer,
        ];
        for t in textures {
            let json = serde_json::to_string(&t).unwrap();
            let parsed: BarTexture = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, t);
        }
    }

    #[test]
    fn test_glow_type_all_variants() {
        let glows = vec![
            GlowType::None, GlowType::BoxGlow, GlowType::TextGlow,
            GlowType::InnerGlow, GlowType::DualGlow, GlowType::NeonMultiLayer,
            GlowType::AmbientGlow, GlowType::EdgeGlow, GlowType::PulsingRing,
            GlowType::FireGlow, GlowType::FrostGlow, GlowType::ElectricGlow,
            GlowType::HolographicGlow, GlowType::DropShadow, GlowType::NeonUnderline,
        ];
        for g in glows {
            let json = serde_json::to_string(&g).unwrap();
            let parsed: GlowType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, g);
        }
    }

    #[test]
    fn test_animation_type_all_variants() {
        let animations = vec![
            AnimationType::None, AnimationType::Fade, AnimationType::Scale,
            AnimationType::SlideIn, AnimationType::PulseOnChange, AnimationType::Flash,
            AnimationType::CountUp, AnimationType::Breathe, AnimationType::Bounce,
            AnimationType::Elastic, AnimationType::Flip, AnimationType::Typewriter,
            AnimationType::Shake, AnimationType::BlurIn, AnimationType::Glitch,
            AnimationType::MatrixRain, AnimationType::Ripple, AnimationType::Morph,
            AnimationType::StaggerChildren, AnimationType::Heartbeat,
        ];
        for a in animations {
            let json = serde_json::to_string(&a).unwrap();
            let parsed: AnimationType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, a);
        }
    }

    #[test]
    fn test_background_type_all_variants() {
        let bgs = vec![
            BackgroundType::Solid, BackgroundType::LinearGradient,
            BackgroundType::RadialGradient, BackgroundType::ConicGradient,
            BackgroundType::Pattern, BackgroundType::Transparent,
            BackgroundType::Glass, BackgroundType::MeshGradient,
            BackgroundType::AnimatedGradient, BackgroundType::DiagonalStripes,
            BackgroundType::DotGrid, BackgroundType::Crosshatch,
            BackgroundType::Hexagons, BackgroundType::CarbonFiber,
            BackgroundType::CircuitBoard, BackgroundType::Starfield,
            BackgroundType::Topographic, BackgroundType::NoiseGradient,
            BackgroundType::Waves, BackgroundType::Image,
        ];
        for bg in bgs {
            let json = serde_json::to_string(&bg).unwrap();
            let parsed: BackgroundType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, bg);
        }
    }

    #[test]
    fn test_graph_type_all_variants() {
        let graphs = vec![
            GraphType::Sparkline, GraphType::Area, GraphType::Line,
            GraphType::BarChart, GraphType::Gauge, GraphType::Donut,
            GraphType::StackedArea, GraphType::HorizontalBar,
            GraphType::GroupedBar, GraphType::Radar, GraphType::Heatmap,
            GraphType::TreeMap, GraphType::Scatter, GraphType::Waterfall,
            GraphType::Funnel, GraphType::Candlestick, GraphType::Sankey,
            GraphType::Bubble, GraphType::StepLine, GraphType::PolarArea,
        ];
        for g in graphs {
            let json = serde_json::to_string(&g).unwrap();
            let parsed: GraphType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, g);
        }
    }

    #[test]
    fn test_font_family_all_variants() {
        let families = vec![
            FontFamily::System, FontFamily::Mono, FontFamily::Display,
            FontFamily::Geometric, FontFamily::Rounded, FontFamily::Condensed,
            FontFamily::Handwriting, FontFamily::Pixel, FontFamily::Serif,
            FontFamily::SlabSerif, FontFamily::Futuristic, FontFamily::Gaming,
            FontFamily::TabularNums,
        ];
        for f in families {
            let json = serde_json::to_string(&f).unwrap();
            let parsed: FontFamily = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, f);
        }
    }

    #[test]
    fn test_text_outline_all_variants() {
        let outlines = vec![
            TextOutline::None, TextOutline::Thin, TextOutline::Medium,
            TextOutline::Thick, TextOutline::ColoredThin, TextOutline::DoubleOutline,
            TextOutline::ShadowOutline, TextOutline::Embossed,
        ];
        for o in outlines {
            let json = serde_json::to_string(&o).unwrap();
            let parsed: TextOutline = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, o);
        }
    }

    #[test]
    fn test_theme_preset_serialization() {
        let presets = vec![
            ThemePreset::ImpForgeDefault, ThemePreset::Cyberpunk,
            ThemePreset::Arctic, ThemePreset::Ember,
            ThemePreset::Imperial, ThemePreset::Matrix,
            ThemePreset::Corporate, ThemePreset::Synthwave,
            ThemePreset::Forest, ThemePreset::Daylight,
            ThemePreset::DeepSea, ThemePreset::Crimson,
            ThemePreset::Candy, ThemePreset::Monochrome,
            ThemePreset::SolarFlare, ThemePreset::HighContrast,
            ThemePreset::Minimal, ThemePreset::Retro8Bit,
            ThemePreset::Holographic,
            ThemePreset::Custom("my-theme".into()),
        ];
        for p in presets {
            let json = serde_json::to_string(&p).unwrap();
            let parsed: ThemePreset = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, p);
        }
    }

    #[test]
    fn test_theme_palette_colors() {
        let cyberpunk = theme_palette(&ThemePreset::Cyberpunk);
        assert_eq!(cyberpunk.accent, "#FF00FF");
        assert_eq!(cyberpunk.neon, "#FF00FF");

        let matrix = theme_palette(&ThemePreset::Matrix);
        assert_eq!(matrix.accent, "#00FF41");
        assert_eq!(matrix.bg_primary, "#000000");

        let hc = theme_palette(&ThemePreset::HighContrast);
        assert_eq!(hc.text_primary, "#FFFFFF");
        assert_eq!(hc.border, "#FFFFFF");
    }

    #[test]
    fn test_font_registry_comprehensive() {
        let fonts = available_fonts();
        assert!(fonts.len() >= 30, "Expected 30+ fonts, got {}", fonts.len());
        // Verify bundled fonts
        let bundled: Vec<_> = fonts.iter().filter(|f| f.bundled).collect();
        assert!(bundled.len() >= 3);
        // Verify categories
        let categories: std::collections::HashSet<_> = fonts.iter().map(|f| f.category.as_str()).collect();
        assert!(categories.contains("sans-serif"));
        assert!(categories.contains("monospace"));
        assert!(categories.contains("display"));
        assert!(categories.contains("serif"));
        assert!(categories.contains("handwriting"));
        // Verify variable fonts exist
        assert!(fonts.iter().any(|f| f.is_variable));
    }
}
