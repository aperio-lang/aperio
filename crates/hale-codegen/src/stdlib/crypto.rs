//! `std::crypto::*` path-call lowering.

use hale_syntax::ast::Expr;
use inkwell::values::BasicValueEnum;

use crate::codegen::{CodegenError, CodegenTy, Cx, Scope};

pub(crate) trait CryptoStdlib<'ctx> {
    fn lower_std_crypto_sha1(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError>;

    fn lower_std_crypto_sha256(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError>;

    fn lower_std_crypto_hmac_sha256(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError>;

    fn lower_std_crypto_crc32(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError>;
}

impl<'ctx, 'p> CryptoStdlib<'ctx> for Cx<'ctx, 'p> {
    /// ws-echo `sha1-base64-missing`: lower
    /// `std::crypto::sha1(b: Bytes) -> Bytes`. Returns a 20-byte
    /// digest. Stand-alone implementation in the C runtime per
    /// RFC 3174 — no OpenSSL dependency. Anchored in the
    /// program-lifetime payload arena.
    fn lower_std_crypto_sha1(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError> {
        if args.len() != 1 {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::sha1 takes 1 arg (b), got {}",
                args.len()
            )));
        }
        let (b_val, b_ty) = self.lower_expr(&args[0], scope)?;
        if !matches!(b_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::sha1: b must be Bytes, got {:?}",
                b_ty
            )));
        }
        let b_val = self.unpack_view_if_needed(b_val, &b_ty)?;
        let f = self
            .module
            .get_function("lotus_crypto_sha1")
            .expect("lotus_crypto_sha1 declared");
        let call = self
            .builder
            .build_call(f, &[b_val.into()], "sha1.ret")
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?;
        let ptr = call
            .try_as_basic_value()
            .left()
            .expect("returns ptr");
        Ok((ptr, CodegenTy::Bytes))
    }

    /// C3 (pond follow-up): lower
    /// `std::crypto::sha256(b: Bytes) -> Bytes`. Returns a 32-byte
    /// digest per FIPS 180-4. Stand-alone implementation in the
    /// C runtime — no libcrypto dependency. Anchored in the
    /// program-lifetime payload arena. Drops pond/crypto's
    /// ~140-line pure-Hale O(N²) sha256.hl.
    fn lower_std_crypto_sha256(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError> {
        if args.len() != 1 {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::sha256 takes 1 arg (b), got {}",
                args.len()
            )));
        }
        let (b_val, b_ty) = self.lower_expr(&args[0], scope)?;
        if !matches!(b_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::sha256: b must be Bytes, got {:?}",
                b_ty
            )));
        }
        let b_val = self.unpack_view_if_needed(b_val, &b_ty)?;
        let f = self
            .module
            .get_function("lotus_crypto_sha256")
            .expect("lotus_crypto_sha256 declared");
        let call = self
            .builder
            .build_call(f, &[b_val.into()], "sha256.ret")
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?;
        let ptr = call
            .try_as_basic_value()
            .left()
            .expect("returns ptr");
        Ok((ptr, CodegenTy::Bytes))
    }

    /// C3 (pond follow-up): lower
    /// `std::crypto::hmac_sha256(key: Bytes, msg: Bytes) -> Bytes`.
    /// Returns the 32-byte HMAC tag per RFC 2104. Anchored in
    /// the payload arena. Drops pond/crypto's hmac.hl wrapper.
    fn lower_std_crypto_hmac_sha256(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError> {
        if args.len() != 2 {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::hmac_sha256 takes 2 args (key, msg), got {}",
                args.len()
            )));
        }
        let (key_val, key_ty) = self.lower_expr(&args[0], scope)?;
        if !matches!(key_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::hmac_sha256: key must be Bytes, got {:?}",
                key_ty
            )));
        }
        let key_val = self.unpack_view_if_needed(key_val, &key_ty)?;
        let (msg_val, msg_ty) = self.lower_expr(&args[1], scope)?;
        if !matches!(msg_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::hmac_sha256: msg must be Bytes, got {:?}",
                msg_ty
            )));
        }
        let msg_val = self.unpack_view_if_needed(msg_val, &msg_ty)?;
        let f = self
            .module
            .get_function("lotus_crypto_hmac_sha256")
            .expect("lotus_crypto_hmac_sha256 declared");
        let call = self
            .builder
            .build_call(
                f,
                &[key_val.into(), msg_val.into()],
                "hmac_sha256.ret",
            )
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?;
        let ptr = call
            .try_as_basic_value()
            .left()
            .expect("returns ptr");
        Ok((ptr, CodegenTy::Bytes))
    }

    /// 2026-05-27: lower
    /// `std::crypto::crc32(b: Bytes) -> Int`. IEEE 802.3
    /// reversed polynomial (`0xEDB88320`), init `0xFFFFFFFF`,
    /// final XOR `0xFFFFFFFF` — the variant zlib's `crc32()`
    /// and Python's `binascii.crc32` return. Returns the
    /// 4-byte checksum as `Int` (caller casts/compares as
    /// needed). Non-fallible, no arena allocation.
    fn lower_std_crypto_crc32(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError> {
        if args.len() != 1 {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::crc32 takes 1 arg (b), got {}",
                args.len()
            )));
        }
        let (b_val, b_ty) = self.lower_expr(&args[0], scope)?;
        if !matches!(b_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::crypto::crc32: b must be Bytes, got {:?}",
                b_ty
            )));
        }
        let b_val = self.unpack_view_if_needed(b_val, &b_ty)?;
        let f = self
            .module
            .get_function("lotus_crypto_crc32")
            .expect("lotus_crypto_crc32 declared");
        let call = self
            .builder
            .build_call(f, &[b_val.into()], "crc32.ret")
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?;
        let iv = call
            .try_as_basic_value()
            .left()
            .expect("returns i64");
        Ok((iv, CodegenTy::Int))
    }
}
