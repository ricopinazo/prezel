use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Admin,
    User,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TokenClaims {
    pub(crate) role: Role,
}

pub(crate) fn generate_token<T: Serialize>(claims: T, secret: &str) -> String {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .expect("Failed to encode claims")
}

pub(crate) fn decode_token<T: DeserializeOwned>(
    token: &str,
    secret: &str,
    validate_exp: bool,
) -> anyhow::Result<T> {
    let mut validation = Validation::new(Algorithm::HS256);
    if validate_exp {
        validation.set_required_spec_claims(&["exp"]);
    } else {
        validation.set_required_spec_claims::<String>(&[]);
    }
    let decoded = decode::<T>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )?;
    Ok(decoded.claims)
}

// TODO: wrap the above function instead
pub(crate) fn decode_auth_token(token: &str, secret: &str) -> anyhow::Result<TokenClaims> {
    let result = decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::new(Algorithm::HS256),
    );
    let decoded = result?;
    Ok(decoded.claims)
}
