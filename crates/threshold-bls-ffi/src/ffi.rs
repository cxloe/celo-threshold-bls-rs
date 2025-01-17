//! # BLS12-377 FFI Bindings for Blind Threshold Signatures.
use rand_chacha::ChaChaRng;
use rand_core::{RngCore, SeedableRng};

use serde::{de::DeserializeOwned, Serialize};
use threshold_bls::{
    poly::{Idx as Index, Poly},
    sig::{
        BlindScheme, BlindThresholdScheme, Scheme, Share, SignatureScheme, ThresholdScheme, Token,
    },
};

use bls_crypto::ffi::Buffer;
use pyo3::prelude::*;


use crate::*;

///////////////////////////////////////////////////////////////////////////
// User -> Library
///////////////////////////////////////////////////////////////////////////

/// Given a message and a seed, it will blind it and return the blinded message
///
/// * message: A cleartext message which you want to blind
/// * seed: A 32 byte seed for randomness. You can get one securely via `crypto.randomBytes(32)`
/// * blinded_message_out : Pointer to the memory where the blinded message will be written to
/// * blinding_factor_out : Pointer to the object storing the blinding factor
///
/// The `blinding_factor_out` should be saved for unblinding any
/// signatures on `blinded_message_out`. It lives in-memory.
///
/// You should use `free_vec` to free `blinded_message_out` and `destroy_token` to destroy
/// `blinding_factor_out`.
///
/// # Safety
/// - If the same seed is used twice, the blinded result WILL be the same
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn blind(
    message: *const Buffer,
    seed: *const Buffer,
    blinded_message_out: *mut Buffer,
    blinding_factor_out: *mut *mut Token<PrivateKey>,
) -> bool {
    if message.is_null()
        || seed.is_null()
        || blinded_message_out.is_null()
        || blinding_factor_out.is_null()
    {
        return false;
    }

    // convert the seed to randomness
    let seed = <&[u8]>::from(unsafe { &*seed });
    let mut rng = get_rng(seed);

    // blind the message with this randomness
    let message = <&[u8]>::from(unsafe { &*message });
    let (blinding_factor, blinded_message_bytes) = SigScheme::blind_msg(message, &mut rng);

    unsafe { *blinded_message_out = Buffer::from(&blinded_message_bytes[..]) };
    std::mem::forget(blinded_message_bytes);
    unsafe { *blinding_factor_out = Box::into_raw(Box::new(blinding_factor)) };

    true
}

/// Given a blinded signature and a blinding_factor used for blinding, it returns the signature
/// unblinded
///
/// * blinded_signature: A message which has been blinded or a blind signature
/// * blinding_factor: The blinding_factor used to blind the message
/// * unblinded_signature: Pointer to the memory where the unblinded signature will be written to
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn unblind(
    blinded_signature: *const Buffer,
    blinding_factor: *const Token<PrivateKey>,
    unblinded_signature: *mut Buffer,
) -> bool {
    if blinded_signature.is_null() || blinding_factor.is_null() || unblinded_signature.is_null() {
        return false;
    }

    let blinded_signature = <&[u8]>::from(unsafe { &*blinded_signature });
    let blinding_factor = unsafe { &*blinding_factor };

    let sig = match SigScheme::unblind_sig(blinding_factor, blinded_signature) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *unblinded_signature = Buffer::from(&sig[..]) };
    std::mem::forget(sig);

    true
}

/// Verifies the signature after it has been unblinded. Users will call this on the
/// threshold signature against the full public key
///
/// * public_key: The public key used to sign the message
/// * message: The message which was signed
/// * signature: The signature which was produced on the message
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn verify(
    public_key: *const PublicKey,
    message: *const Buffer,
    signature: *const Buffer,
) -> bool {
    if public_key.is_null() || message.is_null() || signature.is_null() {
        return false;
    }

    let public_key = unsafe { &*public_key };
    let message = <&[u8]>::from(unsafe { &*message });

    // checks the signature on the message hash
    let signature = <&[u8]>::from(unsafe { &*signature });
    SigScheme::verify(public_key, message, signature).is_ok()
}

