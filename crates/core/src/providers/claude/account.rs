// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! serde skips every key but the account id and the organisation name, so no email or name is read.

use crate::account::{fnv, SignedIn};
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    #[serde(rename = "oauthAccount")]
    oauth: Option<Oauth>,
}

#[derive(Deserialize)]
struct Oauth {
    #[serde(rename = "accountUuid")]
    account_uuid: Option<String>,
    #[serde(rename = "organizationName")]
    organization_name: Option<String>,
}

pub fn signed_in(bytes: &[u8]) -> Option<SignedIn> {
    let c: Config = serde_json::from_slice(bytes).ok()?;
    let o = c.oauth?;
    let uuid = o.account_uuid.filter(|u| !u.is_empty())?;
    Some(SignedIn {
        key: format!("{:016x}", fnv(uuid.as_bytes())),
        label: o.organization_name.unwrap_or_default().trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape of a real ~/.claude.json oauthAccount (values invented): personal fields must not reach the result.
    #[test]
    fn keeps_hash_and_organisation_only() {
        let json = br#"{"numStartups": 3, "oauthAccount": {"accountUuid": "0b6f-aa", "emailAddress": "me@example.com",
            "organizationUuid": "o-1", "organizationName": "Acme Pro", "displayName": "Me", "billingType": "stripe_subscription"}}"#;
        let s = signed_in(json).unwrap();
        assert_eq!(s.label, "Acme Pro");
        assert_eq!(s.key.len(), 16);
        let dbg = format!("{s:?}");
        assert!(
            !dbg.contains("example.com") && !dbg.contains("0b6f-aa") && !dbg.contains("\"Me\"")
        );
        let other = signed_in(br#"{"oauthAccount": {"accountUuid": "0b6f-ab"}}"#).unwrap();
        assert_ne!(other.key, s.key);
        assert_eq!(other.label, "");
        assert!(signed_in(br#"{"primaryApiKey": "x"}"#).is_none());
    }
}
