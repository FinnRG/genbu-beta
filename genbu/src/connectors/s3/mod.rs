use std::{error::Error, fmt::Debug};

use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{types::SdkError, Client, Endpoint};

use crate::stores::{
    files::storage::{Bucket, FileError},
    Reset, Setup,
};

pub mod database;
pub mod filesystem;
pub mod storage;

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

    async fn delete_bucket(&mut self, bucket: Bucket) -> Result<(), FileError> {
        let resp = self
            .client
            .delete_bucket()
            .bucket(bucket.to_bucket_name())
            .send()
            .await;
        resp.map(|_| ()).map_err(map_sdk_err)
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

const BUCKETS: [Bucket; 4] = [
    Bucket::UserFiles,
    Bucket::VideoFiles,
    Bucket::NotebookFiles,
    Bucket::ProfileImages,
];

#[async_trait]
impl Reset for S3Store {
    #[cfg(debug_assertions)]
    async fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        for bucket in BUCKETS {
            self.delete_bucket(bucket).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Setup for S3Store {
    async fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        for bucket in BUCKETS {
            self.create_bucket(bucket).await?;
        }
        Ok(())
    }
}