///////////////////////////////////////////////////////////////////////////
// Service -> Library
///////////////////////////////////////////////////////////////////////////

/// Signs the message with the provided private key and returns the signature
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn sign(
    private_key: *const PrivateKey,
    message: *const Buffer,
    signature: *mut Buffer,
) -> bool {
    if private_key.is_null() || message.is_null() || signature.is_null() {
        return false;
    }

    let private_key = unsafe { &*private_key };
    let message = <&[u8]>::from(unsafe { &*message });

    let sig = match SigScheme::sign(private_key, message) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *signature = Buffer::from(&sig[..]) };
    std::mem::forget(sig);

    true
}

/// Signs a *blinded* message with the provided private key and returns the signature
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn sign_blinded_message(
    private_key: *const PrivateKey,
    message: *const Buffer,
    signature: *mut Buffer,
) -> bool {
    if private_key.is_null() || message.is_null() || signature.is_null() {
        return false;
    }

    let private_key = unsafe { &*private_key };
    let message = <&[u8]>::from(unsafe { &*message });

    let sig = match SigScheme::blind_sign(private_key, message) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *signature = Buffer::from(&sig[..]) };
    std::mem::forget(sig);

    true
}

/// Signs the message with the provided **share** of the private key and returns the **partial**
/// signature.
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn partial_sign(
    share: *const Share<PrivateKey>,
    message: *const Buffer,
    signature: *mut Buffer,
) -> bool {
    if share.is_null() || message.is_null() || signature.is_null() {
        return false;
    }

    let share = unsafe { &*share };
    let message = unsafe { &*message };
    let sig = match SigScheme::partial_sign(share, <&[u8]>::from(message)) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *signature = Buffer::from(&sig[..]) };
    std::mem::forget(sig);

    true
}

/// Signs a *blinded* message with the provided *share* of the private key and returns the
/// *partial blind* signature.
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn partial_sign_blinded_message(
    share: *const Share<PrivateKey>,
    blinded_message: *const Buffer,
    signature: *mut Buffer,
) -> bool {
    if share.is_null() || blinded_message.is_null() || signature.is_null() {
        return false;
    }

    let share = unsafe { &*share };
    let message = unsafe { &*blinded_message };
    let sig = match SigScheme::sign_blind_partial(share, <&[u8]>::from(message)) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *signature = Buffer::from(&sig[..]) };
    std::mem::forget(sig);

    true
}

///////////////////////////////////////////////////////////////////////////
// Combiner -> Library
///////////////////////////////////////////////////////////////////////////

/// Verifies a partial signature against the public key corresponding to the secret shared
/// polynomial.
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn partial_verify(
    // TODO: The polynomial does not have a constant length type. Is it safe to not
    // pass any length parameter?
    polynomial: *const Poly<PublicKey>,
    blinded_message: *const Buffer,
    signature: *const Buffer,
) -> bool {
    if polynomial.is_null() || blinded_message.is_null() || signature.is_null() {
        return false;
    }

    let polynomial = unsafe { &*polynomial };
    let blinded_message = <&[u8]>::from(unsafe { &*blinded_message });
    let signature = <&[u8]>::from(unsafe { &*signature });

    SigScheme::partial_verify(polynomial, blinded_message, signature).is_ok()
}

/// Verifies a partial *blinded* signature against the public key corresponding to the secret shared
/// polynomial.
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
#[no_mangle]
pub unsafe extern "C" fn partial_verify_blind_signature(
    // TODO: The polynomial does not have a constant length type. Is it safe to not
    // pass any length parameter?
    polynomial: *const Poly<PublicKey>,
    blinded_message: *const Buffer,
    signature: *const Buffer,
) -> bool {
    if polynomial.is_null() || blinded_message.is_null() || signature.is_null() {
        return false;
    }

    let polynomial = unsafe { &*polynomial };
    let blinded_message = <&[u8]>::from(unsafe { &*blinded_message });
    let signature = <&[u8]>::from(unsafe { &*signature });

    SigScheme::verify_blind_partial(polynomial, blinded_message, signature).is_ok()
}

