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