use std::error::Error;
use reqwest::Client;
use serde_json::json;

pub struct OpenAIClient {
    http_client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIClient {
    pub fn new(http_client: Client, api_key: String, model: &str) -> Self {
        OpenAIClient {
            http_client,
            api_key,
            model: model.to_string(),
            base_url: "https://api.openai.com".to_string(),
        }
    }

    // Create a new client with a custom base URL (for testing)
    pub fn new_with_base_url(http_client: Client, api_key: String, model: &str, base_url: String) -> Self {
        OpenAIClient {
            http_client,
            api_key,
            model: model.to_string(),
            base_url,
        }
    }

    pub async fn format_release_notes(&self, unformatted: &str) -> Result<String, Box<dyn Error>> {
        let prompt = Self::build_release_notes_prompt(unformatted);
        let formatted_notes = self.request_chat_completion(&prompt).await?;
        Ok(formatted_notes)
    }

    async fn request_chat_completion(&self, prompt: &str) -> Result<String, Box<dyn Error>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.5,
        });

        let resp = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let json_response: serde_json::Value = resp.json().await?;
        if let Some(content) = json_response["choices"][0]["message"]["content"].as_str() {
            Ok(content.to_string())
        } else {
            Err("Failed to extract formatted release notes from OpenAI response.".into())
        }
    }

    fn build_release_notes_prompt(unformatted_notes: &str) -> String {
        format!(
            r#"TEMPLATE: https://onezelis.atlassian.net/browse/[Ticket ID]
                EXAMPLE: https://onezelis.atlassian.net/browse/PRDY-3441
                EXPECTED RESULT EXAMPLE: * [PDE-3441](https://onezelis.atlassian.net/browse/PDE-3441) Fixed an issue by @Human-Glitch in https://github.com/mdx-dev/CostEngine/pull/2329

               INSTRUCTIONS:
                - Please follow this template and deep link each item with the ticket url as shown in the example. 
                - Always print the answer in a way that Github Release Notes understands as raw text, so the formatting is preserved when editing Github Release Notes.
                - Create a heading for each Ticket ID Type: PD, PDE, PRDY
                - Assign each line item to one of these headings by the ticket id number ascending:\n\n{}
            "#,
            unformatted_notes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;
    use tokio::runtime::Runtime;

    #[test]
    fn given_valid_credentials_when_creating_client_then_succeeds() {
        let client = Client::new();
        let api_key = "test_api_key".to_string();
        let model = "gpt-4";
        let openai_client = OpenAIClient::new(client, api_key.clone(), model);
        
        // Verify the model was set correctly
        assert_eq!(openai_client.model, model);
        assert_eq!(openai_client.api_key, api_key);
    }

    #[test]
    fn given_unformatted_notes_when_building_prompt_then_returns_valid_prompt() {
        let unformatted_notes = "PDE-1234: Fixed bug\nPRDY-5678: Added feature";
        let prompt = OpenAIClient::build_release_notes_prompt(unformatted_notes);
        
        // Verify the prompt contains our unformatted notes
        assert!(prompt.contains(unformatted_notes));
        // Verify the prompt contains the template instructions
        assert!(prompt.contains("TEMPLATE: https://onezelis.atlassian.net/browse/[Ticket ID]"));
    }

    #[test]
    fn given_valid_input_when_formatting_release_notes_then_returns_formatted_notes() {
        let mut server = mockito::Server::new();
        
        // Create mock response that mimics OpenAI API - using simple content to avoid escape issues
        let mock_response = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677858242,
            "model": "gpt-4",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Formatted release notes"
                    },
                    "finish_reason": "stop",
                    "index": 0
                }
            ]
        }"#;
        
        let mock = server.mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let client = Client::new();
        let openai_client = OpenAIClient::new_with_base_url(
            client,
            "fake_api_key".to_string(),
            "gpt-4",
            server.url()
        );

        // Test format_release_notes method
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let notes = "PDE-1234: Fixed bug\nPRDY-5678: Added feature";
            openai_client.format_release_notes(notes).await.unwrap()
        });

        // Verify the result
        assert_eq!(result, "Formatted release notes");
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_error_response_when_formatting_release_notes_then_handles_error() {
        let mut server = mockito::Server::new();
        
        // Create a mock response with missing content field
        let mock_response = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677858242,
            "model": "gpt-4",
            "choices": [
                {
                    "message": {
                        "role": "assistant"
                        // Missing "content" field
                    },
                    "finish_reason": "stop",
                    "index": 0
                }
            ]
        }"#;
        
        let mock = server.mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let client = Client::new();
        let openai_client = OpenAIClient::new_with_base_url(
            client,
            "fake_api_key".to_string(),
            "gpt-4",
            server.url()
        );

        // Test format_release_notes method with invalid response
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let notes = "PDE-1234: Fixed bug";
            openai_client.format_release_notes(notes).await
        });
        
        // Verify that we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }
}
