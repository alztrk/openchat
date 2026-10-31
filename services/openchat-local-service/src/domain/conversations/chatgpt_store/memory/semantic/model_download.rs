//! Semantic model file verification and bounded download helpers.

use super::*;

pub(super) async fn verify_file(
    path: PathBuf,
    asset: ModelFile,
) -> Result<bool, SemanticMemoryError> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    };
    if file
        .metadata()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?
        .len()
        != asset.size
    {
        return Ok(false);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file
            .read(&mut buffer)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hasher.finalize()) == asset.sha256)
}

pub(super) async fn download_file(
    destination: &Path,
    asset: ModelFile,
    completed_bytes: u64,
    total_bytes: u64,
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), SemanticMemoryError> {
    let url = format!(
        "{MODEL_DOWNLOAD_BASE_URL}/{MODEL_REPOSITORY}/resolve/{MODEL_REVISION}/{}?download=true",
        asset.path,
    );
    download_file_from_url(
        &url,
        destination,
        asset,
        completed_bytes,
        total_bytes,
        progress,
        cancellation,
    )
    .await
}

pub(super) async fn download_file_from_url(
    url: &str,
    destination: &Path,
    asset: ModelFile,
    completed_bytes: u64,
    total_bytes: u64,
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), SemanticMemoryError> {
    if *cancellation.borrow() {
        return Err(SemanticMemoryError::Cancelled);
    }

    let temporary = destination.with_extension("partial");
    let mut resume_offset = match tokio::fs::metadata(&temporary).await {
        Ok(metadata) if metadata.len() <= asset.size => metadata.len(),
        Ok(_) => {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            0
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    };
    if resume_offset == asset.size {
        if verify_file(temporary.clone(), asset).await? {
            if destination.exists() {
                fs::remove_file(destination)?;
            }
            tokio::fs::rename(&temporary, destination)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            report_progress(
                progress,
                "downloading",
                completed_bytes.saturating_add(asset.size),
                total_bytes,
                Some(asset.local_name),
            )?;
            return Ok(());
        }
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        resume_offset = 0;
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .redirect(crate::domain::hugging_face::hugging_face_redirect_policy())
        .build()
        .map_err(|error| SemanticMemoryError::Http(error.to_string()))?;
    let mut request = client.get(url);
    if resume_offset > 0 {
        request = request.header(RANGE, format!("bytes={resume_offset}-"));
    }
    let response = request
        .send()
        .await
        .map_err(|error| SemanticMemoryError::Http(error.to_string()))?;
    let (response_offset, response_length) = match response.status() {
        StatusCode::OK => (0, asset.size),
        StatusCode::PARTIAL_CONTENT => {
            let content_range = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| parse_content_range(value, resume_offset, asset.size))
                .ok_or(SemanticMemoryError::Integrity)?;
            (resume_offset, content_range)
        }
        status => {
            return Err(SemanticMemoryError::Http(format!(
                "model host returned status {status}"
            )));
        }
    };
    if response
        .content_length()
        .is_some_and(|length| length != response_length)
    {
        return Err(SemanticMemoryError::Integrity);
    }

    let mut hasher = Sha256::new();
    if response_offset > 0 {
        let mut prefix = tokio::fs::File::open(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        let mut hashed_bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let length = prefix
                .read(&mut buffer)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            if length == 0 {
                break;
            }
            hashed_bytes = hashed_bytes.saturating_add(length as u64);
            hasher.update(&buffer[..length]);
        }
        if hashed_bytes != response_offset {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            return Err(SemanticMemoryError::Integrity);
        }
    }

    let mut output_options = tokio::fs::OpenOptions::new();
    output_options.create(true).write(true);
    if response_offset > 0 {
        output_options.append(true);
    } else {
        output_options.truncate(true);
    }
    let mut output = output_options
        .open(&temporary)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    report_progress(
        progress,
        "downloading",
        completed_bytes.saturating_add(response_offset),
        total_bytes,
        Some(asset.local_name),
    )?;
    let mut stream = response.bytes_stream();
    let mut received_bytes = 0_u64;
    let mut bytes_since_sync = 0_u64;
    let mut last_progress_at = Instant::now() - PROGRESS_MIN_INTERVAL;
    loop {
        if *cancellation.borrow() {
            persist_partial_download(&mut output).await?;
            return Err(SemanticMemoryError::Cancelled);
        }
        let next = tokio::select! {
            item = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, stream.next()) => item,
            changed = cancellation.changed() => {
                if changed.is_err() || *cancellation.borrow() {
                    persist_partial_download(&mut output).await?;
                    return Err(SemanticMemoryError::Cancelled);
                }
                continue;
            }
        };
        let chunk = match next {
            Ok(Some(Ok(chunk))) => chunk,
            Ok(Some(Err(error))) => {
                persist_partial_download(&mut output).await?;
                return Err(SemanticMemoryError::Http(error.to_string()));
            }
            Ok(None) => break,
            Err(_) => {
                persist_partial_download(&mut output).await?;
                return Err(SemanticMemoryError::Http(
                    "download stopped receiving data for 60 seconds".into(),
                ));
            }
        };
        received_bytes = received_bytes
            .checked_add(chunk.len() as u64)
            .filter(|size| *size <= response_length)
            .ok_or(SemanticMemoryError::Integrity)?;
        hasher.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        bytes_since_sync = bytes_since_sync.saturating_add(chunk.len() as u64);
        if bytes_since_sync >= PARTIAL_SYNC_INTERVAL_BYTES {
            persist_partial_download(&mut output).await?;
            bytes_since_sync = 0;
        }

        if received_bytes == response_length || last_progress_at.elapsed() >= PROGRESS_MIN_INTERVAL
        {
            if let Err(error) = report_progress(
                progress,
                "downloading",
                completed_bytes
                    .saturating_add(response_offset)
                    .saturating_add(received_bytes),
                total_bytes,
                Some(asset.local_name),
            ) {
                persist_partial_download(&mut output).await?;
                return Err(error);
            }
            last_progress_at = Instant::now();
        }
    }
    persist_partial_download(&mut output).await?;
    drop(output);
    if received_bytes != response_length
        || response_offset.saturating_add(received_bytes) != asset.size
    {
        return Err(SemanticMemoryError::Http(
            "model host ended the download before the file was complete".to_owned(),
        ));
    }
    if format!("{:x}", hasher.finalize()) != asset.sha256 {
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        return Err(SemanticMemoryError::Integrity);
    }
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    tokio::fs::rename(&temporary, destination)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    Ok(())
}

