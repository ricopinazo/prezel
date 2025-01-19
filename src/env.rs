use std::{collections::HashMap, ops::Add};

use crate::db::EnvVar;

#[derive(Debug, Clone, Default)]
pub(crate) struct EnvVars(HashMap<String, String>);

impl EnvVars {
    pub(crate) fn new(env: &[(&str, &str)]) -> Self {
        env.into()
    }

    pub(crate) fn empty() -> Self {
        Self(Default::default())
    }
}

impl IntoIterator for EnvVars {
    type Item = String;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let vec: Vec<_> = self.into();
        vec.into_iter()
    }
}

impl From<EnvVars> for HashMap<String, String> {
    fn from(value: EnvVars) -> Self {
        value.0
    }
}

impl From<Vec<EnvVar>> for EnvVars {
    fn from(value: Vec<EnvVar>) -> Self {
        let map = value.into_iter().map(|env| (env.name, env.value));
        Self(map.collect())
    }
}

impl From<HashMap<String, String>> for EnvVars {
    fn from(value: HashMap<String, String>) -> Self {
        Self(value)
    }
}

impl From<&[(&str, &str)]> for EnvVars {
    fn from(value: &[(&str, &str)]) -> Self {
        Self(
            value
                .into_iter()
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
        )
    }
}

impl From<EnvVars> for Vec<String> {
    fn from(value: EnvVars) -> Self {
        value
            .0
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect()
    }
}

impl Add for EnvVars {
    type Output = Self;

    // TODO: make sure this overwrites the conflicting values on the old one
    fn add(self, other: Self) -> Self {
        Self(self.0.into_iter().chain(other.0).collect())
    }
}
