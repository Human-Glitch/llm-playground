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
}

impl GitHubClient {
    pub fn new(client: Client, token: String) -> Self {
        GitHubClient {
            client,
            token,
        }
    }

    /// Helper to build the API URL.
    fn api_url(&self, endpoint: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/{}",
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