pub(super) async fn persist_partial_download(
    output: &mut tokio::fs::File,
) -> Result<(), SemanticMemoryError> {
    output
        .flush()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    output
        .sync_data()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))
}

pub(super) fn parse_content_range(
    value: &str,
    expected_start: u64,
    expected_total: u64,
) -> Option<u64> {
    let (unit, range) = value.split_once(' ')?;
    if unit != "bytes" {
        return None;
    }
    let (positions, total) = range.split_once('/')?;
    if total.parse::<u64>().ok()? != expected_total {
        return None;
    }
    let (start, end) = positions.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    if start != expected_start || end < start || end >= expected_total {
        return None;
    }
    Some(end - start + 1)
}

pub(super) fn report_progress(
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    phase: &str,
    downloaded_bytes: u64,
    total_bytes: u64,
    file_name: Option<&str>,
) -> Result<(), SemanticMemoryError> {
    let mut data = serde_json::json!({
        "phase": phase,
        "downloadedBytes": downloaded_bytes,
        "totalBytes": total_bytes,
    });
    if let Some(file_name) = file_name {
        data["fileName"] = serde_json::json!(file_name);
    }
    progress
        .send(data)
        .map_err(|_| SemanticMemoryError::Worker("semantic progress receiver closed".to_owned()))
}
