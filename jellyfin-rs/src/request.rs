use crate::{request::sealed::QuerySealed, AuthStatus, JellyfinClient, Result};
use http::{
    Method, header::{CONTENT_LENGTH, CONTENT_TYPE, HOST}
};
use serde::Serialize;
use tracing::debug;

impl<Auth: AuthStatus> JellyfinClient<Auth> {
    pub fn build_uri(&self, uri: impl PathBuilder, query: impl Query) -> Result<String> {
        let mut path = self.inner.uri_base.clone();
        uri.append(&mut path);
        query.append(&mut path)?;
        Ok(path)
    }
    pub fn request(
        &self,
        uri: impl PathBuilder,
        query: impl Query,
    ) -> Result<http::request::Builder> {
        let uri = self.build_uri(uri, query)?;
        debug!("sending request to {uri}");
        let builder = http::request::Builder::new()
            .uri(uri)
            .header(HOST, self.inner.host_header.clone());
        Ok(self.inner.auth.add_auth_header(builder))
    }

    pub fn get(&self, uri: impl PathBuilder, query: impl Query) -> Result<http::request::Builder> {
        self.request(uri, query)
    }
    pub fn post(&self, uri: impl PathBuilder, query: impl Query) -> Result<http::request::Builder> {
        Ok(self.request(uri, query)?.method(Method::POST))
    }
    pub fn delete(
        &self,
        uri: impl PathBuilder,
        query: impl Query,
    ) -> Result<http::request::Builder> {
        Ok(self.request(uri, query)?.method(Method::DELETE))
    }
}

pub trait RequestBuilderExt {
    fn json_body(self, val: &impl Serialize) -> Result<http::Request<String>>;
    fn empty_body(self) -> Result<http::Request<String>>;
}

impl RequestBuilderExt for http::request::Builder {
    fn json_body(self, val: &impl Serialize) -> Result<http::Request<String>> {
        let body = serde_json::to_string(val)?;
        let len = body.len().to_string();
        Ok(self.header(CONTENT_LENGTH, len)
            .header(CONTENT_TYPE, "application/json")
            .body(body)?)
    }

    fn empty_body(self) -> Result<http::Request<String>> {
        Ok(self.header(CONTENT_LENGTH, "0").body(String::new())?)
    }
}

mod sealed {
    use serde::Serialize;

    use crate::request::NoQuery;

    pub trait PathBuilderSealed {}
    impl PathBuilderSealed for &str {}
    impl<F: FnOnce(&mut String)> PathBuilderSealed for F {}

    pub trait QuerySealed {}
    impl QuerySealed for NoQuery {}
    impl<S: Serialize> QuerySealed for S {}
}

pub trait PathBuilder: sealed::PathBuilderSealed {
    fn append(self, str: &mut String);
}
impl PathBuilder for &str {
    fn append(self, str: &mut String) {
        str.push_str(self);
    }
}
impl<F: FnOnce(&mut String)> PathBuilder for F {
    fn append(self, str: &mut String) {
        self(str)
    }
}

pub trait Query: QuerySealed {
    fn append(self, str: &mut String) -> std::result::Result<(), serde_urlencoded::ser::Error>;
}
pub struct NoQuery;
impl Query for NoQuery {
    #[inline(always)]
    fn append(self, _str: &mut String) -> std::result::Result<(), serde_urlencoded::ser::Error> {
        Ok(())
    }
}
impl<S: Serialize> Query for S {
    fn append(self, str: &mut String) -> std::result::Result<(), serde_urlencoded::ser::Error> {
        str.push('?');
        str.push_str(&serde_urlencoded::to_string(self)?);
        Ok(())
    }
}
