use std::{
    error::Error,
    fmt::Debug,
    fs::File,
    io::{BufReader, Read},
    time::Duration,
};

use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{
    model::{CompletedMultipartUpload, CompletedPart, Part},
    presigning::config::PresigningConfig,
    types::{ByteStream, SdkError},
    Client, Endpoint,
};
use genbu_stores::{
    files::{
        database::{UploadLease, UploadLeaseStore, UploadLeaseStoreError},
        file_storage::{Bucket, FileError, FileStore, PresignError},
    },
    OffsetDateTime, Uuid,
};
use thiserror::Error;

use crate::types::StoreUploadLease;

#[derive(Clone)]
pub struct S3Store {
    client: Client,
}

// TODO: Move the error code into a separate file
// TODO: Properly match sdk errors, look at aws-sdk-rust changelog for more inforation
fn map_sdk_err<E: Error + 'static, R: Debug + 'static>(err: SdkError<E, R>) -> FileError {
    match err {
        SdkError::TimeoutError(_) => FileError::Connection(Box::new(err)),
        _ => FileError::Other(Box::new(err)),
    }
}

impl S3Store {
    async fn create_bucket(&mut self, bucket: Bucket) -> Result<(), FileError> {
        let resp = self
            .client
            .create_bucket()
            .bucket(bucket.to_bucket_name())
            .send()
            .await;
        match resp {
            Ok(_) => Ok(()),
            Err(SdkError::ServiceError(err))
                if err.err().is_bucket_already_exists()
                    || err.err().is_bucket_already_owned_by_you() =>
            {
                Ok(())
            }
            Err(e) => Err(map_sdk_err(e)),
        }
    }
    // TODO: Give server config here
    pub async fn new() -> Self {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let config = aws_config::from_env()
            .region(region_provider)
            .endpoint_resolver(Endpoint::immutable("http://127.0.0.1:9000").unwrap())
            .load()
            .await;
        let client = Client::new(&config);
        Self { client }
    }
}

#[async_trait]
impl FileStore for S3Store {
    fn can_presign() -> bool {
        true
    }

    async fn setup(&mut self) -> Result<(), FileError> {
        let buckets = vec![
            Bucket::UserFiles,
            Bucket::VideoFiles,
            Bucket::NotebookFiles,
            Bucket::ProfileImages,
        ];
        for &bucket in &buckets {
            self.create_bucket(bucket).await?;
        }
        Ok(())
    }

    async fn upload_file(
        &mut self,
        bucket: Bucket,
        file: &File,
        name: &str,
    ) -> Result<(), FileError> {
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        let stream = ByteStream::from(buffer);
        let res = self
            .client
            .put_object()
            .bucket(bucket.to_bucket_name())
            .key(name)
            .body(stream)
            .send()
            .await;
        res.map(|_| ()).map_err(map_sdk_err)
    }

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
        file_size: usize,
        chunk_size: usize,
    ) -> Result<(Vec<String>, String), FileError> {
        let mut chunk_count = (file_size / chunk_size) + 1;
        let size_of_last_chunk = file_size % chunk_size;

        if size_of_last_chunk == 0 {
            chunk_count -= 1;
        }

        if file_size == 0 {
            return Err(FileError::FileIsEmpty);
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
            let part_number = (chunk_index as i32) + 1;
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

#[async_trait]
impl UploadLeaseStore for S3Store {
    type StoreLease = StoreUploadLease;

    async fn int_add(&mut self, lease: &UploadLease) -> Result<(), UploadLeaseStoreError> {
        todo!()
    }

    async fn int_delete(
        &mut self,
        id: &Uuid,
    ) -> Result<Option<Self::StoreLease>, UploadLeaseStoreError> {
        todo!()
    }

    async fn int_get(&self, id: &Uuid) -> Result<Option<Self::StoreLease>, UploadLeaseStoreError> {
        todo!()
    }

    async fn int_get_by_user(
        &self,
        id: &Uuid,
    ) -> Result<Vec<Self::StoreLease>, UploadLeaseStoreError> {
        todo!()
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

#[derive(Debug, Error)]
#[error("no upload id was returned from store")]
struct NoUploadId;

fn new_presign_err<U, T: Error + 'static>(e: SdkError<T>) -> Result<U, FileError> {
    Err(FileError::Presigning(PresignError::Other(Box::new(e))))
}
