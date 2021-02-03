use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use std::{fmt, io::ErrorKind, ops::Deref, str::FromStr};
use structopt::StructOpt;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq, Hash)]
pub struct CorsOrigin(String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AllowedOrigins(Vec<CorsOrigin>);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash, StructOpt)]
#[serde(deny_unknown_fields)]
#[structopt(rename_all = "kebab-case")]
pub struct Cors {
    /// REST_CORS - List of allowed origins, semicolon (;) separated. If none provided, echos request origin
    #[serde(default)]
    #[structopt(long, env = "ASMTPD_CORS_ALLOWED_ORIGINS", parse(try_from_str = parse_allowed_origins))]
    pub allowed_origins: Option<AllowedOrigins>,
    /// REST_CORS - Cache max age in seconds. If none provided, CORS responses won't be cached
    #[structopt(long, env = "ASMTPD_CORS_MAX_AGE_SECS")]
    pub max_age_secs: Option<u64>,
}

fn parse_allowed_origins(arg: &str) -> Result<AllowedOrigins, std::io::Error> {
    let mut res: Vec<CorsOrigin> = Vec::new();
    for origin_str in arg.split(';') {
        res.push(CorsOrigin::from_str(origin_str)?);
    }
    Ok(AllowedOrigins(res))
}

impl<'de> Deserialize<'de> for CorsOrigin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CorsOriginVisitor;
        impl<'de> Visitor<'de> for CorsOriginVisitor {
            type Value = CorsOrigin;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                write!(fmt, "an origin in format http[s]://example.com[:3000]",)
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                CorsOrigin::from_str(v).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(CorsOriginVisitor)
    }
}

impl FromStr for CorsOrigin {
    type Err = std::io::Error;

    fn from_str(origin: &str) -> Result<Self, Self::Err> {
        let uri = warp::http::uri::Uri::from_str(origin).map_err(|invalid_uri| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid uri: {}.\n{}", origin, invalid_uri),
            )
        })?;
        if let Some(s) = uri.scheme_str() {
            if s != "http" && s != "https" {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "Cors origin invalid schema {}, only [http] and [https] are supported: ",
                        uri.scheme_str().unwrap()
                    ),
                ));
            }
        } else {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "Cors origin missing schema, only [http] or [https] are supported",
            ));
        }

        if let Some(p) = uri.path_and_query() {
            if p.as_str() != "/" {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid value {} in cors schema.", p.as_str()),
                ));
            }
        }
        Ok(CorsOrigin(origin.trim_end_matches('/').to_owned()))
    }
}

impl AsRef<str> for CorsOrigin {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AllowedOrigins {
    type Target = Vec<CorsOrigin>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for AllowedOrigins {
    fn default() -> Self {
        AllowedOrigins(Vec::new())
    }
}
