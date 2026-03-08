// ImpForge Task Type Classifier
// Ultra-fast keyword-based classification (<10ms, no embedding needed)

/// Task types that determine model routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    // Code-oriented tasks
    CodeGeneration,      // "write a function that..."
    CodeExplanation,     // "explain this code..."
    DockerfileGen,       // "create a Dockerfile for..."
    N8nWorkflowGen,      // "build an n8n workflow that..."

    // Text-oriented tasks
    ReadmeSummary,       // README summarization
    GeneralChat,         // Free conversation
    TechQuestion,        // Technical questions
    ResearchDigest,      // arXiv paper summarization

    // Agentic tasks
    WebAutomation,       // Playwright MCP tasks
    MultiStepReasoning,  // Complex multi-step tasks
    ImageGeneration,     // Image creation

    // Local-only tasks
    SimpleClassification, // Short classification tasks
}

impl TaskType {
    /// Human-readable description for UI
    pub fn description(&self) -> &'static str {
        match self {
            Self::CodeGeneration => "Code Generation",
            Self::CodeExplanation => "Code Explanation",
            Self::DockerfileGen => "Dockerfile Generation",
            Self::N8nWorkflowGen => "n8n Workflow Generation",
            Self::ReadmeSummary => "README Summary",
            Self::GeneralChat => "General Chat",
            Self::TechQuestion => "Technical Question",
            Self::ResearchDigest => "Research Digest",
            Self::WebAutomation => "Web Automation",
            Self::MultiStepReasoning => "Multi-Step Reasoning",
            Self::ImageGeneration => "Image Generation",
            Self::SimpleClassification => "Simple Classification",
        }
    }

    /// Estimated token cost tier
    pub fn cost_tier(&self) -> &'static str {
        match self {
            Self::ReadmeSummary | Self::SimpleClassification => "FREE (Local)",
            Self::CodeGeneration | Self::DockerfileGen | Self::N8nWorkflowGen => "FREE (Devstral)",
            Self::GeneralChat | Self::TechQuestion | Self::ResearchDigest => "FREE (Llama 4)",
            Self::MultiStepReasoning => "FREE (Qwen3-30B)",
            Self::WebAutomation | Self::CodeExplanation => "FREE (Devstral)",
            Self::ImageGeneration => "FREE (FLUX) / Local SD",
        }
    }
}

/// Ultra-fast keyword-based classification
/// Returns TaskType in <10ms without any LLM or embedding call
pub fn classify_fast(prompt: &str) -> TaskType {
    let lower = prompt.to_lowercase();

    // Priority 1: Explicit task markers
    if is_dockerfile_task(&lower) {
        return TaskType::DockerfileGen;
    }
    if is_n8n_task(&lower) {
        return TaskType::N8nWorkflowGen;
    }
    if is_web_automation_task(&lower) {
        return TaskType::WebAutomation;
    }
    if is_image_task(&lower) {
        return TaskType::ImageGeneration;
    }

    // Priority 2: Code indicators
    if is_code_task(&lower) {
        if is_explanation_request(&lower) {
            return TaskType::CodeExplanation;
        }
        return TaskType::CodeGeneration;
    }

    // Priority 3: Research/Academic
    if is_research_task(&lower) {
        return TaskType::ResearchDigest;
    }

    // Priority 4: README
    if is_readme_task(&lower) {
        return TaskType::ReadmeSummary;
    }

    // Priority 5: Multi-step reasoning (complex questions)
    if is_multistep_task(&lower) {
        return TaskType::MultiStepReasoning;
    }

    // Priority 6: Technical questions
    if is_tech_question(&lower) {
        return TaskType::TechQuestion;
    }

    // Priority 7: Single-word prompts → simple classification (local model)
    if lower.split_whitespace().count() == 1 && !lower.ends_with('?') {
        return TaskType::SimpleClassification;
    }

    // Default: General chat
    TaskType::GeneralChat
}

// --- Helper functions for classification ---

fn is_dockerfile_task(text: &str) -> bool {
    text.contains("dockerfile")
        || text.contains("docker-compose")
        || text.contains("docker compose")
        || (text.contains("container") && text.contains("create"))
        || (text.contains("docker") && (text.contains("build") || text.contains("erstell")))
}

