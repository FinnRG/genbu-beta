use wopi_rs::file::{CheckFileInfoRequest, FileRequest, FileRequestType};

use crate::stores::{files::filesystem::Filesystem, users::User};

pub async fn wopi_file<'a>(
    mut filesystem: impl Filesystem,
    user: &'a User,
    file_req: FileRequest<&'a [u8]>,
) -> http::Request<&'a [u8]> {
    let res = match file_req.request {
        FileRequestType::CheckFileInfo(r) => handle_check_file_info(filesystem, user, r),
        _ => todo!(),
    };
    res.await
}

async fn handle_check_file_info(
    mut filesystem: impl Filesystem,
    user: &User,
    file_req: CheckFileInfoRequest,
) -> http::Request<&[u8]> {
    todo!()
}
