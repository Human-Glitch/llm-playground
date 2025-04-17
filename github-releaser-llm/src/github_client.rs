use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::error::Error;
use regex::Regex;

// Struct definitions needed by the GitHubClient
#[derive(Deserialize)]
pub struct GitHubRelease {
    pub id: u64,
    pub body: Option<String>,
    pub prerelease: Option<bool>,
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
        
        // Get the appropriate branch for this release
        let branch = self.get_release_branch_for_tag(tag).await?;
        
        let body = json!({
            "tag_name": tag,
            "target_commitish": branch,
            "name": tag,
            "draft": false,
            "prerelease": true,
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

    /// Check if a branch exists in the repository
    pub async fn branch_exists(&self, branch: &str) -> Result<bool, Box<dyn Error>> {
        let url = self.api_url(&format!("branches/{}", branch));

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "release_updater")
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        
        Ok(resp.status().is_success())
    }

    /// Parse a semantic version tag (e.g., v1.2.3) and increment the patch version
    pub fn increment_patch_version(&self, tag: &str) -> Result<String, Box<dyn Error>> {
        let re = Regex::new(r"^v(\d+)\.(\d+)\.(\d+)(.*)$")?;
        
        if let Some(caps) = re.captures(tag) {
            let major = caps.get(1).unwrap().as_str();
            let minor = caps.get(2).unwrap().as_str();
            let patch = caps.get(3).unwrap().as_str();
            let suffix = caps.get(4).map_or("", |m| m.as_str());
            
            let new_patch = patch.parse::<u32>().unwrap() + 1;
            Ok(format!("v{}.{}.{}{}", major, minor, new_patch, suffix))
        } else {
            Err(format!("Invalid semantic version tag format: {}", tag).into())
        }
    }

    /// Get the minor version part of a tag (e.g., v1.2.3 -> 1.2)
    pub fn get_minor_version(&self, tag: &str) -> Result<String, Box<dyn Error>> {
        let re = Regex::new(r"^v(\d+)\.(\d+)\.(\d+)(.*)$")?;
        
        if let Some(caps) = re.captures(tag) {
            let major = caps.get(1).unwrap().as_str();
            let minor = caps.get(2).unwrap().as_str();
            
            Ok(format!("{}.{}", major, minor))
        } else {
            Err(format!("Invalid semantic version tag format: {}", tag).into())
        }
    }
    
    /// Get the release branch name for a tag following the convention release/v{major}.{minor}.x
    pub fn get_release_branch_name(&self, tag: &str) -> Result<String, Box<dyn Error>> {
        let minor_version = self.get_minor_version(tag)?;
        Ok(format!("release/v{}.x", minor_version))
    }
    
    /// Check if a release exists for a given tag and is in prerelease state
    pub async fn is_prerelease(&self, tag: &str) -> Result<bool, Box<dyn Error>> {
        if let Some(release) = self.get_release_by_tag(tag).await? {
            return Ok(release.prerelease.unwrap_or(false));
        }
        
        Ok(false)
    }
    
    /// Check if conditions are met to increment the patch version:
    /// 1. Previous tag exists and is in prerelease state
    /// 2. The release branch for the minor version exists (using format release/v{major}.{minor}.x)
    pub async fn should_increment_patch(&self, tag: &str) -> Result<bool, Box<dyn Error>> {
        // Check if the current tag has a release that's in prerelease state
        let is_pre = self.is_prerelease(tag).await?;
        
        if !is_pre {
            return Ok(false);
        }
        
        // Get the release branch name following the convention release/v{major}.{minor}.x
        let branch_name = self.get_release_branch_name(tag)?;
        
        // Check if the branch exists
        let branch_exists = self.branch_exists(&branch_name).await?;
        
        Ok(is_pre && branch_exists)
    }

    /// Determine if a tag should be incremented, and if so, return the new tag
    pub async fn determine_tag_version(&self, requested_tag: &str) -> Result<String, Box<dyn Error>> {
        if self.should_increment_patch(requested_tag).await? {
            let new_tag = self.increment_patch_version(requested_tag)?;
            println!("ℹ️ The requested tag {} is in pre-release state with an existing minor version branch.", requested_tag);
            println!("ℹ️ Creating a new patch version: {}", new_tag);
            return Ok(new_tag);
        }
        
        Ok(requested_tag.to_string())
    }
    
    /// Get the release branch corresponding to a tag following the convention release/v{major}.{minor}.x
    pub async fn get_release_branch_for_tag(&self, tag: &str) -> Result<String, Box<dyn Error>> {
        // Get the branch name using our naming convention
        let branch_name = self.get_release_branch_name(tag)?;
        
        // Check if the branch exists
        if self.branch_exists(&branch_name).await? {
            return Ok(branch_name);
        }
        
        // If the branch doesn't exist, use the direct tag-based branch name for new releases
        let fallback_branch = format!("release/{}", tag);
        
        println!("⚠️  Branch {} not found. Creating a new branch {}.", branch_name, fallback_branch);
        Ok(fallback_branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;
    use tokio::runtime::Runtime;

    // Tests for semantic versioning operations
    #[test]
    fn given_semantic_version_tag_when_getting_minor_version_then_returns_correct_version() {
        let client = Client::new();
        let token = "test_token".to_string();
        let github_client = GitHubClient::new(client, token);
        
        let minor_version = github_client.get_minor_version("v1.2.3").unwrap();
        assert_eq!(minor_version, "1.2");
        
        let minor_version = github_client.get_minor_version("v2.0.1").unwrap();
        assert_eq!(minor_version, "2.0");
        
        // Test with pre-release suffix
        let minor_version = github_client.get_minor_version("v3.4.5-alpha").unwrap();
        assert_eq!(minor_version, "3.4");
    }

    #[test]
    fn given_semantic_version_tag_when_incrementing_patch_version_then_returns_incremented_version() {
        let client = Client::new();
        let token = "test_token".to_string();
        let github_client = GitHubClient::new(client, token);
        
        let incremented = github_client.increment_patch_version("v1.2.3").unwrap();
        assert_eq!(incremented, "v1.2.4");
        
        let incremented = github_client.increment_patch_version("v2.0.9").unwrap();
        assert_eq!(incremented, "v2.0.10");
        
        // Test with suffix
        let incremented = github_client.increment_patch_version("v3.4.5-beta").unwrap();
        assert_eq!(incremented, "v3.4.6-beta");
    }

    #[test]
    fn given_semantic_version_tag_when_getting_release_branch_name_then_returns_correct_branch_format() {
        let client = Client::new();
        let token = "test_token".to_string();
        let github_client = GitHubClient::new(client, token);
        
        let branch_name = github_client.get_release_branch_name("v1.2.3").unwrap();
        assert_eq!(branch_name, "release/v1.2.x");
        
        let branch_name = github_client.get_release_branch_name("v2.0.1").unwrap();
        assert_eq!(branch_name, "release/v2.0.x");
        
        // Test with pre-release suffix
        let branch_name = github_client.get_release_branch_name("v3.4.5-alpha").unwrap();
        assert_eq!(branch_name, "release/v3.4.x");
    }

    #[test]
    fn given_prerelease_tag_and_existing_branch_when_determining_tag_version_then_increments_patch_version() {
        let mut server = mockito::Server::new();
        
        // Mock for checking existing release
        let mock_release = server.mock("GET", "/repos/Human-Glitch/llm-playground/releases/tags/v1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 12345, "body": "Release notes", "prerelease": true}"#)
            .create();
        
        // Mock for checking branch existence
        let mock_branch = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/release/v1.0.x")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name": "release/v1.0.x"}"#)
            .create();
        
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let new_tag = github_client.determine_tag_version("v1.0.0").await.unwrap();
            new_tag
        });
        
        // Should increment because tag exists and is prerelease, and branch exists
        assert_eq!(result, "v1.0.1");
        
        // Verify the mocks were called
        mock_release.assert();
        mock_branch.assert();
    }

    // Tests for branch management
    #[test]
    fn given_tag_when_branch_exists_then_returns_minor_version_branch() {
        let mut server = mockito::Server::new();
        
        // Mock for checking existing branch
        let mock_branch = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/release/v1.0.x")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name": "release/v1.0.x"}"#)
            .create();
        
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let branch = github_client.get_release_branch_for_tag("v1.0.0").await.unwrap();
            branch
        });
        
        // Should return the minor version branch since it exists
        assert_eq!(result, "release/v1.0.x");
        
        // Verify the mock was called
        mock_branch.assert();
    }

    #[test]
    fn given_tag_when_branch_does_not_exist_then_returns_tag_specific_branch() {
        let mut server = mockito::Server::new();
        
        // Mock for checking non-existing branch
        let mock_branch = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/release/v1.0.x")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Not Found"}"#)
            .create();
        
        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let branch = github_client.get_release_branch_for_tag("v1.0.0").await.unwrap();
            branch
        });
        
        // Should return the fallback branch name since the minor version branch doesn't exist
        assert_eq!(result, "release/v1.0.0");
        
        // Verify the mock was called
        mock_branch.assert();
    }

    // Tests for client creation and initialization
    #[test]
    fn given_valid_credentials_when_creating_client_then_succeeds() {
        let client = Client::new();
        let token = "test_token".to_string();
        let github_client = GitHubClient::new(client, token);
        
        // Test passes if client is created successfully without panicking
        assert_eq!(
            github_client.api_url("test_endpoint"),
            "https://api.github.com/repos/Human-Glitch/llm-playground/test_endpoint"
        );
    }
    
    // Tests for release management
    #[test]
    fn given_valid_tag_when_getting_release_by_tag_then_returns_release() {
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
    fn given_nonexistent_tag_when_getting_release_by_tag_then_returns_none() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for a non-existent tag
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/releases/tags/v9.9.9")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Not Found"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let release = github_client.get_release_by_tag("v9.9.9").await.unwrap();
            release
        });
        
        // Verify we got None for a non-existent tag
        assert!(result.is_none());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_error_response_when_getting_release_by_tag_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/releases/tags/v1.0.0")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Internal Server Error"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.get_release_by_tag("v1.0.0").await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_main_branch_when_getting_latest_commit_then_returns_sha() {
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
    fn given_error_response_when_getting_latest_commit_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/commits/error-branch")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Internal Server Error"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.get_latest_commit_sha("error-branch").await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_valid_tag_info_when_creating_tag_object_then_returns_sha() {
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
    fn given_error_response_when_creating_tag_object_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/git/tags")
            .with_status(422)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Validation Failed"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.create_tag_object("invalid-tag", "Invalid Tag", "invalid-sha").await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_valid_tag_when_creating_tag_ref_then_succeeds() {
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
    fn given_error_response_when_creating_tag_ref_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/git/refs")
            .with_status(422)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Validation Failed"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.create_tag_ref("invalid-tag", "invalid-sha").await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_release_parameters_when_creating_release_then_returns_created_release() {
        let mut server = mockito::Server::new();
        
        // Add mock for the branch check
        let mock_branch = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/release/v1.0.x")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name": "release/v1.0.x"}"#)
            .create();
        
        // Set up the mock response for release creation
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/releases")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 54321, "body": "Auto-generated release notes"}"#)
            .match_body(Matcher::Json(json!({
                "tag_name": "v1.0.0",
                "target_commitish": "release/v1.0.x",
                "name": "v1.0.0",
                "draft": false,
                "prerelease": true,
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
        
        // Verify the mocks were called
        mock_branch.assert();
        mock.assert();
    }

    #[test]
    fn given_error_response_when_creating_release_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Add mock for the branch check
        let mock_branch = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/release/v1.0.x")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name": "release/v1.0.x"}"#)
            .create();
        
        // Set up the mock response for a failed release creation
        let mock = server.mock("POST", "/repos/Human-Glitch/llm-playground/releases")
            .with_status(422)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Validation Failed"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.create_release("v1.0.0").await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mocks were called
        mock_branch.assert();
        mock.assert();
    }

    #[test]
    fn given_prerelease_tag_when_checking_prerelease_status_then_returns_true() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/releases/tags/v1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 12345, "body": "Release notes", "prerelease": true}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.is_prerelease("v1.0.0").await.unwrap()
        });
        
        // Verify the result
        assert!(result);
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_branch_name_when_checking_existence_then_returns_true_if_exists() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an existing branch
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/main")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name": "main"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.branch_exists("main").await.unwrap()
        });
        
        // Verify the result
        assert!(result);
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_branch_name_when_checking_existence_then_returns_false_if_not_exists() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for a non-existent branch
        let mock = server.mock("GET", "/repos/Human-Glitch/llm-playground/branches/non-existent")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Not Found"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.branch_exists("non-existent").await.unwrap()
        });
        
        // Verify the result
        assert!(!result);
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_release_id_and_notes_when_updating_release_then_succeeds() {
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
    fn given_error_response_when_updating_release_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("PATCH", "/repos/Human-Glitch/llm-playground/releases/12345")
            .with_status(422)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Validation Failed"}"#)
            .create();

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
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_release_id_when_deleting_release_then_succeeds() {
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
    fn given_error_response_when_deleting_release_then_returns_error() {
        let mut server = mockito::Server::new();
        
        // Set up the mock response for an error
        let mock = server.mock("DELETE", "/repos/Human-Glitch/llm-playground/releases/99999")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "Not Found"}"#)
            .create();

        let client = Client::new();
        let github_client = GitHubClient::new_with_base_url(
            client, 
            "fake_token".to_string(),
            server.url()
        );
        
        // Test the method with our mock
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            github_client.delete_release(99999).await
        });
        
        // Verify we got an error
        assert!(result.is_err());
        
        // Verify the mock was called
        mock.assert();
    }

    #[test]
    fn given_tag_name_when_deleting_tag_then_succeeds() {
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