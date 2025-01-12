// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use crate::decl_defs::{DeclTy, DeclTy_, FunParam, FunType};
use crate::pos::PosId;
use crate::reason::Reason;
use crate::typing_decl_provider::Class;
use crate::typing_defs::{Exact, ExpandEnv, Ty, Ty_, TypeExpansion, TypeExpansions};
use crate::typing_env::TEnv;
use crate::typing_error::ReasonsCallback;

pub struct Phase;

impl Phase {
    fn localize<R: Reason>(env: &TEnv<R>, ety_env: &mut ExpandEnv<'_, R>, ty: DeclTy<R>) -> Ty<R> {
        use DeclTy_::*;
        use Ty_::*;
        let tyf = &env.ctx.typing_ty_provider;
        let r = ty.reason().clone();
        match &**ty.node() {
            DTprim(p) => tyf.mk_ty(r, &Tprim(p.clone())),
            DTapply(pos_id, tyl) => match env
                .ctx
                .typing_decl_provider
                .get_class_or_typedef(pos_id.id())
            {
                Some(cls) => Self::localize_class_instantiation(
                    env,
                    ety_env,
                    r,
                    pos_id.clone(),
                    &tyl,
                    Some(&cls),
                ),
                None => {
                    Self::localize_class_instantiation(env, ety_env, r, pos_id.clone(), &tyl, None)
                }
            },
            DTfun(ft) => {
                let pos = r.pos().clone();
                let ft = Self::localize_ft(env, ety_env, pos, ft);
                tyf.mk_ty(r, &Tfun(ft))
            }
        }
    }

    fn localize_class_instantiation<R: Reason>(
        env: &TEnv<R>,
        ety_env: &mut ExpandEnv<'_, R>,
        r: R,
        sid: PosId<R::P>,
        ty_args: &[DeclTy<R>],
        class_info: Option<&Class<R>>,
    ) -> Ty<R> {
        use Ty_::*;
        let tyf = &env.ctx.typing_ty_provider;
        match class_info {
            None => {
                // Without class info, we don't know the kinds of the arguments.
                // We assume they are non-HK types.
                //
                // TODO(hrust): this is not semantically equivalent to the
                // ocaml code. The ocaml code uses a copy of the ety_env
                let tyl = ety_env.with_expand_visible_newtype(|ety_env| {
                    ty_args
                        .iter()
                        .map(|ty| Self::localize(env, ety_env, ty.clone()))
                        .collect()
                });
                tyf.mk_ty(r, &Tclass(sid, Exact::Nonexact, tyl))
            }
            Some(_class_info) => {
                // TODO(hrust): enum_type
                // TODO(hrust): tparams
                assert!(ty_args.is_empty());
                tyf.mk_ty(r, &Tclass(sid, Exact::Nonexact, vec![]))
            }
        }
    }

    fn localize_ft<R: Reason>(
        env: &TEnv<R>,
        ety_env: &mut ExpandEnv<'_, R>,
        _def_pos: R::P,
        ft: &FunType<R, DeclTy<R>>,
    ) -> FunType<R, Ty<R>> {
        // TODO(hrust): tparams
        assert!(ft.ft_params.is_empty());
        let params = ft
            .ft_params
            .iter()
            .map(|fp| {
                let ty = Self::localize_possibly_enforced_ty(env, ety_env, fp.fp_type.clone());
                FunParam {
                    fp_pos: fp.fp_pos.clone(),
                    fp_name: fp.fp_name.clone(),
                    fp_type: ty,
                }
            })
            .collect();
        let ret = Self::localize_possibly_enforced_ty(env, ety_env, ft.ft_ret.clone());
        FunType {
            ft_params: params,
            ft_ret: ret,
        }
    }

    fn localize_possibly_enforced_ty<R: Reason>(
        env: &TEnv<R>,
        ety_env: &mut ExpandEnv<'_, R>,
        ty: DeclTy<R>,
    ) -> Ty<R> {
        Self::localize(env, ety_env, ty)
    }

    pub fn localize_no_subst<R: Reason>(
        env: &TEnv<R>,
        ignore_errors: bool,
        report_cycle: Option<TypeExpansion<R>>,
        ty: DeclTy<R>,
    ) -> Ty<R> {
        let ty_clone = ty.clone();
        let on_error = || {
            if ignore_errors {
                ReasonsCallback::ignore()
            } else {
                ReasonsCallback::invalid_type_hint(ty_clone.pos().clone())
            }
        };
        let mut ety_env = ExpandEnv::new(&env.ctx);
        ety_env
            .set_type_expansions(TypeExpansions::with_report_cycle(report_cycle))
            .set_on_error(ReasonsCallback::<R>::new(&on_error));
        Self::localize(env, &mut ety_env, ty)
    }
}
