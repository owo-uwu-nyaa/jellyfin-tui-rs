pub trait Sha256 {
    fn new() -> Self;
    fn update(&mut self, buf: &[u8]);
    fn finalize(self) -> [u8; 32];
}

pub struct Unimplemented {}
impl Sha256 for Unimplemented {
    fn new() -> Self {
        unimplemented!()
    }
    fn update(&mut self, __buf: &[u8]) {
        unimplemented!()
    }
    fn finalize(self) -> [u8; 32] {
        unimplemented!()
    }
}
#[cfg(not(any(
    feature = "sha2-ring",
    feature = "sha2-aws-lc-rs",
    feature = "sha2-openssl"
)))]
pub type Default = Unimplemented;

#[cfg(feature = "sha2-ring")]
pub struct Ring {
    inner: ring::digest::Context,
}
#[cfg(feature = "sha2-ring")]
impl Sha256 for Ring {
    fn new() -> Self {
        Self {
            inner: ring::digest::Context::new(&ring::digest::SHA256),
        }
    }
    fn update(&mut self, buf: &[u8]) {
        self.inner.update(buf);
    }
    fn finalize(self) -> [u8; 32] {
        self.inner.finish().as_ref().try_into().unwrap()
    }
}
#[cfg(all(
    feature = "sha2-ring",
    not(any(feature = "sha2-aws-lc-rs", feature = "sha2-openssl"))
))]
pub type Default = Ring;

#[cfg(feature = "sha2-aws-lc-rs")]
pub struct AwsLcRs {
    inner: aws_lc_rs::digest::Context,
}
#[cfg(feature = "sha2-aws-lc-rs")]
impl Sha256 for AwsLcRs {
    fn new() -> Self {
        Self {
            inner: aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256),
        }
    }
    fn update(&mut self, buf: &[u8]) {
        self.inner.update(buf)
    }
    fn finalize(self) -> [u8; 32] {
        self.inner.finish().as_ref().try_into().unwrap()
    }
}
#[cfg(all(feature = "sha2-aws-lc-rs", not(feature = "sha2-openssl")))]
pub type Default = AwsLcRs;

#[cfg(feature = "sha2-openssl")]
pub struct Openssl {
    inner: openssl::sha::Sha256,
}
#[cfg(feature = "sha2-openssl")]
impl Sha256 for Openssl {
    fn new() -> Self {
        Self {
            inner: openssl::sha::Sha256::new(),
        }
    }
    fn update(&mut self, buf: &[u8]) {
        self.inner.update(buf);
    }
    fn finalize(self) -> [u8; 32] {
        self.inner.finish()
    }
}
#[cfg(feature = "sha2-openssl")]
pub type Default = Openssl;
