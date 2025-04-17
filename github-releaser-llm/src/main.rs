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

/// Process the GitHub release including checking for existing pre-releases,
/// incrementing the version if needed, and creating or updating releases.
async fn process_release(
    gh_client: &GitHubClient,
    requested_tag: &str,
    http_client: Client,
    openai_api_key: String,
) -> Result<(), Box<dyn Error>> {
    // Display the branch naming format for improved logging
    println!("🚀 Starting release process for '{}' using branch format release/v{}.{}.x...", 
        requested_tag,
        // Use placeholder values since we're just showing the format
        "major", "minor"
    );
    
    // Determine if we need to increment the version based on criteria
    let tag = gh_client.determine_tag_version(requested_tag).await?;
    
    // If the tag is different, we're creating a new incremented version
    let is_incremented_version = tag != requested_tag;
    
    if is_incremented_version {
        println!("⬆️ Using incremented version {} instead of {}", tag, requested_tag);
    }
    
    // 1. Check for existing GitHub release for the new tag.
    println!("Step 1: Checking for existing GitHub release...");
    if let Some(release) = gh_client.get_release_by_tag(&tag).await? {
        if is_incremented_version {
            // For incremented versions, update the existing release instead of deleting it
            println!("  Found existing release for incremented version (ID: {}). Will update instead of recreate.", release.id);
        } else {
            // Only delete if not an incremented version, preserving immutability of existing releases
            println!("  Found existing release (ID: {}). Deleting...", release.id);
            gh_client.delete_release(release.id).await?;
            println!("  ✅ Existing release deleted successfully.");
        }
    } else {
        println!("  ✅ No existing release found. Proceeding with creation.");
    }

    // 2. For non-incremented versions, we might need to delete the tag
    if !is_incremented_version {
        println!("Step 2: Checking existing Git tag...");
        match gh_client.delete_tag(&tag).await {
            Ok(_) => println!("  ✅ Successfully deleted tag {}", tag),
            Err(e) => {
                println!("  ℹ️ Tag {} doesn't exist or was already deleted ({})", tag, e);
                // Not returning error as this is an acceptable condition
            }
        }
    } else {
        println!("Step 2: Skipping tag deletion for incremented version to maintain immutability.");
    }

    // Determine which branch to use for the release
    let branch = gh_client.get_release_branch_for_tag(&tag).await?;
    println!("Step 3: Using release branch: {}", branch);
    
    // 3. Retrieve the latest commit SHA from the release branch.
    println!("Step 4: Retrieving latest commit from branch {}...", branch);
    let commit_sha = match gh_client.get_latest_commit_sha(&branch).await {
        Ok(sha) => {
            println!("  ✅ Found commit: {}", sha);
            sha
        },
        Err(e) => {
            return Err(format!("Failed to get latest commit from branch '{}': {}", branch, e).into());
        }
    };

    // 4. Create an annotated tag object and then its reference if it doesn't exist
    let existing_release = gh_client.get_release_by_tag(&tag).await?;
    
    if existing_release.is_none() || !is_incremented_version {
        println!("Step 5: Creating annotated tag...");
        let tag_message = format!("Release {}", tag);
        let tag_object_sha = gh_client.create_tag_object(&tag, &tag_message, &commit_sha).await?;
        gh_client.create_tag_ref(&tag, &tag_object_sha).await?;
        println!("  ✅ Tag created and pushed successfully.");
    } else {
        println!("Step 5: Skipping tag creation as it already exists for incremented version.");
    }

    // 5. Create or update GitHub release
    let release = if let Some(existing) = existing_release {
        println!("Step 6: Using existing GitHub release...");
        existing
    } else {
        println!("Step 6: Creating new GitHub release...");
        gh_client.create_release(&tag).await?
    };
    
    println!("  ✅ Release ready (ID: {}).", release.id);

    // 6. Retrieve the release notes
    println!("Step 7: Getting release notes...");
    let auto_notes = match &release.body {
        Some(notes) if !notes.trim().is_empty() => {
            println!("  ✅ Release notes retrieved.");
            notes.clone()
        },
        _ => {
            return Err("No release notes found or notes are empty.".into());
        }
    };
    
    // 7. Send the notes to OpenAI for formatting.
    let openai_client = OpenAIClient::new(http_client, openai_api_key, "gpt-4o");

    let formatted_notes = openai_client.format_release_notes(&auto_notes).await?;
    println!("Formatted Release Notes:\n{}", formatted_notes);

    // 8. Update the GitHub release with the formatted release notes.
    gh_client.update_release(release.id, &formatted_notes).await?;
    println!("  ✅ Release notes updated successfully.");

    Ok(())
}
