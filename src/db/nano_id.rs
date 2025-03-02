use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize)]
#[sqlx(transparent)]
pub(crate) struct NanoId(String);

impl NanoId {
    pub(super) fn random() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NanoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for NanoId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<NanoId> for String {
    fn from(value: NanoId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct MaybeNanoId(pub(super) Option<NanoId>);

impl From<Option<String>> for MaybeNanoId {
    fn from(value: Option<String>) -> Self {
        Self(value.map(|id| id.into()))
    }
}

pub(crate) trait IntoOptString {
    fn into_opt_string(self) -> Option<String>;
}

impl IntoOptString for Option<NanoId> {
    fn into_opt_string(self) -> Option<String> {
        self.map(|id| id.into())
    }
}