fn is_n8n_task(text: &str) -> bool {
    text.contains("n8n")
        || text.contains("workflow")
        || (text.contains("automation") && !text.contains("browser"))
        || text.contains("trigger")
        || text.contains("webhook")
        || text.contains("automate")
        || text.contains("automatisier")
}

fn is_web_automation_task(text: &str) -> bool {
    text.contains("navigate to")
        || text.contains("click on")
        || text.contains("fill out")
        || text.contains("fill in")
        || text.contains("browser automation")
        || text.contains("playwright")
        || text.contains("scrape")
        || text.contains("extract from website")
}

fn is_image_task(text: &str) -> bool {
    text.contains("generate image")
        || text.contains("create image")
        || text.contains("generate a picture")
        || text.contains("draw")
        || text.contains("bild erstellen")
        || text.contains("bildgenerierung")
        || (text.contains("image") && text.contains("of"))
}

fn is_code_task(text: &str) -> bool {
    let code_markers = [
        "function", "class", "impl ", "fn ", "def ", "const ", "let ", "var ",
        "async", "await", "struct ", "enum ", "interface ", "type ", "pub fn",
        "public ", "private ", "protected ", "return ", "import ", "export ",
        "code", "schreib", "write", "implement", "create a", "build a",
        "funktion", "methode", "algorithm",
    ];

    let code_score: usize = code_markers.iter()
        .filter(|&&marker| text.contains(marker))
        .count();

    code_score >= 2 || (text.contains("code") && code_score >= 1)
}

fn is_explanation_request(text: &str) -> bool {
    text.contains("explain")
        || text.contains("erklär")
        || text.contains("what does")
        || text.contains("how does")
        || text.contains("was macht")
        || text.contains("wie funktioniert")
        || text.contains("understand")
        || text.contains("versteh")
}

fn is_research_task(text: &str) -> bool {
    text.contains("paper")
        || text.contains("arxiv")
        || text.contains("research")
        || text.contains("studie")
        || text.contains("forschung")
        || text.contains("academic")
        || text.contains("wissenschaft")
        || text.contains("publication")
}

fn is_readme_task(text: &str) -> bool {
    text.contains("readme")
        || text.contains("summarize this repo")
        || text.contains("zusammenfass")
        || (text.contains("summary") && text.contains("project"))
}

fn is_multistep_task(text: &str) -> bool {
    text.contains("step by step")
        || text.contains("schritt für schritt")
        || text.contains("analyze and then")
        || text.contains("first") && text.contains("then")
        || text.contains("plan")
        || text.contains("strategy")
        || text.contains("design a system")
        || text.contains("architecture")
}

fn is_tech_question(text: &str) -> bool {
    text.contains("how to")
        || text.contains("wie kann ich")
        || text.contains("what is")
        || text.contains("was ist")
        || text.contains("difference between")
        || text.contains("unterschied zwischen")
        || text.contains("best practice")
        || text.contains("why does")
        || text.contains("warum")
        || text.ends_with('?')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dockerfile_classification() {
        assert_eq!(classify_fast("Create a Dockerfile for Python"), TaskType::DockerfileGen);
        assert_eq!(classify_fast("Build a docker-compose for my app"), TaskType::DockerfileGen);
    }

    #[test]
    fn test_n8n_classification() {
        assert_eq!(classify_fast("Create an n8n workflow"), TaskType::N8nWorkflowGen);
        assert_eq!(classify_fast("Automate email notifications"), TaskType::N8nWorkflowGen);
    }

    #[test]
    fn test_code_classification() {
        assert_eq!(classify_fast("Write a function that sorts an array"), TaskType::CodeGeneration);
        assert_eq!(classify_fast("Explain this async function"), TaskType::CodeExplanation);
    }

    #[test]
    fn test_general_chat() {
        assert_eq!(classify_fast("Hello there"), TaskType::GeneralChat);
        assert_eq!(classify_fast("Tell me a joke"), TaskType::GeneralChat);
        assert_eq!(classify_fast("Good morning"), TaskType::GeneralChat);
    }

    #[test]
    fn test_german_prompts() {
        assert_eq!(classify_fast("Schreib eine Funktion die sortiert"), TaskType::CodeGeneration);
        assert_eq!(classify_fast("Erkläre mir diesen Code"), TaskType::CodeExplanation);
        assert_eq!(classify_fast("Erstelle ein Dockerfile"), TaskType::DockerfileGen);
    }
}
