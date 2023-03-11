use std::time::Duration;

use aws_sdk_s3::{
    model::{CompletedMultipartUpload, CompletedPart},
    presigning::config::PresigningConfig,
    types::{ByteStream, SdkError},
};
use tracing::error;

use crate::stores::files::{
    storage::{Bucket, FileError, InvalidPartSize, Part, PresignError},
    FileStorage,
};

use super::{map_sdk_err, S3Store};

#[async_trait::async_trait]
impl FileStorage for S3Store {
    async fn delete_file(&mut self, bucket: Bucket, name: &str) -> Result<(), FileError> {
        let res = self
            .client
            .delete_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .send()
            .await;
        res.map(|_| ()).map_err(map_sdk_err)
    }

    async fn get_download_url(&self, bucket: Bucket, name: &str) -> Result<String, FileError> {
        let res = self
            .client
            .get_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .set_response_content_disposition(Some("attachment".to_owned()))
            .presigned(PresigningConfig::expires_in(Duration::from_secs(1800)).unwrap())
            .await;
        res.map(|r| r.uri().to_string()).map_err(map_sdk_err)
    }

    async fn get_presigned_upload_urls(
        &self,
        bucket: Bucket,
        file: &str,
        file_size: u64,
        chunk_size: u64,
    ) -> Result<(Vec<String>, String), FileError> {
        let mut chunk_count = (file_size / chunk_size) + 1;
        let size_of_last_chunk = file_size % chunk_size;

        if chunk_count > 1 && size_of_last_chunk == 0 {
            chunk_count -= 1;
        }

        let mut upload_parts = Vec::new();

        let multipart_upload = self
            .client
            .create_multipart_upload()
            .bucket(bucket.to_bucket_name())
            .key(file)
            .send()
            .await
            .map_err(map_sdk_err)?;
        let Some(upload_id) = multipart_upload.upload_id() else {
            error!("Failed to retrive upload id");
            return Err(FileError::Other(Box::new(NoUploadId)));
        };

        for chunk_index in 0..chunk_count {
            let chunk_index: i32 = chunk_index
                .try_into()
                .map_err(|_| FileError::Other(InvalidPartSize.into()))?;
            let part_number = chunk_index + 1;
            let presign_res = self
                .client
                .upload_part()
                .key(file)
                .bucket(bucket.to_bucket_name())
                .upload_id(upload_id)
                .part_number(part_number)
                .presigned(PresigningConfig::expires_in(Duration::from_secs(1800)).unwrap())
                .await;
            let presign_res = match presign_res {
                Ok(res) => res,
                Err(e) => return new_presign_err(e),
            };
            upload_parts.push(presign_res.uri().to_string());
        }
        Ok((upload_parts, upload_id.into()))
    }

    async fn finish_multipart_upload(
        &self,
        bucket: Bucket,
        file: &str,
        upload_id: &str,
        parts: Vec<Part>,
    ) -> Result<(), FileError> {
        let completed_multipart_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(
                parts
                    .into_iter()
                    .map(|part| {
                        CompletedPart::builder()
                            .set_e_tag(Some(part.e_tag))
                            .set_part_number(Some(part.part_number))
                            .build()
                    })
                    .collect(),
            ))
            .build();
        self.client
            .complete_multipart_upload()
            .bucket(bucket.to_bucket_name())
            .key(file)
            .upload_id(upload_id)
            .multipart_upload(completed_multipart_upload)
            .send()
            .await
            .map(|_| ())
            .map_err(map_sdk_err)
    }

    async fn upload(&mut self, bucket: Bucket, name: &str, data: Vec<u8>) -> Result<(), FileError> {
        self.client
            .put_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .body(ByteStream::from(data))
            .send()
            .await
            .map(|_| ())
            .map_err(map_sdk_err)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("no upload id was returned from store")]
struct NoUploadId;

fn new_presign_err<U, T: std::error::Error + 'static>(e: SdkError<T>) -> Result<U, FileError> {
    Err(FileError::Presigning(PresignError::Other(Box::new(e))))
}
