use clap::Parser;
use reqwest::Client;
use std::env;
use std::error::Error;
use dotenv;

mod github_client;
mod openai_client;

use github_client::GitHubClient;
use openai_client::OpenAIClient;

#[derive(Parser)]
struct Cli {
    /// Release tag (e.g. v1.2.3)
    #[arg(short, long)]
    tag: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    dotenv::dotenv().ok();
    let github_token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN is missing.");
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is missing.");

    let args = Cli::parse();
    let tag = args.tag;
    let http_client = Client::new();

    let gh_client = GitHubClient::new(http_client.clone(), github_token);

    // Execute the release process
    process_release(&gh_client, &tag, http_client, openai_api_key).await?;

    println!("Release update process for '{}' completed successfully.", tag);
    Ok(())
}

/// Process the GitHub release including deleting existing release/tag,
/// creating new tag and release, and updating with formatted release notes.
async fn process_release(
    gh_client: &GitHubClient,
    tag: &str,
    http_client: Client,
    openai_api_key: String,
) -> Result<(), Box<dyn Error>> {
    println!("🚀 Starting release process for '{}'...", tag);
    
    // 1. Delete existing GitHub release (if exists).
    println!("Step 1: Checking for existing GitHub release...");
    if let Some(release) = gh_client.get_release_by_tag(tag).await? {
        println!("  Found existing release (ID: {}). Deleting...", release.id);
        gh_client.delete_release(release.id).await?;
        println!("  ✅ Existing release deleted successfully.");
    } else {
        println!("  ✅ No existing release found. Proceeding.");
    }

    // 2. Delete the Git tag (remote ref).
    println!("Step 2: Deleting existing Git tag (if any)...");
    match gh_client.delete_tag(tag).await {
        Ok(_) => println!("  ✅ Successfully deleted tag {}", tag),
        Err(e) => {
            println!("  ℹ️ Tag {} doesn't exist or was already deleted ({})", tag, e);
            // Not returning error as this is an acceptable condition
        }
    }

    // 3. Retrieve the latest commit SHA from the release branch.
    let branch = format!("release/{}", tag);
    println!("Step 3: Retrieving latest commit from branch {}...", branch);
    let commit_sha = match gh_client.get_latest_commit_sha(&branch).await {
        Ok(sha) => {
            println!("  ✅ Found commit: {}", sha);
            sha
        },
        Err(e) => {
            return Err(format!("Failed to get latest commit from branch '{}': {}", branch, e).into());
        }
    };

    // 4. Create an annotated tag object and then its reference.
    println!("Step 4: Creating annotated tag...");
    let tag_message = format!("Release {}", tag);
    let tag_object_sha = gh_client.create_tag_object(tag, &tag_message, &commit_sha).await?;
    gh_client.create_tag_ref(tag, &tag_object_sha).await?;
    println!("  ✅ Tag created and pushed successfully.");

    // 5. Create a new GitHub release with autogenerated release notes.
    println!("Step 5: Creating GitHub release...");
    let release = gh_client.create_release(tag).await?;
    println!("  ✅ Release created (ID: {}).", release.id);

    // 6. Retrieve the autogenerated release notes from the created release.
    println!("Step 6: Getting auto-generated release notes...");
    let auto_notes = match &release.body {
        Some(notes) if !notes.trim().is_empty() => {
            println!("  ✅ Auto-generated notes retrieved:\n {}).", notes);
            notes.clone()
        },
        _ => {
            return Err("No auto-generated release notes found or notes are empty.".into());
        }
    };
    
    // 7. Send the autogenerated notes to OpenAI for formatting.
    let openai_client = OpenAIClient::new(http_client, openai_api_key, "gpt-4o");

    let formatted_notes = openai_client.format_release_notes(&auto_notes).await?;
    println!("Formatted Release Notes:\n{}", formatted_notes);

    // 8. Update the GitHub release with the formatted release notes.
    gh_client.update_release(release.id, &formatted_notes).await?;

    Ok(())
}
