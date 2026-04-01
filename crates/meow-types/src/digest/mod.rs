pub mod error;

use std::{fmt, str::FromStr};

use blake2::{Blake2b, digest::consts::U32};
use error::DigestError;
use serde::{Deserialize, Serialize};

/// The result type related to addresses.
pub type Result<T> = std::result::Result<T, DigestError>;

/// The digest length.
pub const DIGEST_LENGTH: usize = 32;

/// The meow digest type.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub struct Digest([u8; DIGEST_LENGTH]);

impl Digest {
    /// The zero digest constant.
    pub const ZERO: Self = Self([0; DIGEST_LENGTH]);

    /// Creates a new digest.
    pub fn new(bytes: [u8; DIGEST_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Computes the digest of the given signable data.
    pub fn compute<T: ?Sized + Serialize>(signable: &T) -> Result<Self> {
        use blake2::Digest;

        let mut hasher = Blake2b::<U32>::default();

        hasher.update(&bcs::to_bytes(signable)?);

        Ok(Self::new(hasher.finalize().into()))
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(s: &str) -> Result<Self> {
        let bytes: Vec<u8> = bs58::decode(s).into_vec()?;

        Digest::try_from(bytes.as_slice())
    }
}

impl TryFrom<&[u8]> for Digest {
    type Error = DigestError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        <[u8; DIGEST_LENGTH]>::try_from(bytes)
            .map_err(|_| DigestError::InvalidDigestBytesLength {
                actual: bytes.len(),
                expected: DIGEST_LENGTH,
            })
            .map(Digest)
    }
}

impl AsRef<[u8]> for Digest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
