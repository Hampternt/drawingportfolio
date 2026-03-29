use aws_sdk_s3::{Client, config::Region, primitives::ByteStream};
use aws_config::BehaviorVersion;

pub struct ObjectStorage {
    client: Client,
    bucket: String,
    public_url: String,
}

impl ObjectStorage {
    pub async fn from_env() -> Self {
        let endpoint = std::env::var("STORAGE_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:9000".to_string());
        let bucket = std::env::var("STORAGE_BUCKET")
            .unwrap_or_else(|_| "portfolio-images".to_string());
        let public_url = std::env::var("STORAGE_PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:3000/static".to_string());

        let config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&endpoint)
            .region(Region::new("auto"))
            .load()
            .await;

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(s3_config);

        ObjectStorage { client, bucket, public_url }
    }

    /// Upload bytes, return the public URL for the stored object.
    pub async fn upload(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<String, String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| format!("upload failed: {e}"))?;

        Ok(format!("{}/{}", self.public_url.trim_end_matches('/'), key))
    }

    /// Delete an object by its key (extracted from its public URL).
    pub async fn delete_by_url(&self, image_url: &str) -> Result<(), String> {
        let key = image_url
            .trim_start_matches(self.public_url.trim_end_matches('/'))
            .trim_start_matches('/');

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| format!("delete failed: {e}"))?;

        Ok(())
    }
}
