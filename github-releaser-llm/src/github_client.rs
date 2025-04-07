use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::error::Error;

// Struct definitions needed by the GitHubClient
#[derive(Deserialize)]
pub struct GitHubRelease {
    pub id: u64,
    pub body: Option<String>,
}

#[derive(Deserialize)]
struct Commit {
    sha: String,
}

#[derive(Deserialize)]
struct TagObjectResponse {
    sha: String,
}

pub struct GitHubClient {
    client: Client,
    token: String,
    base_url: String,
}

impl GitHubClient {
    pub fn new(client: Client, token: String) -> Self {
        GitHubClient {
            client,
            token,
            base_url: "https://api.github.com".to_string(),
        }
    }

    // Create a new client with a custom base URL (for testing)
    pub fn new_with_base_url(client: Client, token: String, base_url: String) -> Self {
        GitHubClient {
            client,
            token,
            base_url,
        }
    }

    /// Helper to build the API URL.
    fn api_url(&self, endpoint: &str) -> String {
        format!(
            "{}/repos/{}/{}/{}",
            self.base_url,
            "Human-Glitch",
            "llm-playground",
            endpoint
        )
    }

    /// Get a release by tag.
    pub async fn get_release_by_tag(&self, tag: &str) -> Result<Option<GitHubRelease>, Box<dyn Error>> {
        let url = self.api_url(&format!("releases/tags/{}", tag));

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        
        match resp.status() {
            StatusCode::OK => {
                let release: GitHubRelease = resp.json().await?;
                Ok(Some(release))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(format!("Failed to get release: {}", resp.text().await?).into()),
        }
    }

    /// Delete a release by its ID.
    pub async fn delete_release(&self, release_id: u64) -> Result<(), Box<dyn Error>> {
        let url = self.api_url(&format!("releases/{}", release_id));

        let resp = self
            .client
            .delete(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if resp.status().is_success() {
            println!("Deleted GitHub release id: {}", release_id);
            Ok(())
        } else {
            Err(format!("Failed to delete release: {}", resp.text().await?).into())
        }
    }

    /// Delete a tag reference.
    pub async fn delete_tag(&self, tag: &str) -> Result<(), Box<dyn Error>> {
        let url = self.api_url(&format!("git/refs/tags/{}", tag));

        let resp = self
            .client
            .delete(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND {
            println!("Deleted tag reference: {}", tag);
            Ok(())
        } else {
            Err(format!("Failed to delete tag: {}", resp.text().await?).into())
        }
    }

    /// Get the latest commit SHA from a branch.
    pub async fn get_latest_commit_sha(&self, branch: &str) -> Result<String, Box<dyn Error>> {
        let url = self.api_url(&format!("commits/{}", branch));

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if resp.status().is_success() {
            let commit: Commit = resp.json().await?;
            Ok(commit.sha)
        } else {
            Err(format!(
                "Failed to get latest commit: {}",
                resp.text().await?
            )
            .into())
        }
    }

    /// Create an annotated tag object.
    pub async fn create_tag_object(
        &self,
        tag: &str,
        message: &str,
        object: &str,
    ) -> Result<String, Box<dyn Error>> {
        let url = self.api_url("git/tags");
        let body = json!({
            "tag": tag,
            "message": message,
            "object": object,
            "type": "commit"
        });

        let resp = self
            .client
            .post(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            let tag_resp: TagObjectResponse = resp.json().await?;
            Ok(tag_resp.sha)
        } else {
            Err(format!("Failed to create tag object: {}", resp.text().await?).into())
        }
    }

    /// Create a tag reference pointing to the tag object.
    pub async fn create_tag_ref(&self, tag: &str, sha: &str) -> Result<(), Box<dyn Error>> {
        let url = self.api_url("git/refs");
        let body = json!({
            "ref": format!("refs/tags/{}", tag),
            "sha": sha
        });

        let resp = self
            .client
            .post(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            println!("Created tag reference for: {}", tag);
            Ok(())
        } else {
            Err(format!("Failed to create tag ref: {}", resp.text().await?).into())
        }
    }

    /// Create a GitHub release using auto-generated release notes.
    pub async fn create_release(&self, tag: &str) -> Result<GitHubRelease, Box<dyn Error>> {
        let url = self.api_url("releases");

        let body = json!({
            "tag_name": tag,
            "target_commitish": "HEAD",
            "name": tag,
            "draft": false,
            "prerelease": false,
            "generate_release_notes": true
        });

        let resp = self
            .client
            .post(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            println!("Created GitHub release for tag: {}", tag);
            let release: GitHubRelease = resp.json().await?;
            Ok(release)
        } else {
            Err(format!("Failed to create release: {}", resp.text().await?).into())
        }
    }

    /// Update an existing GitHub release with new release notes.
    pub async fn update_release(&self, release_id: u64, notes: &str) -> Result<(), Box<dyn Error>> {
        let url = self.api_url(&format!("releases/{}", release_id));
        let body = json!({
            "body": notes
        });

        let resp = self
            .client
            .patch(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            println!("Updated release notes for release id: {}", release_id);
            Ok(())
        } else {
            Err(format!("Failed to update release: {}", resp.text().await?).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;
    use tokio::runtime::Runtime;

    #[test]
    fn test_github_client_creation() {
        let client = Client::new();
        let token = "test_token".to_string();
        let github_client = GitHubClient::new(client, token);
        
        // Test passes if client is created successfully without panicking
        assert_eq!(
            github_client.api_url("test_endpoint"),
            "https://api.github.com/repos/Human-Glitch/llm-playground/test_endpoint"
        );
    }

    #[test]
    fn test_get_release_by_tag_success() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/releases/tags/v1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 12345, "body": "Release notes"}"#)
            .create();

        // Create a client that will use our mock server instead of the real GitHub API
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let release = github_client.get_release_by_tag("v1.0.0").await.unwrap();
            release
        });
        
        // Verify the result
        assert!(result.is_some());
        let release = result.unwrap();
        assert_eq!(release.id, 12345);
        assert_eq!(release.body.unwrap(), "Release notes");
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_get_latest_commit_sha() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/commits/main")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"sha": "abc123def456"}"#)
            .create();

        // Create a client that will use our mock server instead of the real GitHub API
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let sha = github_client.get_latest_commit_sha("main").await.unwrap();
            sha
        });
        
        // Verify the result
        assert_eq!(result, "abc123def456");
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_create_tag_object() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/git/tags")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"sha": "tag_object_sha_123"}"#)
            .match_body(Matcher::Json(json!({
                "tag": "v1.0.0",
                "message": "Version 1.0.0",
                "object": "commit_sha_456",
                "type": "commit"
            })))
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let sha = github_client.create_tag_object("v1.0.0", "Version 1.0.0", "commit_sha_456").await.unwrap();
            sha
        });
        
        // Verify the result
        assert_eq!(result, "tag_object_sha_123");
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_create_tag_ref() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/git/refs")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{}"#)
            .match_body(Matcher::Json(json!({
                "ref": "refs/tags/v1.0.0",
                "sha": "tag_sha_123"
            })))
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.create_tag_ref("v1.0.0", "tag_sha_123").await
        });
        
        // Verify the result
        assert!(result.is_ok());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_create_release() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/releases")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 54321, "body": "Auto-generated release notes"}"#)
            .match_body(Matcher::Json(json!({
                "tag_name": "v1.0.0",
                "target_commitish": "HEAD",
                "name": "v1.0.0",
                "draft": false,
                "prerelease": false,
                "generate_release_notes": true
            })))
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let release = github_client.create_release("v1.0.0").await.unwrap();
            release
        });
        
        // Verify the result
        assert_eq!(result.id, 54321);
        assert_eq!(result.body.unwrap(), "Auto-generated release notes");
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_update_release() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("PATCH", "/repos/Human-Glitch/llm-playground/releases/12345")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{}"#)
            .match_body(Matcher::Json(json!({
                "body": "Updated release notes"
            })))
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.update_release(12345, "Updated release notes").await
        });
        
        // Verify the result
        assert!(result.is_ok());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_delete_release() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("DELETE", "/repos/Human-Glitch/llm-playground/releases/12345")
            .with_status(204)
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.delete_release(12345).await
        });
        
        // Verify the result
        assert!(result.is_ok());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn test_delete_tag() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("DELETE", "/repos/Human-Glitch/llm-playground/git/refs/tags/v1.0.0")
            .with_status(204)
            .create();

        // Create a client that will use our mock server
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.delete_tag("v1.0.0").await
        });
        
        // Verify the result
        assert!(result.is_ok());
        
        // Verify the mock was called
        mock.assert();
    }
}