#[pyclass]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyBuffer(*const Buffer);
#[pyclass]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyMutBuffer(*mut Buffer);

// FIXME: this is bad and evil.
// This should not be written, we will not writw this.meme
// This is not a place of honor
// Whats a little undefined behavior among friends?
unsafe impl Sync for PyBuffer{}
unsafe impl Send for PyBuffer{}
unsafe impl Sync for PyMutBuffer{}
unsafe impl Send for PyMutBuffer{}

/// Combines a flattened vector of partial signatures to a single threshold signature
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
/// - This function does not check if the signatures are valid!
///
/// Returns true if successful, otherwise false.
#[no_mangle]
#[pyfunction]
pub unsafe extern "C" fn combine(
    threshold: usize,
    signatures: PyBuffer,
    asig: PyMutBuffer,
) -> bool {
    if signatures.0.is_null() || asig.0.is_null() {
        return false; 
    }

    // split the flattened vector to a Vec<Vec<u8>> where each element is a serialized signature
    let signatures = <&[u8]>::from(unsafe { &*signatures.0 });
    let sigs = signatures
        .chunks(PARTIAL_SIG_LENGTH)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<u8>>>();

    let signature = match SigScheme::aggregate(threshold, &sigs) {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { *asig.0 = Buffer::from(&signature[..]) };
    std::mem::forget(signature);

    true
}

#[pymodule]
fn blind_threshold_bls(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(combine, m)?)?;
    Ok(())
}

///////////////////////////////////////////////////////////////////////////
// Serialization
///////////////////////////////////////////////////////////////////////////

#[no_mangle]
/// Deserializes a public key from the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn deserialize_pubkey(
    pubkey_buf: *const u8,
    pubkey: *mut *mut PublicKey,
) -> bool {
    deserialize(pubkey_buf, PUBKEY_LEN, pubkey)
}

#[no_mangle]
/// Deserializes a private key from the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn deserialize_privkey(
    privkey_buf: *const u8,
    privkey: *mut *mut PrivateKey,
) -> bool {
    deserialize(privkey_buf, PRIVKEY_LEN, privkey)
}

#[no_mangle]
/// Deserializes a signature from the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn deserialize_sig(sig_buf: *const u8, sig: *mut *mut Signature) -> bool {
    deserialize(sig_buf, SIGNATURE_LEN, sig)
}

#[no_mangle]
/// Serializes a public key to the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn serialize_pubkey(
    pubkey: *const PublicKey,
    pubkey_buf: *mut *mut u8,
) -> bool {
    serialize(pubkey, pubkey_buf)
}

#[no_mangle]
/// Serializes a private key to the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn serialize_privkey(
    privkey: *const PrivateKey,
    privkey_buf: *mut *mut u8,
) -> bool {
    serialize(privkey, privkey_buf)
}

#[no_mangle]
/// Serializes a signature to the provided buffer
///
/// # Safety
/// - **This function will dereference the provided pointers. If any invalid pointers are passed
///     then the software will crash**.
/// - If NULL pointers are passed, the function will return false
///
/// Returns true if successful, otherwise false.
pub unsafe extern "C" fn serialize_sig(sig: *const Signature, sig_buf: *mut *mut u8) -> bool {
    serialize(sig, sig_buf)
}

fn deserialize<T: DeserializeOwned>(in_buf: *const u8, len: usize, out: *mut *mut T) -> bool {
    let buf = unsafe { std::slice::from_raw_parts(in_buf, len) };

    let obj = if let Ok(res) = bincode::deserialize(buf) {
        res
    } else {
        return false;
    };

    unsafe { *out = Box::into_raw(Box::new(obj)) };

    true
}

