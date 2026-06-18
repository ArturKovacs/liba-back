use axum::body::Body;
use axum::http::{StatusCode, response::Builder};
use log::{debug, error, warn};
use std::convert::Infallible;
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::RwLock;
use tower_service::Service;

/// In-memory cache for file contents
pub type FileCache = RwLock<HashMap<String, Vec<u8>>>;

struct FileServerInner {
    pub canonical_base_path: PathBuf,
    pub cache: FileCache,
}

/// A tower Service that serves files from a directory with in-memory caching
#[derive(Clone)]
pub struct FileServer {
    inner: Arc<FileServerInner>
}

impl FileServer {
    /// Create a new FileServer for the given base path
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self, &'static str> {
        let canonical_base_path = base_path.into().canonicalize().map_err(|_| {
            "Failed to cannonicalize base_path in FileServer::new"
        })?;
        Ok(Self {
            inner: Arc::new(FileServerInner { 
                canonical_base_path,
                cache: Default::default()
            })
        })
    }
}

type Response = axum::response::Response<axum::body::Body>;
type Request = axum::http::Request<axum::body::Body>;

impl Service<Request> for FileServer {
    type Response = Response;

    // For reasons beyond me, axum requires the fallback service to be Infallible.
    // ALthough it appaears to allow returning any status through the status of the Ok(Response)
    type Error = Infallible; 

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let server = Arc::clone(&self.inner);
        let path = request.uri().path().to_string();

        Box::pin(async move {
            // Prevent directory traversal attacks
            if path.contains("..") {
                warn!("Attempted directory traversal: {}", path);
                return create_error_response(StatusCode::FORBIDDEN);
            }

            // Check cache first
            {
                let cache_lock = server.cache.read().await;
                if let Some(content) = cache_lock.get(&path) {
                    debug!("Serving '{}' from cache", path);
                    let filename = path.split('/').last().unwrap_or("");
                    return create_response(content.clone(), filename);
                }
            }

            // Not in cache, load from disk
            let file_path = server.canonical_base_path.join(path.trim_start_matches('/'));

            // Verify the file is within base_path
            let canonical_file = match file_path.canonicalize() {
                Ok(path) => path,
                Err(_) => {
                    warn!("Failed to canonicalize file path {:?}", file_path);
                    return create_error_response(StatusCode::NOT_FOUND);
                }
            };

            if !canonical_file.starts_with(&server.canonical_base_path) {
                warn!("Attempted access outside base directory: {}", path);
                return create_error_response(StatusCode::FORBIDDEN);
            }

            // Read file from disk
            let content = match tokio::fs::read(&canonical_file).await {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to read file '{}': {}", path, e);
                    return create_error_response(StatusCode::NOT_FOUND);
                }
            };

            // Store in cache
            {
                let mut cache_lock = server.cache.write().await;
                cache_lock.insert(path.clone(), content.clone());
                debug!("Cached file '{}'", path);
            }

            let filename = path.split('/').last().unwrap_or("");
            create_response(content, filename)
        })
    }
}

fn infer_content_type_from_filename(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    
    match lower.split('.').last().unwrap_or("") {
        // Text files
        "txt" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "xml" => "application/xml",
        "json" => "application/json",
        
        // JavaScript
        "js" => "application/javascript",
        "mjs" => "application/javascript",
        "ts" => "application/typescript",
        
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "ogg" => "audio/ogg",
        
        // Video
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        
        // Archives
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/x-rar-compressed",
        
        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        
        // Fonts
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        
        // Default
        _ => "application/octet-stream",
    }
}

fn create_error_response(status: StatusCode) -> Result<Response, Infallible> {
    Ok(Builder::new()
        .status(status)
        .body(Body::empty())
        .unwrap_or_default())
}

fn create_response(contents: Vec<u8>, filename: &str) -> Result<Response, Infallible> {
    Ok(Builder::new()
        .status(StatusCode::OK)
        .header("Content-Type", infer_content_type_from_filename(filename))
        .body(contents.into())
        .unwrap_or_else(|_| create_error_response(StatusCode::INTERNAL_SERVER_ERROR).unwrap())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use axum::{body::Bytes, http::Request as HttpRequestType};
    use tower_http::body;

    const MAX_BODY_LEN: usize = 1024;

    async fn get_response_body(response: Response) -> Bytes {
        axum::body::to_bytes(response.into_body(), MAX_BODY_LEN).await.unwrap()
    }

    fn create_request(path: &str) -> Request {
        HttpRequestType::builder()
            .uri(path)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn test_file_caching() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, b"Hello, World!").unwrap();

        let mut file_server = FileServer::new(dir.path()).unwrap();

        // First read should load from disk
        let request = create_request("/test.txt");
        let result = file_server.call(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        
        let body = get_response_body(response).await;
        let expected_body = Bytes::from(Vec::from(b"Hello, World!"));

        assert_eq!(body, expected_body);
    }

    #[tokio::test]
    async fn test_directory_traversal_prevention() {
        let root_dir = tempdir().unwrap();
        let child_dir = root_dir.path().join("child");
        fs::create_dir(&child_dir).unwrap();

        // Create a file in the parent directory (outside the allowed base path)
        let parent_file = root_dir.path().join("secret.txt");
        fs::write(&parent_file, b"Secret content").unwrap();

        // Create a file in the child directory (inside the allowed base path)
        let child_file = child_dir.join("public.txt");
        fs::write(&child_file, b"Public content").unwrap();

        // Create FileServer with only the child directory as base path
        let mut file_server = FileServer::new(&child_dir).unwrap();

        // Should be able to access files in the child directory
        let request = create_request("/public.txt");
        let result = file_server.call(request).await;
        assert!(result.is_ok());

        let body = get_response_body(result.unwrap()).await;
        let expected_body = Bytes::from(Vec::from(b"Public content"));

        assert_eq!(body, expected_body);

        // Should NOT be able to access files in the parent directory via directory traversal
        let request = create_request("/../secret.txt");
        let result = file_server.call(request).await;
        let success = match result {
            Ok(response) => response.status().is_success(),
            Err(_) => false
        };
        assert_eq!(success, false);
    }

    #[tokio::test]
    async fn test_cache_persists_after_file_modification() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let original_content = b"Original content";
        fs::write(&file_path, original_content).unwrap();

        let mut file_server = FileServer::new(dir.path()).unwrap();

        // First read should load from disk and cache
        let request = create_request("/test.txt");
        let result = file_server.call(request).await;
        assert!(result.is_ok());
        let response = result.unwrap();

        let body = get_response_body(response).await;
        let expected_body = Bytes::from(Vec::from(original_content));
        assert_eq!(body, expected_body);

        // Modify the file on disk
        fs::write(&file_path, b"Modified content").unwrap();

        // Second read should return the cached value, not the modified one
        let request = create_request("/test.txt");
        let result = file_server.call(request).await;
        assert!(result.is_ok());
        let response = result.unwrap();

        let body = get_response_body(response).await;
        let expected_body = Bytes::from(Vec::from(original_content));
        assert_eq!(body, expected_body, "Should serve cached content, not the modified file");
    }
}
