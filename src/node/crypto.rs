use sp_core::traits::CryptoExt;
use sp_core::{sr25519, ed25519, ecdsa};

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