fn serialize<T: Serialize>(in_obj: *const T, out_bytes: *mut *mut u8) -> bool {
    let obj = unsafe { &*in_obj };
    let mut marshalled = if let Ok(res) = bincode::serialize(obj) {
        res
    } else {
        return false;
    };

    unsafe {
        *out_bytes = marshalled.as_mut_ptr();
    };
    std::mem::forget(marshalled);

    true
}

#[no_mangle]
/// Frees the memory allocated for the blinding factor
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_token(token: *mut Token<PrivateKey>) {
    Box::from_raw(token);
}

#[no_mangle]
/// Frees the memory allocated for the threshold keys helper
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_keys(keys: *mut Keys) {
    Box::from_raw(keys);
}

#[no_mangle]
/// Frees the memory allocated for the keypair helper
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_keypair(keypair: *mut Keypair) {
    Box::from_raw(keypair);
}

#[no_mangle]
/// Frees the memory allocated for a private key
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_privkey(private_key: *mut PrivateKey) {
    Box::from_raw(private_key);
}

#[no_mangle]
/// Frees the memory allocated for a vector
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn free_vector(bytes: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(bytes, len as usize, len as usize) });
}

#[no_mangle]
/// Frees the memory allocated for a public key
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_pubkey(public_key: *mut PublicKey) {
    Box::from_raw(public_key);
}

#[no_mangle]
/// Frees the memory allocated for a signature
///
/// # Safety
///
/// The pointer must point to a valid instance of the data type
pub unsafe extern "C" fn destroy_sig(signature: *mut Signature) {
    Box::from_raw(signature);
}

///////////////////////////////////////////////////////////////////////////
// Helpers
//
// These should be exposed behind a helper module and should not be made part
// of the public API
///////////////////////////////////////////////////////////////////////////

/// Generates a t-of-n polynomial and private key shares
///
/// The return value should be destroyed with `destroy_keys`.
///
/// # Safety
///
/// WARNING: This is a helper function for local testing of the library. Do not use
/// in production, unless you trust the person that generated the keys.
///
/// The seed MUST be at least 32 bytes long
#[no_mangle]
pub unsafe extern "C" fn threshold_keygen(
    n: usize,
    t: usize,
    seed: *const Buffer,
    keys: *mut *mut Keys,
) {
    let seed = <&[u8]>::from(unsafe { &*seed });
    let mut rng = get_rng(seed);
    let private = Poly::<PrivateKey>::new_from(t - 1, &mut rng);
    let shares = (0..n)
        .map(|i| private.eval(i as Index))
        .map(|e| Share {
            index: e.index,
            private: e.value,
        })
        .collect();
    let polynomial: Poly<PublicKey> = private.commit();
    let threshold_public_key = polynomial.public_key().clone();

    let keys_local = Keys {
        shares,
        polynomial,
        threshold_public_key,
        t,
        n,
    };

    unsafe {
        *keys = Box::into_raw(Box::new(keys_local));
    };
}

/// Generates a single private key from the provided seed.
///
/// The return value should be destroyed with `destroy_keypair`.
///
/// # Safety
///
/// The seed MUST be at least 32 bytes long
#[no_mangle]
pub unsafe extern "C" fn keygen(seed: *const Buffer, keypair: *mut *mut Keypair) {
    let seed = <&[u8]>::from(unsafe { &*seed });
    let mut rng = get_rng(seed);
    let (private, public) = SigScheme::keypair(&mut rng);
    let keypair_local = Keypair { private, public };
    unsafe { *keypair = Box::into_raw(Box::new(keypair_local)) };
}

