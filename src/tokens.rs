use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) enum Role {
    owner,
    member,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TokenClaims {
    pub(crate) role: Role,
}

pub(crate) fn generate_token(claims: TokenClaims, secret: &str) -> String {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .expect("Failed to encode claims")
}

pub(crate) fn decode_token(token: &str, secret: &str) -> anyhow::Result<TokenClaims> {
    let decoded = decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(decoded.claims)
}
