use crate::actors::page_renderer::{FileData, FilePart};
use crate::config::CONFIG;
use actix_multipart::Multipart;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::io::Write;
use path_clean::PathClean;

pub async fn handle_multipart(
    mut multipart: Multipart,
) -> (
    serde_json::Map<String, serde_json::Value>,
    HashMap<String, FilePart>,
) {
    let mut form_data = serde_json::Map::new();
    let mut files = HashMap::new();
    
    let temp_dir = match &CONFIG.temp_dir {
        Some(dir) if !dir.is_empty() => {
            let path = std::path::PathBuf::from(dir);
            let cleaned_path = path.clean();
            if cleaned_path.is_absolute() {
                cleaned_path
            } else {
                std::env::current_dir().unwrap().join(cleaned_path)
            }
        }
        _ => std::env::temp_dir(),
    };

    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        log::error!("Failed to create temporary directory '{}': {}", temp_dir.display(), e);
    }

    while let Some(item) = multipart.next().await {
        let mut field = item.unwrap();
        let content_disposition = field.content_disposition().unwrap();
        let field_name = content_disposition.get_name().unwrap().to_string();

        if let Some(filename) = content_disposition.get_filename() {
            let filename = filename.to_string();
            let mut buffer = Vec::new();
            let mut file_data: Option<FileData> = None;
            let mut temp_file: Option<std::fs::File> = None;

            let content_type = field
                .content_type()
                .map(|mime| mime.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            let headers = field
                .headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_string()))
                .collect();

            while let Some(chunk) = field.next().await {
                let chunk = chunk.unwrap();
                if file_data.is_none() {
                    let max_size = CONFIG.max_memory_size.unwrap_or(500 * 1024); // 500 KB default
                    if buffer.len() + chunk.len() > max_size {
                        let temp_file_path = temp_dir.join(uuid::Uuid::new_v4().to_string());
                        let absolute_path = std::fs::canonicalize(&temp_file_path).unwrap_or_else(|_| temp_file_path.clone());
                        log::info!("Streaming file upload '{}' to disk: {}", filename, absolute_path.display());
                        let mut file = std::fs::File::create(&temp_file_path).unwrap();
                        file.write_all(&buffer).unwrap();
                        file.write_all(&chunk).unwrap();
                        temp_file = Some(file);
                        file_data = Some(FileData::OnDisk(temp_file_path));
                        buffer.clear();
                    } else {
                        buffer.extend_from_slice(&chunk);
                    }
                } else {
                    temp_file.as_mut().unwrap().write_all(&chunk).unwrap();
                }
            }

            let final_file_data = file_data.unwrap_or(FileData::InMemory(buffer));

            files.insert(
                field_name,
                FilePart {
                    filename,
                    content_type,
                    headers,
                    data: final_file_data,
                },
            );
        } else {
            let mut buffer = Vec::new();
            while let Some(chunk) = field.next().await {
                buffer.extend_from_slice(&chunk.unwrap());
            }
            form_data.insert(
                field_name,
                serde_json::Value::String(String::from_utf8(buffer).unwrap()),
            );
        }
    }
    (form_data, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
    use actix_multipart::Multipart;
    use actix_web::{
        error::PayloadError,
        web::{Bytes, Payload},
    };
    use futures_util::stream::iter;
    use std::pin::Pin;
    use actix_rt;

#[test]
fn test_handle_multipart_in_memory() {
    use actix_rt::System;
    System::new().block_on(async {
        let body = Bytes::from(
            "--boundary\r\n\
            Content-Disposition: form-data; name=\"field1\"\r\n\r\n\
            value1\r\n\
            --boundary\r\n\
            Content-Disposition: form-data; name=\"file1\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\r\n\
            Hello, world!\r\n\
            --boundary--\r\n",
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=boundary"),
        );

        let stream = iter(vec![Ok::<_, PayloadError>(body)]);
        let payload = actix_http::Payload::from(Box::pin(stream) as Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, PayloadError>>>>);

        let multipart = Multipart::new(&headers, payload);
        let (form_data, files) = handle_multipart(multipart).await;

        assert_eq!(form_data.len(), 1);
        assert_eq!(
            form_data.get("field1").unwrap(),
            &serde_json::Value::String("value1".to_string())
        );

        assert_eq!(files.len(), 1);
        let file_part = files.get("file1").unwrap();
        assert_eq!(file_part.filename, "test.txt");
        assert_eq!(file_part.content_type, "text/plain");

        if let FileData::InMemory(data) = &file_part.data {
            assert_eq!(data, &b"Hello, world!");
        } else {
            panic!("Expected file data to be in memory");
        }
    });
}

#[test]
fn test_handle_multipart_on_disk() {
    use actix_rt::System;
    System::new().block_on(async {
        use actix_http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
        use actix_web::{
            error::PayloadError,
            web::{Bytes},
        };
        use futures_util::stream::iter;
        use std::pin::Pin;
        let mut body = Vec::new();
        body.extend_from_slice(b"--boundary\r\n");
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"file1\"; filename=\"test.txt\"\r\n");
        body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
        let large_data = vec![0; 1024 * 1024]; // 1MB
        body.extend_from_slice(&large_data);
        body.extend_from_slice(b"\r\n--boundary--\r\n");

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=boundary"),
        );

        let stream = iter(vec![Ok::<_, PayloadError>(Bytes::from(body))]);
        let payload = actix_http::Payload::from(Box::pin(stream) as Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, PayloadError>>>>);

        let multipart = Multipart::new(&headers, payload);
        let (_form_data, files) = handle_multipart(multipart).await;

        assert_eq!(files.len(), 1);
        let file_part = files.get("file1").unwrap();
        assert_eq!(file_part.filename, "test.txt");

        if let FileData::OnDisk(path) = &file_part.data {
            let file_content = std::fs::read(path).unwrap();
            assert_eq!(file_content.len(), large_data.len());
            std::fs::remove_file(path).unwrap();
        } else {
            panic!("Expected file data to be on disk");
        }
    });
}
}