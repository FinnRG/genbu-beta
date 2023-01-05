use std::time::Duration;

use aws_sdk_s3::{
    model::{CompletedMultipartUpload, CompletedPart, Part},
    presigning::config::PresigningConfig,
    types::SdkError,
};

use crate::stores::files::{
    storage::{Bucket, FileError, InvalidPartSize, PresignError},
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

    async fn get_presigned_url(&self, bucket: Bucket, name: &str) -> Result<String, FileError> {
        let expires_in = Duration::from_secs(20);
        let presigned_request = self
            .client
            .get_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .presigned(PresigningConfig::expires_in(expires_in).unwrap())
            .await;
        match presigned_request {
            Ok(req) => Ok(req.uri().to_string()),
            Err(e) => new_presign_err(e),
        }
    }

    async fn get_presigned_upload_url(
        &self,
        bucket: Bucket,
        name: &str,
    ) -> Result<String, FileError> {
        let expires_in = Duration::from_secs(900);
        let presigned_request = self
            .client
            .put_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .presigned(PresigningConfig::expires_in(expires_in).unwrap())
            .await;
        match presigned_request {
            Ok(req) => Ok(req.uri().to_string()),
            Err(e) => new_presign_err(e),
        }
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

        if size_of_last_chunk == 0 {
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
    ) -> Result<(), FileError> {
        let parts = self
            .client
            .list_parts()
            .bucket(bucket.to_bucket_name())
            .upload_id(upload_id)
            .key(file)
            .send()
            .await
            .map_err(map_sdk_err)?;
        let completed_multipart_upload = CompletedMultipartUpload::builder()
            .set_parts(
                parts
                    .parts()
                    .map(|parts| parts.iter().map(part_to_completed).collect()),
            )
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
}

fn part_to_completed(p: &Part) -> CompletedPart {
    CompletedPart::builder()
        .set_e_tag(p.e_tag().map(Into::into))
        .part_number(p.part_number())
        .set_checksum_sha1(p.checksum_sha1().map(Into::into))
        .set_checksum_crc32(p.checksum_crc32().map(Into::into))
        .set_checksum_sha256(p.checksum_sha256().map(Into::into))
        .set_checksum_crc32_c(p.checksum_crc32_c().map(Into::into))
        .build()
}

#[derive(Debug, thiserror::Error)]
#[error("no upload id was returned from store")]
struct NoUploadId;

fn new_presign_err<U, T: std::error::Error + 'static>(e: SdkError<T>) -> Result<U, FileError> {
    Err(FileError::Presigning(PresignError::Other(Box::new(e))))
}
