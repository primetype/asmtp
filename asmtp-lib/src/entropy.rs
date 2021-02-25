use keynesis::memsec::Scrubbed as _;
use rand_core::{CryptoRng, RngCore};
use std::{
    convert::TryFrom,
    fmt::{self, Formatter},
    str::FromStr,
};

/// 64 random bytes used as _entropy_ for generating a [`Seed`].
///
/// For example one way to do it can be:
/// `Seed::derive_from_key(entropy.as_ref(), password)`
///
/// while any other byte slice can be used to derive the [`Seed`] from
/// this defines a standard to use to provide a stronger/safer
///
/// [`Seed`]: keynesis::Seed
#[derive(Clone, PartialEq, Eq)]
pub struct Entropy([u8; Self::SIZE]);

impl Entropy {
    pub const SIZE: usize = 64;

    pub fn generate<RNG>(mut rng: RNG) -> Self
    where
        RNG: RngCore + CryptoRng,
    {
        let mut entropy = Self([0; Self::SIZE]);

        rng.fill_bytes(&mut entropy.0);

        entropy
    }
}

impl Drop for Entropy {
    fn drop(&mut self) {
        self.0.scrub()
    }
}

impl AsRef<[u8]> for Entropy {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl fmt::Display for Entropy {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(&self.0))
    }
}

impl fmt::Debug for Entropy {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Entropy")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl FromStr for Entropy {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl TryFrom<String> for Entropy {
    type Error = hex::FromHexError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_ref())
    }
}

impl<'a> TryFrom<&'a str> for Entropy {
    type Error = hex::FromHexError;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let mut entropy = Entropy([0; Self::SIZE]);

        hex::decode_to_slice(value, &mut entropy.0)?;

        Ok(entropy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{Arbitrary, Gen};

    impl Arbitrary for Entropy {
        fn arbitrary(g: &mut Gen) -> Self {
            let mut entropy = Entropy([0; Self::SIZE]);
            for byte in entropy.0.iter_mut() {
                *byte = u8::arbitrary(g);
            }
            entropy
        }
    }

    #[quickcheck]
    fn to_string_from_str(entropy: Entropy) -> bool {
        let s = entropy.to_string();
        let decoded = s.parse().unwrap();

        entropy == decoded
    }
}
