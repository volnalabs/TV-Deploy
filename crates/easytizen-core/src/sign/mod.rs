//! Signing a `.wgt` without the Tizen SDK.
//!
//! A Samsung TV accepts a widget only when it carries two W3C XML-DSig documents:
//! `author-signature.xml` (who built it) and `signature1.xml` (the distributor certificate
//! that names the TVs allowed to run it). This module produces both from PKCS#12 keystores,
//! using the same algorithms the current Tizen tools use (RSA-SHA512, SHA-512 digests,
//! exclusive C14N for `SignedInfo`, C14N 1.1 for the `#prop` object).
//!
//! The output is verified in the test-suite by an independent implementation (`xmlsec1`).
//! Acceptance by a physical TV still has to be confirmed — see docs/KNOWN-LIMITATIONS.md.

pub mod identity;
pub mod wgt;

pub use identity::{CertInfo, IdentityError, SigningIdentity};
pub use wgt::{sign_wgt, SignError};

#[cfg(all(test, unix))]
mod interop_tests;

#[cfg(all(test, unix))]
pub(crate) fn openssl3_for_tests() -> bool {
    interop_tests::openssl3()
}
