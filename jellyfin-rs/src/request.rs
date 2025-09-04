use crate::{AuthStatus, JellyfinClient, Result, request::sealed::QuerySealed};
use http::{
    Method,
    header::{CONTENT_TYPE, HOST},
};
use serde::Serialize;

impl<Auth: AuthStatus> JellyfinClient<Auth> {
    pub fn build_path(&self, uri: impl PathBuilder, query: impl Query) -> Result<String> {
        let mut path = self.uri_base.clone();
        uri.append(&mut path);
        query.append(&mut path)?;
        Ok(path)
    }
    pub fn request(
        &self,
        uri: impl PathBuilder,
        query: impl Query,
    ) -> Result<http::request::Builder> {
        let builder = http::request::Builder::new()
            .uri(self.build_path(uri, query)?)
            .header(HOST, self.host_header.clone());
        Ok(self.auth.add_auth_header(builder))
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
        Ok(self
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(val)?)?)
    }

    fn empty_body(self) -> Result<http::Request<String>> {
        Ok(self.body(String::new())?)
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
