use crate::{
    ai_utils::{ChatMessage, OpenAIChatCompletionResponse, OpenAIChatRequest, clean_ai_output},
    config::AppConfig,
    errors::AIError,
};

/// Helper function to execute the AI request and process the response
async fn execute_ai_request(
    config: &AppConfig,
    messages: Vec<ChatMessage>,
) -> Result<String, AIError> {
    let request_payload = OpenAIChatRequest {
        model: config.ai.model_name.clone(),
        messages,
        temperature: Some(config.ai.temperature),
        stream: false,
    };

    if let Ok(json_string) = serde_json::to_string_pretty(&request_payload) {
        tracing::debug!(
            "Sending JSON payload to AI for explanation:\n{}",
            json_string
        );
    } else {
        tracing::warn!("Failed to serialize AI request payload for debugging.");
    }

    let client = reqwest::Client::new();
    let mut request_builder = client.post(&config.ai.api_url);

    // Add authorization header if api_key is present
    if let Some(api_key) = &config.ai.api_key {
        if !api_key.is_empty() {
            tracing::debug!("Using API key for AI explanation request.");
            request_builder = request_builder.bearer_auth(api_key);
        }
    }

    let openai_response = request_builder
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI explanation request failed during send: {}", e);
            // This error could be a network issue, DNS resolution failure, etc.
            // AIError::RequestFailed is a general error for reqwest issues.
            // AIError::ExplainerNetworkError could be used if a more specific categorization is needed
            // and can be reliably determined from `e`.
            AIError::RequestFailed(e)
        })?;

    if !openai_response.status().is_success() {
        let status_code = openai_response.status();
        let body = openai_response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body from AI response".to_string());
        tracing::error!(
            "AI explainer API request failed with status: {}: {}",
            status_code,
            body
        );
        return Err(AIError::ApiResponseError(status_code, body));
    }

    // Successfully received a response, now parse it.
    match openai_response.json::<OpenAIChatCompletionResponse>().await {
        Ok(response_data) => {
            if let Some(choice) = response_data.choices.get(0) {
                let original_content = &choice.message.content;
                if original_content.trim().is_empty() {
                    tracing::warn!("AI explainer returned an empty message content.");
                    Err(AIError::EmptyMessage)
                } else {
                    let cleaned_content = clean_ai_output(original_content);
                    tracing::debug!(
                        "Cleaned AI explanation received: \"{}\"",
                        cleaned_content.chars().take(100).collect::<String>()
                    ); // Log snippet
                    Ok(cleaned_content)
                }
            } else {
                tracing::warn!("No choices found in AI explainer response.");
                Err(AIError::NoChoiceInResponse)
            }
        }
        Err(e) => {
            tracing::error!("Failed to parse JSON response from AI explainer: {}", e);
            // This error occurs if the response body is not valid JSON matching OpenAIChatCompletionResponse
            Err(AIError::ResponseParseFailed(e))
        }
    }
}

/// Takes the raw output from a Git command (typically its help text)
pub async fn explain_git_command_output(
    config: &AppConfig,
    command_output: &str,
) -> Result<String, AIError> {
    if command_output.trim().is_empty() {
        // This is not an error, but a valid case where there's nothing to explain
        return Ok("The command produced no output for the AI to explain. \
            It might be a command that doesn't print to stdout/stderr on success, \
            or it requires specific conditions to produce output."
            .to_string());
    }

    tracing::debug!(
        "Requesting AI explaination for command output (first 200 chars):\n---\n{}\n---",
        command_output.chars().take(200).collect::<String>()
    );

    let system_prompt_content = config
        .prompts
        .get("explanation")
        .cloned()
        .unwrap_or_else(|| {
            tracing::warn!("Explanation prompt not found in config, using empty string");
            "".to_string()
        });

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt_content, // Use the prompt from config
        },
        ChatMessage {
            role: "user".to_string(),
            content: command_output.to_string(), // Send the full output
        },
    ];

    match execute_ai_request(config, messages).await {
        Ok(ai_explanation) => {
            let formatted_output = format!(
                "## Original Output\n\n```text\n{}\n```\n\n## AI Explanation\n\n{}",
                command_output, ai_explanation
            );
            Ok(formatted_output)
        }
        Err(e) => Err(e),
    }
}

/// Takes a Git command (as a sequence of its parts/arguments)
/// and returns an AI-generated explanation of waht that comand does.
pub async fn explain_git_command(
    config: &AppConfig,
    command_parts: &[String],
) -> Result<String, AIError> {
    if command_parts.is_empty() {
        // This is not an error from AI's perspective but an invalid input to this function.
        return Ok("No command parts provided for the AI to explain".to_string());
    }

    let command_to_explain = format!("git {}", command_parts.join(" "));
    tracing::debug!(
        "Requesting AI explanation for command: {}",
        command_to_explain
    );

    let user_message_content = command_to_explain;

    let system_prompt_content = config
        .prompts
        .get("explanation")
        .cloned()
        .unwrap_or_else(|| {
            tracing::warn!("Explanation prompt not found in config, using empty string");
            "".to_string()
        });

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt_content, // Use the prompt from config
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_message_content,
        },
    ];

    execute_ai_request(config, messages).await
}
