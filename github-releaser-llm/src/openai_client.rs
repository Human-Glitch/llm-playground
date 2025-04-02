use std::error::Error;
use reqwest::Client;
use serde_json::json;

pub struct OpenAIClient {
    http_client: Client,
    api_key: String,
    model: String,
}

impl OpenAIClient {
    pub fn new(http_client: Client, api_key: String, model: &str) -> Self {
        OpenAIClient {
            http_client,
            api_key,
            model: model.to_string(),
        }
    }

    pub async fn format_release_notes(&self, unformatted: &str) -> Result<String, Box<dyn Error>> {
        let prompt = Self::build_release_notes_prompt(unformatted);
        let formatted_notes = self.request_chat_completion(&prompt).await?;
        Ok(formatted_notes)
    }

    async fn request_chat_completion(&self, prompt: &str) -> Result<String, Box<dyn Error>> {
        let url = "https://api.openai.com/v1/chat/completions";
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.5,
        });

        let resp = self
            .http_client
            .post(url)
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
