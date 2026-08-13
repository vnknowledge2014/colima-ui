//! Telemetry consent state.
//!
//! Three states, persisted in the settings table:
//!   - unset   → never asked; the consent dialog should be shown once
//!   - declined → asked and refused; never ask again, never record
//!   - granted → asked and agreed; recording is allowed
//!
//! The default when unset is "do not record". Consent is opt-in, not opt-out.

use serde::{Deserialize, Serialize};

/// The settings key under which consent is stored.
const CONSENT_KEY: &str = "telemetry.consent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consent {
    /// Never asked. Recording is off; the dialog should be shown.
    Unset,
    /// Asked and refused. Recording is off; do not ask again.
    Declined,
    /// Asked and agreed. Recording is allowed.
    Granted,
}

impl Consent {
    /// Whether events may be recorded. Only an explicit grant allows it.
    pub fn allows_recording(self) -> bool {
        matches!(self, Consent::Granted)
    }

    /// Whether the consent dialog should still be shown. Only when never asked —
    /// a decline is final and must not be re-prompted.
    pub fn should_prompt(self) -> bool {
        matches!(self, Consent::Unset)
    }

    fn from_stored(value: Option<String>) -> Consent {
        match value.as_deref() {
            Some("granted") => Consent::Granted,
            Some("declined") => Consent::Declined,
            _ => Consent::Unset,
        }
    }

    fn as_stored(self) -> &'static str {
        match self {
            Consent::Granted => "granted",
            Consent::Declined => "declined",
            Consent::Unset => "unset",
        }
    }
}

/// Read the current consent from the settings table.
pub async fn current() -> Consent {
    let stored = crate::commands::knowledge_bank::get_setting(CONSENT_KEY.to_string())
        .await
        .unwrap_or(None);
    Consent::from_stored(stored)
}

/// Persist a consent decision.
pub async fn set(consent: Consent) -> Result<(), crate::error::ColimaError> {
    crate::commands::knowledge_bank::set_setting(CONSENT_KEY.to_string(), consent.as_stored().to_string())
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_does_not_record_but_prompts() {
        assert!(!Consent::Unset.allows_recording());
        assert!(Consent::Unset.should_prompt());
    }

    #[test]
    fn declined_neither_records_nor_reprompts() {
        assert!(!Consent::Declined.allows_recording());
        assert!(!Consent::Declined.should_prompt());
    }

    #[test]
    fn granted_records_and_does_not_reprompt() {
        assert!(Consent::Granted.allows_recording());
        assert!(!Consent::Granted.should_prompt());
    }

    #[test]
    fn unknown_stored_value_is_treated_as_unset() {
        assert_eq!(Consent::from_stored(Some("garbage".into())), Consent::Unset);
        assert_eq!(Consent::from_stored(None), Consent::Unset);
    }
}
