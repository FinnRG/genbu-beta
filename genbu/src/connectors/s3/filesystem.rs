use std::{error::Error, fmt::Debug};

use aws_sdk_s3::types::SdkError;
use aws_smithy_types_convert::date_time::DateTimeExt;

use crate::stores::{
    files::{
        filesystem::{Filesystem, FilesystemError, SResult, Userfile},
        storage::Bucket,
    },
    Uuid,
};

use super::S3Store;

fn map_sdk_err<E: Error + Send + Sync + 'static, R: Debug + Send + Sync + 'static>(
    err: SdkError<E, R>,
) -> FilesystemError {
    match err {
        SdkError::TimeoutError(_) => FilesystemError::Connection(Box::new(err)),
        _ => FilesystemError::Other(Box::new(err)),
    }
}

#[async_trait::async_trait]
impl Filesystem for S3Store {
    async fn list(&self, user_id: Uuid, base_path: &str) -> SResult<Vec<Userfile>> {
        let resp = self
            .client
            .list_objects_v2()
            .bucket(Bucket::UserFiles.to_bucket_name())
            .prefix(base_path.to_owned())
            .delimiter("\\".to_owned())
            .send()
            .await
            .map_err(map_sdk_err)?;
        Ok(resp
            .contents
            .unwrap_or_default()
            .iter()
            .map(|object| Userfile {
                name: object.key.clone().unwrap_or_default(),
                last_modified: object.last_modified.and_then(|t| t.to_time().ok()),
                owner: user_id,
                size: Some(object.size),
                is_folder: false,
            })
            .chain(
                resp.common_prefixes
                    .unwrap_or_default()
                    .iter()
                    .map(|common_prefix| Userfile {
                        name: common_prefix.prefix.clone().unwrap_or_default(),
                        last_modified: None,
                        owner: user_id,
                        size: None,
                        is_folder: true,
                    }),
            )
            .collect())
    }
    async fn delete(&mut self, path: &str) -> SResult<()> {
        self.client
            .delete_object()
            .bucket(Bucket::UserFiles.to_bucket_name())
            .key(path)
            .send()
            .await
            .map_err(map_sdk_err)?;
        Ok(())
    }
}