/// Gets the `index`'th share corresponding to the provided `Keys` pointer
///
/// The return value should be destroyed with `destroy_keys`.
///
/// # Safety
///
/// WARNING: This is a helper function for local testing of the library. Do not use
/// in production, unless you trust the person that generated the keys.
///
/// The seed MUST be at least 32 bytes long
#[no_mangle]
pub unsafe extern "C" fn share_ptr(keys: *const Keys, index: usize) -> *const Share<PrivateKey> {
    &(*keys).shares[index] as *const Share<PrivateKey>
}

/// Gets the number of shares corresponding to the provided `Keys` pointer
///
/// # Safety
/// The provided pointer will be dereferenced, so there must be valid data beneath it
#[no_mangle]
pub unsafe extern "C" fn num_shares(keys: *const Keys) -> usize {
    (*keys).shares.len()
}

/// Gets a pointer to the polynomial corresponding to the provided `Keys` pointer
///
/// # Safety
/// The provided pointer will be dereferenced, so there must be valid data beneath it
#[no_mangle]
pub unsafe extern "C" fn polynomial_ptr(keys: *const Keys) -> *const Poly<PublicKey> {
    &(*keys).polynomial as *const Poly<PublicKey>
}

/// Gets a pointer to the threshold public key corresponding to the provided `Keys` pointer
///
/// # Safety
/// The provided pointer will be dereferenced, so there must be valid data beneath it
#[no_mangle]
pub unsafe extern "C" fn threshold_public_key_ptr(keys: *const Keys) -> *const PublicKey {
    &(*keys).threshold_public_key as *const PublicKey
}

/// Gets a pointer to the public key corresponding to the provided `KeyPair` pointer
///
/// # Safety
/// The provided pointer will be dereferenced, so there must be valid data beneath it
#[no_mangle]
pub unsafe extern "C" fn public_key_ptr(keypair: *const Keypair) -> *const PublicKey {
    &(*keypair).public as *const PublicKey
}

/// Gets a pointer to the private key corresponding to the provided `KeyPair` pointer
///
/// # Safety
/// The provided pointer will be dereferenced, so there must be valid data beneath it
#[no_mangle]
pub unsafe extern "C" fn private_key_ptr(keypair: *const Keypair) -> *const PrivateKey {
    &(*keypair).private as *const PrivateKey
}

/// T-of-n threshold key parameters
#[derive(Debug, Clone)]
pub struct Keys {
    shares: Vec<Share<PrivateKey>>,
    polynomial: Poly<PublicKey>,
    threshold_public_key: PublicKey,
    pub t: usize,
    pub n: usize,
}

#[derive(Clone)]
#[repr(C)]
/// A BLS12-377 Keypair
pub struct Keypair {
    /// The private key
    private: PrivateKey,
    /// The public key
    public: PublicKey,
}

fn get_rng(digest: &[u8]) -> impl RngCore {
    let seed = from_slice(digest);
    ChaChaRng::from_seed(seed)
}

fn from_slice(bytes: &[u8]) -> [u8; 32] {
    let mut array = [0; 32];
    let bytes = &bytes[..array.len()]; // panics if not enough data
    array.copy_from_slice(bytes);
    array
}

