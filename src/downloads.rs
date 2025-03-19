use std::{path::Path, time::Duration};

use anyhow::{Result, bail};
use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::Client;
use tokio::{fs::File, io::AsyncWriteExt};

/// A helper function for downloading files with retry attempts built in.
///
/// This module provides resilient file downloading capabilities with automatic retries,
/// exponential backoff, and streaming downloads to conserve memory. Files are downloaded
/// in chunks and written progressively to disk.
///
/// # Arguments
///
/// * `url` - A string slice containing the URL to download from
/// * `client` - A reqwest HTTP client instance to make the request with
/// * `target_dir` - A Path reference specifying where to save the downloaded file
///
/// # Returns
///
/// Returns a Result containing () on success, or an error if the download fails after retries
///
/// # Errors
///
/// This function will return an error if:
/// - The URL is invalid or cannot be parsed
/// - Network connectivity issues prevent downloading
/// - The target directory is not writable
/// - The downloaded file cannot be created or written
/// - The server returns a non-success status code (except 404 which is warned)
///
/// # Details
///
/// The function implements:
/// - Automatic retries with exponential backoff
/// - Streaming downloads to handle large files
/// - Progress tracking via log messages
/// - Filename extraction from URLs
/// - HTTP status code handling
/// - Error recovery and retry logic
pub async fn request_dataset(url: &str, client: Client, target_dir: &Path) -> Result<()> {
    debug!("Downloading dataset file from {}", url);

    // Download the file (retrying if necessary)
    let response = match download_with_retries(&client, url).await {
        Ok(r) => r,
        Err(e) => {
            bail!("The request encountered an error: {:?}. Skipping.", e);
        }
    };

    let filename = match url.split('/').last() {
        Some(name) => name,
        None => {
            bail!(
                "Failed to extract filename from URL, which may be corrupted: {}",
                url
            );
        }
    };

    if response.status().is_success() {
        let file_path = target_dir.join(filename);
        let mut file = File::create(file_path).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    file.write_all(&chunk).await?;
                }
                Err(e) => {
                    error!("Error while reading chunk from {}: {}", url, e);
                    return Err(e.into());
                }
            }
        }
        info!("Downloaded {}", filename);
    } else if response.status().as_u16() == 404 {
        warn!("File not found: {}", url);
    } else {
        error!(
            "Failed to download {}: HTTP {}",
            filename,
            response.status()
        );
        bail!(
            "Failed to download {}: HTTP {}",
            filename,
            response.status()
        );
    }

    Ok(())
}

async fn download_with_retries(client: &Client, url: &str) -> Result<reqwest::Response> {
    let mut attempt = 0;
    let max_attempts = 5;

    loop {
        attempt += 1;
        debug!("Performing attempt #{} to download from {}.", &attempt, url);
        match run_http_request(client, url).await {
            Ok(response) => {
                debug!("Successfully downloaded files for URL {}", url);
                return Ok(response);
            }
            Err(e) => {
                // early return an error if 5 attempts have been made unsuccessfully
                if attempt >= max_attempts {
                    error!(
                        "Failed to download files for URL {} after {} attempts:\n\n{}",
                        url, attempt, e
                    );
                    return Err(e);
                }
                // if there are remaining attempts, add an exponential backoff before proceeding to give the
                // server a break
                let delay = Duration::from_secs(2_u64.pow(attempt));
                warn!(
                    "Attempt {} failed for URL {}: {}. Retrying in {} seconds...",
                    attempt,
                    url,
                    e,
                    delay.as_secs()
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}

async fn run_http_request(client: &Client, url: &str) -> Result<reqwest::Response> {
    debug!("Downloading {}", url);

    let response = client.get(url).send().await?;

    if response.status().is_success() {
        debug!("Downloaded successful for {}", url);
        Ok(response)
    } else {
        bail!("Failed to download from URL {}: {}", url, response.status())
    }
}
