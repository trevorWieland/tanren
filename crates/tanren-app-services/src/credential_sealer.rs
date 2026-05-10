use tanren_configuration_secrets::{CredentialSealingFailure, CredentialValueSealer};
use tanren_store::StoreError;

use crate::AppServiceError;

#[derive(Debug, Clone)]
pub(crate) enum CredentialSealerState {
    Ready(CredentialValueSealer),
    Unavailable(CredentialSealingFailure),
}

impl CredentialSealerState {
    pub(crate) fn from_result(
        result: Result<CredentialValueSealer, CredentialSealingFailure>,
    ) -> Self {
        match result {
            Ok(sealer) => Self::Ready(sealer),
            Err(err) => Self::Unavailable(err),
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::from_result(CredentialValueSealer::from_env())
    }

    pub(crate) fn as_result(&self) -> Result<&CredentialValueSealer, CredentialSealingFailure> {
        match self {
            Self::Ready(sealer) => Ok(sealer),
            Self::Unavailable(err) => Err(err.clone()),
        }
    }
}

pub(crate) fn map_credential_sealer_error(err: &CredentialSealingFailure) -> AppServiceError {
    AppServiceError::Store(StoreError::CredentialEncryption {
        detail: err.to_string(),
    })
}
