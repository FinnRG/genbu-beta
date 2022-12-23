// Used to convert the internal user representations into the standard user.
pub fn deep_into<T: Into<U>, U, E: Into<F>, F>(res: Result<Option<T>, E>) -> Result<Option<U>, F> {
    match res {
        Ok(Some(u)) => Ok(Some(u.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn deep_into_vec<T: Into<U>, U, E: Into<F>, F>(res: Result<Vec<T>, E>) -> Result<Vec<U>, F> {
    match res {
        Ok(v) => Ok(v.into_iter().map(Into::into).collect::<Vec<U>>()),
        Err(e) => Err(e.into()),
    }
}
