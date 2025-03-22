pub trait ShaImpl {
    type S256: Sha256;
    type S1: Sha1;
}

pub trait Sha256 {
    fn new() -> Self;
    fn update(&mut self, buf: &[u8]);
    fn finalize(self) -> [u8; 32];
}

pub trait Sha1: Send {
    fn new() -> Self;
    fn update(&mut self, buf: &[u8]);
    fn finalize(self) -> [u8; 20];
}

pub struct Unimplemented;
impl ShaImpl for Unimplemented {
    type S256 = Unimplemented;
    type S1 = Unimplemented;
}
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
impl Sha1 for Unimplemented {
    fn new() -> Self {
        unimplemented!()
    }
    fn update(&mut self, __buf: &[u8]) {
        unimplemented!()
    }
    fn finalize(self) -> [u8; 20] {
        unimplemented!()
    }
}
#[cfg(not(any(
    feature = "sha-ring",
    feature = "sha-aws-lc-rs",
    feature = "sha-openssl"
)))]
pub type Default = Unimplemented;

#[cfg(feature = "sha-ring")]
pub struct Ring;
#[cfg(feature = "sha-ring")]
pub struct Ring256 {
    inner: ring::digest::Context,
}
#[cfg(feature = "sha-ring")]
pub struct Ring1 {
    inner: ring::digest::Context,
}
#[cfg(feature = "sha-ring")]
impl ShaImpl for Ring {
    type S256 = Ring256;
    type S1 = Ring1;
}
#[cfg(feature = "sha-ring")]
impl Sha256 for Ring256 {
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
#[cfg(feature = "sha-ring")]
impl Sha1 for Ring1 {
    fn new() -> Self {
        Self {
            inner: ring::digest::Context::new(&ring::digest::SHA1),
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
    feature = "sha-ring",
    not(any(feature = "sha-aws-lc-rs", feature = "sha-openssl"))
))]
pub type Default = Ring;

#[cfg(feature = "sha-aws-lc-rs")]
pub struct AwsLcRs;
#[cfg(feature = "sha-aws-lc-rs")]
pub struct AwsLcRs256 {
    inner: aws_lc_rs::digest::Context,
}
#[cfg(feature = "sha-aws-lc-rs")]
pub struct AwsLcRs1 {
    inner: aws_lc_rs::digest::Context,
}
#[cfg(feature = "sha-aws-lc-rs")]
impl ShaImpl for AwsLcRs {
    type S256 = AwsLcRs256;
    type S1 = AwsLcRs1;
}
#[cfg(feature = "sha-aws-lc-rs")]
impl Sha256 for AwsLcRs256 {
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
#[cfg(feature = "sha-aws-lc-rs")]
impl Sha256 for AwsLcRs1 {
    fn new() -> Self {
        Self {
            inner: aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA1),
        }
    }
    fn update(&mut self, buf: &[u8]) {
        self.inner.update(buf)
    }
    fn finalize(self) -> [u8; 20] {
        self.inner.finish().as_ref().try_into().unwrap()
    }
}

#[cfg(all(feature = "sha-aws-lc-rs", not(feature = "sha-openssl")))]
pub type Default = AwsLcRs;

#[cfg(feature = "sha-openssl")]
pub struct Openssl;
#[cfg(feature = "sha-openssl")]
pub struct Openssl256 {
    inner: openssl::sha::Sha256,
}
#[cfg(feature = "sha-openssl")]
pub struct Openssl1 {
    inner: openssl::sha::Sha1,
}
#[cfg(feature = "sha-openssl")]
impl ShaImpl for Openssl {
    type S256 = Openssl256;

    type S1 = Openssl1;
}
#[cfg(feature = "sha-openssl")]
impl Sha256 for Openssl256 {
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
#[cfg(feature = "sha-openssl")]
impl Sha1 for Openssl1 {
    fn new() -> Self {
        Self {
            inner: openssl::sha::Sha1::new(),
        }
    }
    fn update(&mut self, buf: &[u8]) {
        self.inner.update(buf);
    }
    fn finalize(self) -> [u8; 20] {
        self.inner.finish()
    }
}
#[cfg(feature = "sha-openssl")]
pub type Default = Openssl;