// The general pattern in these FFI tests is:
// 1. create a MaybeUninit pointer
// 2. pass it to the function
// 3. assert that the function call was successful
// 4. assume the pointer is now initialized
#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn threshold_verify_ffi() {
        threshold_verify_ffi_should_blind(true);
        threshold_verify_ffi_should_blind(false);
    }

    fn threshold_verify_ffi_should_blind(should_blind: bool) {
        let seed = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let msg = vec![1u8, 2, 3, 4, 6];
        let user_seed = &b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"[..];
        let empty_token = Token::new();
        let partial_sign_fn = if should_blind {
            partial_sign_blinded_message
        } else {
            partial_sign
        };
        let partial_verify_fn = if should_blind {
            partial_verify_blind_signature
        } else {
            partial_verify
        };

        let (n, t) = (5, 3);
        let mut keys = MaybeUninit::<*mut Keys>::uninit();
        unsafe { threshold_keygen(n, t, &Buffer::from(&seed[..]), keys.as_mut_ptr()) };
        let keys = unsafe { &*keys.assume_init() };

        let (message_to_sign, blinding_factor) = if should_blind {
            let mut blinded_message = MaybeUninit::<Buffer>::uninit();
            let mut blinding_factor = MaybeUninit::<*mut Token<PrivateKey>>::uninit();
            unsafe {
                blind(
                    &Buffer::from(msg.as_ref()),
                    &Buffer::from(user_seed),
                    blinded_message.as_mut_ptr(),
                    blinding_factor.as_mut_ptr(),
                )
            };
            let blinded_message = unsafe { blinded_message.assume_init() };
            let blinding_factor = unsafe { &*blinding_factor.assume_init() };

            (blinded_message, blinding_factor)
        } else {
            (Buffer::from(&msg[..]), &empty_token)
        };

        // 2. partially sign the blinded message
        let mut sigs = Vec::new();
        for i in 0..t {
            let mut partial_sig = MaybeUninit::<Buffer>::uninit();
            let ret = unsafe {
                partial_sign_fn(
                    share_ptr(keys, i),
                    &message_to_sign,
                    partial_sig.as_mut_ptr(),
                )
            };
            assert!(ret);

            let partial_sig = unsafe { partial_sig.assume_init() };
            sigs.push(partial_sig);
        }

        // 3. verify the partial signatures & concatenate them
        let public_key = unsafe { polynomial_ptr(keys) };
        let mut concatenated = Vec::new();
        for sig in &sigs {
            let sig_slice = <&[u8]>::from(sig);
            concatenated.extend_from_slice(sig_slice);
            let ret = unsafe { partial_verify_fn(public_key, &message_to_sign, sig) };
            assert!(ret);
        }
        let concatenated = Buffer::from(&concatenated[..]);

        // 4. generate the threshold signature
        let mut asig = MaybeUninit::<Buffer>::uninit();
        let ret = unsafe { combine(t, PyBuffer(&concatenated), PyMutBuffer(asig.as_mut_ptr())) };
        assert!(ret);
        let asig = unsafe { asig.assume_init() };

        // 5. unblind the threshold signature
        let asig = if should_blind {
            let mut unblinded = MaybeUninit::<Buffer>::uninit();
            let ret = unsafe { unblind(&asig, blinding_factor, unblinded.as_mut_ptr()) };
            assert!(ret);
            unsafe { unblinded.assume_init() }
        } else {
            asig
        };

        // 6. verify the threshold signature against the public key
        let ret = unsafe {
            verify(
                threshold_public_key_ptr(keys),
                &Buffer::from(&msg[..]),
                &asig,
            )
        };
        assert!(ret);
    }

    #[test]
    fn verify_ffi() {
        verify_ffi_should_blind(true);
        verify_ffi_should_blind(false);
    }

    fn verify_ffi_should_blind(should_blind: bool) {
        let seed = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let msg = vec![1u8, 2, 3, 4, 6];
        let user_seed = &b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"[..];
        let empty_token = Token::new();

        let sign_fn = if should_blind {
            sign_blinded_message
        } else {
            sign
        };

        let mut keypair = MaybeUninit::<*mut Keypair>::uninit();
        unsafe { keygen(&Buffer::from(&seed[..]), keypair.as_mut_ptr()) };
        let keypair = unsafe { &*keypair.assume_init() };

        let (message_to_sign, blinding_factor) = if should_blind {
            let mut blinded_message = MaybeUninit::<Buffer>::uninit();
            let mut blinding_factor = MaybeUninit::<*mut Token<PrivateKey>>::uninit();
            unsafe {
                blind(
                    &Buffer::from(msg.as_ref()),
                    &Buffer::from(user_seed),
                    blinded_message.as_mut_ptr(),
                    blinding_factor.as_mut_ptr(),
                )
            };
            let blinded_message = unsafe { blinded_message.assume_init() };
            let blinding_factor = unsafe { &*blinding_factor.assume_init() };

            (blinded_message, blinding_factor)
        } else {
            (Buffer::from(&msg[..]), &empty_token)
        };

        let mut sig = MaybeUninit::<Buffer>::uninit();
        let ret = unsafe { sign_fn(private_key_ptr(keypair), &message_to_sign, sig.as_mut_ptr()) };
        assert!(ret);
        let sig = unsafe { sig.assume_init() };

        let sig = if should_blind {
            let mut unblinded = MaybeUninit::<Buffer>::uninit();
            let ret = unsafe { unblind(&sig, blinding_factor, unblinded.as_mut_ptr()) };
            assert!(ret);

            unsafe { unblinded.assume_init() }
        } else {
            sig
        };

        let ret = unsafe { verify(public_key_ptr(keypair), &Buffer::from(&msg[..]), &sig) };
        assert!(ret);
    }

    #[test]
    fn private_key_serialization() {
        let seed = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let mut keypair = MaybeUninit::<*mut Keypair>::uninit();
        unsafe { keygen(&Buffer::from(&seed[..]), keypair.as_mut_ptr()) };
        let keypair = unsafe { &*keypair.assume_init() };

        let private_key_ptr = unsafe { private_key_ptr(keypair) };
        let private_key = unsafe { &*private_key_ptr };
        let marshalled = bincode::serialize(private_key).unwrap();

        let mut privkey_buf = MaybeUninit::<*mut u8>::uninit();

        let ret = unsafe { serialize_privkey(private_key_ptr, privkey_buf.as_mut_ptr()) };
        assert!(ret);

        let privkey_buf = unsafe { privkey_buf.assume_init() };
        let message = unsafe { std::slice::from_raw_parts(privkey_buf, PRIVKEY_LEN) };
        assert_eq!(marshalled, message);

        let unmarshalled: PrivateKey = bincode::deserialize(message).unwrap();
        assert_eq!(&unmarshalled, private_key);

        let mut de = MaybeUninit::<*mut PrivateKey>::uninit();
        let ret = unsafe { deserialize_privkey(&message[0] as *const u8, de.as_mut_ptr()) };
        assert!(ret);
        let de = unsafe { de.assume_init() };

        assert_eq!(private_key, unsafe { &*de });
    }

    #[test]
    fn public_key_serialization() {
        let seed = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let mut keypair = MaybeUninit::<*mut Keypair>::uninit();
        unsafe { keygen(&Buffer::from(&seed[..]), keypair.as_mut_ptr()) };
        let keypair = unsafe { &*keypair.assume_init() };

        let public_key_ptr = unsafe { public_key_ptr(keypair) };
        let public_key = unsafe { &*public_key_ptr };

        let marshalled = bincode::serialize(public_key).unwrap();

        let mut pubkey_buf = MaybeUninit::<*mut u8>::uninit();

        let ret = unsafe { serialize_pubkey(public_key_ptr, pubkey_buf.as_mut_ptr()) };
        assert!(ret);

        let pubkey_buf = unsafe { pubkey_buf.assume_init() };
        // the serialized result
        let message = unsafe { std::slice::from_raw_parts(pubkey_buf, PUBKEY_LEN) };
        assert_eq!(marshalled, message);

        let unmarshalled: PublicKey = bincode::deserialize(message).unwrap();
        assert_eq!(&unmarshalled, public_key);

        let mut de = MaybeUninit::<*mut PublicKey>::uninit();
        let ret = unsafe { deserialize_pubkey(&message[0] as *const u8, de.as_mut_ptr()) };
        assert!(ret);
        let de = unsafe { de.assume_init() };

        assert_eq!(public_key, unsafe { &*de });
    }
}
