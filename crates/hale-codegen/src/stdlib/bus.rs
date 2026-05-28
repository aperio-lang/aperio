//! `std::bus::*` path-call lowering for `__local_dispatch`. The
//! rest of the bus surface (publish, subscribe, wire) lives in
//! `crate::codegen` (Round 3 target).

use hale_syntax::ast::Expr;
use inkwell::values::BasicValueEnum;

use crate::codegen::{CodegenError, CodegenTy, Cx, Scope};

pub(crate) trait BusStdlib<'ctx> {
    fn lower_std_bus_local_dispatch(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError>;
}

impl<'ctx, 'p> BusStdlib<'ctx> for Cx<'ctx, 'p> {
    /// m105: lower `std::bus::__local_dispatch(subject: String,
    /// wire_bytes: Bytes) -> ()`. Hands wire bytes (received by an
    /// adapter from its transport) through the subject's registered
    /// deserialize fn into the local handler set. The Hale surface
    /// is the inbound counterpart to an adapter's outbound `send`.
    fn lower_std_bus_local_dispatch(
        &mut self,
        args: &[Expr],
        scope: &Scope<'ctx>,
    ) -> Result<(BasicValueEnum<'ctx>, CodegenTy), CodegenError> {
        if args.len() != 2 {
            return Err(CodegenError::Unsupported(format!(
                "std::bus::__local_dispatch takes 2 args (subject, bytes), got {}",
                args.len()
            )));
        }
        let (subj_val, subj_ty) = self.lower_expr(&args[0], scope)?;
        if !matches!(subj_ty, CodegenTy::String | CodegenTy::StringView) {
            return Err(CodegenError::Unsupported(format!(
                "std::bus::__local_dispatch: subject must be String, got {:?}",
                subj_ty
            )));
        }
        let subj_val = self.unpack_view_if_needed(subj_val, &subj_ty)?;
        let (b_val, b_ty) = self.lower_expr(&args[1], scope)?;
        if !matches!(b_ty, CodegenTy::Bytes | CodegenTy::BytesView) {
            return Err(CodegenError::Unsupported(format!(
                "std::bus::__local_dispatch: bytes must be Bytes, got {:?}",
                b_ty
            )));
        }
        let b_val = self.unpack_view_if_needed(b_val, &b_ty)?;
        // The C primitive takes (subject, wire_ptr, wire_size).
        // Bytes carries an explicit length prefix; load it and
        // pass the body pointer plus the length explicitly so the
        // runtime doesn't have to peek at our Bytes layout.
        let i64_t = self.context.i64_type();
        let bytes_ptr = b_val.into_pointer_value();
        let len = self
            .builder
            .build_load(i64_t, bytes_ptr, "dispatch.bytes.len")
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?
            .into_int_value();
        // Body starts after the 8-byte length prefix.
        let body_ptr = unsafe {
            self.builder
                .build_gep(
                    self.context.i8_type(),
                    bytes_ptr,
                    &[i64_t.const_int(8, false)],
                    "dispatch.bytes.body",
                )
                .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?
        };
        let f = self
            .module
            .get_function("lotus_bus_dispatch_wire")
            .expect("lotus_bus_dispatch_wire declared");
        self.builder
            .build_call(
                f,
                &[subj_val.into(), body_ptr.into(), len.into()],
                "bus.local_dispatch",
            )
            .map_err(|e| CodegenError::LlvmEmit(e.to_string()))?;
        // Match the udp surface: return 0 as Int for "success."
        // Callers normally invoke as a statement and ignore the
        // return; the value is here so expression-position calls
        // type-check uniformly.
        Ok((i64_t.const_zero().into(), CodegenTy::Int))
    }

}
