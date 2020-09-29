use sp_core::traits::{CryptoExt, CryptoExtension};
use sp_core::{sr25519, ed25519, ecdsa, offchain};
use sc_client_api::execution_extensions::ExtensionsFactory;
use sp_externalities::Extensions;

/// An impplementation of CryptoExt that does no verification.
pub struct NonVerifyingCrypto;

impl CryptoExt for NonVerifyingCrypto {
    fn sr25519_verify(&self, _sig: &sr25519::Signature, _msg: &[u8], _pubkey: &sr25519::Public) -> bool {
        true
    }

    fn sr25519_verify_deprecated(&self, _sig: &sr25519::Signature, _msg: &[u8], _pubkey: &sr25519::Public) -> bool {
        true
    }

    fn ed25519_verify(&self, _sig: &ed25519::Signature, _msg: &[u8], _pubkey: &ed25519::Public) -> bool {
        true
    }

    fn ecdsa_verify(&self, _sig: &ecdsa::Signature, _msg: &[u8], _pubkey: &ecdsa::Public) -> bool {
        true
    }
}

/// Custom implementation of extensions factory.
pub struct ExtensionFactory;

impl ExtensionsFactory for ExtensionFactory {
    fn extensions_for(&self, _capabilities: offchain::Capabilities) -> Extensions {
        let mut extensions = Extensions::new();
        // register the extension, we would like for the chain to not have any signature verification.
        extensions.register(CryptoExtension(Box::new(NonVerifyingCrypto)));
        extensions
    }
}
