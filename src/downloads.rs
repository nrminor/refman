use std::sync::Arc;
use std::{path::Path, time::Duration};

use anyhow::{Result, bail};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use lychee_lib::{CacheStatus, Status};
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use url::Url;

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
pub async fn request_dataset(
    url: &str,
    client: Client,
    target_dir: &Path,
    mp: Arc<MultiProgress>,
) -> Result<()> {
    // Make sure the url is valid with lychee
    let valid_url = check_url(url).await?;

    // If it is, log out that it's valid
    debug!("Downloading dataset file from {:?}", valid_url);

    // Download the file (retrying if necessary), and access its size
    let response = match download_with_retries(&client, valid_url.as_str()).await {
        Ok(r) => {
            debug!("Successfully downloaded from {:?}", valid_url);
            r
        }
        Err(e) => {
            bail!("The request encountered an error: {:?}. Skipping.", e);
        }
    };
    let total_size = response.content_length().unwrap_or(0);

    // attempt to pull out the filename from the url
    let filename = uri_to_filename(&valid_url).await?;

    // if the response was successful, stream the file's bytes into the output file name
    if response.status().is_success() {
        let file_path = target_dir.join(filename);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create and configure the progress bar.
        let pb = mp.add(ProgressBar::new(total_size));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                )?
                .progress_chars("##-"),
        );
        pb.set_message(format!("Writing data into {}...", filename));

        let mut file = File::create(file_path).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    file.write_all(&chunk).await?;
                    pb.inc(chunk.len() as u64);
                }
                Err(e) => {
                    error!("Error while reading chunk from {}: {}", url, e);
                    return Err(e.into());
                }
            }
        }
        pb.set_message(format!("Writing data into {}...Done!", filename));
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

/// Tests and validates a given URL using `lychee`, ensuring it is accessible and valid.
///
/// This function performs validation checks on the provided URL string using the `lychee` crate's link
/// checker. It handles various response statuses including successful validation, redirects, timeouts,
/// cached responses, and various error conditions.
///
/// # Arguments
///
/// * `url` - A string slice containing the URL to validate
///
/// # Returns
///
/// Returns a Result containing the parsed `Url` if valid, or an error if validation fails
///
/// # Errors
///
/// This function will return an error if:
/// - The URL is invalid or malformed
/// - The server returns an error status
/// - The request times out
/// - The URL has been excluded by the host
/// - The URL format is unsupported
///
/// # Response Handling
///
/// The function handles several different validation outcomes:
/// - Successful validation (200 OK) - Returns parsed URL
/// - Redirects - Warns but proceeds with redirected URL
/// - Timeouts - Returns error with status code if available
/// - Cache hits - Uses cached response if valid
/// - Excluded URLs - Returns error for URLs blocked by host
/// - Unsupported URLs - Warns but attempts to proceed
///
/// This function is used internally by the download utilities to validate URLs before attempting
/// file downloads. It provides robust error handling and detailed logging to help diagnose any
/// connectivity or validation issues.
#[inline]
pub async fn check_url(url: &str) -> Result<Url> {
    debug!("Checking the requested URL '{url}' to make sure it's valid");
    let response = lychee_lib::check(url).await?;
    let response_body = response.body();
    match &response_body.status {
        Status::Ok(status_code) => {
            info!(
                "The URL {url} has been successfully checked with status code {}, and is thus valid and not broken.",
                status_code.as_str()
            );
            let parsed_url = Url::parse(response_body.uri.as_str())?;
            Ok(parsed_url)
        }
        Status::Error(error_kind) => {
            bail!(
                "An error was encountered when checking the provided URI, '{url}': {:?}",
                error_kind
            )
        }
        Status::Timeout(possible_code) => {
            if let Some(code) = possible_code {
                bail!(
                    "The request for the provided URI, '{url}', timed out with the status code {}.",
                    code.as_str()
                )
            } else {
                bail!("The request for the provided URI, '{url}', timed out without a status code.")
            }
        }
        Status::Redirected(status_code) => {
            warn!(
                "The provided URI resulted in a redirect to a different resource with status code {:?}. `refman` will proceed, though it may download a different file than is expected.",
                status_code.as_str()
            );
            let parsed_url = Url::parse(response_body.uri.as_str())?;
            Ok(parsed_url)
        }
        Status::UnknownStatusCode(status_code) => {
            bail!(
                "An unknown status code was received: {:?}",
                status_code.as_str()
            )
        }
        Status::Excluded => {
            bail!("The requested URL '{url}' has been excluded by the host.")
        }
        Status::Unsupported(error_kind) => {
            warn!(
                "The requested URL '{url}' is valid but unsupported by the validator. Proceed with downloading it at your own risk. Here's the validator error: {:?}",
                error_kind
            );
            let parsed_url = Url::parse(response_body.uri.as_str())?;
            Ok(parsed_url)
        }
        Status::Cached(cache_status) => {
            if let CacheStatus::Ok(_) = cache_status {
                info!("A cached response is being used instead of a fresh download.");
                let parsed_url = Url::parse(response_body.uri.as_str())?;
                Ok(parsed_url)
            } else {
                warn!(
                    "The requested url '{url}' appears to be valid and was cached, but the cache has become invalid."
                );
                let parsed_url = Url::parse(response_body.uri.as_str())?;
                Ok(parsed_url)
            }
        }
    }
}

/// Convert a URL into a filename by extracting the last segment of the path.
///
/// This function takes a URL and attempts to extract a filename from its path,
/// which is used for saving downloaded files. It looks for the last segment
/// of the URL path (after the final '/') and returns it if valid.
///
/// The function is used internally by the download utilities to determine
/// the output filename when saving downloaded files to disk. It ensures
/// that downloads have proper filenames based on their source URLs.
///
/// # Arguments
///
/// * `url` - A reference to a parsed URL from which to extract the filename
///
/// # Returns
///
/// Returns a Result containing:
/// - Ok(&str): The extracted filename if found
/// - Err: If no valid filename could be extracted from the URL
///
/// # Errors
///
/// This function will return an error if:
/// - The URL has no path segments
/// - The URL ends in a trailing slash (empty final segment)
/// - The URL path cannot be parsed into segments
///
/// # Example URLs
///
/// Valid URLs that would work:
/// - "https://example.com/files/data.csv" -> "data.csv"
/// - "https://example.com/downloads/dataset.zip" -> "dataset.zip"
///
/// Invalid URLs that would error:
/// - "https://example.com/" (no filename)
/// - "https://example.com/files/" (ends in slash)
/// - "https://example.com" (no path segments)
#[inline]
pub async fn uri_to_filename(url: &Url) -> Result<&str> {
    match url.path_segments().and_then(|segments| segments.last()) {
        Some(filename) if !filename.is_empty() => Ok(filename),
        _ => bail!(
            "Failed to extract filename from URL, which may be corrupted or may not end with the name of a file: {}",
            url
        ),
    }
}
