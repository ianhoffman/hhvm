// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use std::path::PathBuf;
use std::rc::Rc;

use ocamlrep_derive::ToOcamlRep;
use oxidized::global_options::GlobalOptions;

use hackrs::ast_provider::{AstLocalCache, AstProvider};
use hackrs::decl_ty_provider::DeclTyProvider;
use hackrs::folded_decl_provider::{FoldedDeclLocalCache, FoldedDeclProvider};
use hackrs::naming::Nast;
use hackrs::pos::{Prefix, RelativePath, RelativePathCtx};
use hackrs::pos_provider::PosProvider;
use hackrs::shallow_decl_provider::{ShallowDeclLocalCache, ShallowDeclProvider};
use hackrs::sn_provider::SpecialNamesProvider;
use hackrs::tast;
use hackrs::typing_check_utils::TypingCheckUtils;
use hackrs::typing_ctx::TypingCtx;
use hackrs::typing_decl_provider::{TypingDeclLocalCache, TypingDeclProvider};
use hackrs::typing_ty_provider::TypingTyProvider;

use hackrs::reason::Reason;
// use hackrs::reason::BReason;
use hackrs::reason::NReason;

// fn create_nast(path: PathBuf) -> oxidized::aast::Program<(), ()> {}

fn print_tast<R: Reason>(opts: &GlobalOptions, tast: &tast::Program<R>) {
    #[derive(ToOcamlRep)]
    struct StcFfiPrintTastArgs<'a, R: Reason> {
        opts: &'a GlobalOptions,
        tast: &'a tast::Program<R>,
    }

    let stc_ffi_print_tast = unsafe { ocaml_runtime::named_value("stc_ffi_print_tast").unwrap() };

    let args = StcFfiPrintTastArgs { opts, tast };

    unsafe {
        ocaml_runtime::callback_exn(stc_ffi_print_tast, ocamlrep_ocamlpool::to_ocaml(&args))
            .unwrap();
    }
}

#[no_mangle]
pub extern "C" fn stc_main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <FILENAME1> <FILENAME2> <...>", args[0]);
        std::process::exit(1);
    }
    let mut filenames = Vec::new();
    for arg in &args[1..args.len()] {
        filenames.push(PathBuf::from(arg));
    }

    let relative_path_ctx = Rc::new(RelativePathCtx {
        root: PathBuf::new(),
        hhi: PathBuf::new(),
        dummy: PathBuf::new(),
        tmp: PathBuf::new(),
    });

    let options = Rc::new(oxidized::global_options::GlobalOptions::default());
    let pos_provider = Rc::new(PosProvider::new());
    let sn_provider = Rc::new(SpecialNamesProvider::new(pos_provider.clone()));
    let ast_cache = Rc::new(AstLocalCache::new());
    let ast_provider = Rc::new(AstProvider::new(
        ast_cache,
        relative_path_ctx.clone(),
        sn_provider.clone(),
        options.clone(),
    ));
    let decl_ty_provider = Rc::new(DeclTyProvider::<NReason>::new(pos_provider.clone()));
    let shallow_decl_cache = Rc::new(ShallowDeclLocalCache::new());
    let shallow_decl_provider = Rc::new(ShallowDeclProvider::new(
        shallow_decl_cache,
        decl_ty_provider,
        relative_path_ctx.clone(),
    ));
    let folded_decl_cache = Rc::new(FoldedDeclLocalCache::new());
    let folded_decl_provider = Rc::new(FoldedDeclProvider::new(
        folded_decl_cache.clone(),
        shallow_decl_provider.clone(),
    ));
    let typing_decl_cache = Rc::new(TypingDeclLocalCache::new());
    let typing_decl_provider = Rc::new(TypingDeclProvider::new(
        typing_decl_cache.clone(),
        folded_decl_provider.clone(),
    ));
    let typing_ty_provider = Rc::new(TypingTyProvider::new());
    let ctx = Rc::new(TypingCtx::new(
        typing_decl_provider.clone(),
        typing_ty_provider.clone(),
        ast_provider.clone(),
    ));

    let filenames: Vec<_> = filenames
        .into_iter()
        .map(|fln| {
            let suffix = pos_provider.mk_symbol(
                fln.as_os_str()
                    .to_str()
                    .expect("Unly UTF-8 file paths supported"),
            );
            RelativePath::new(Prefix::Root, suffix)
        })
        .collect();

    shallow_decl_provider
        .add_from_files(&mut filenames.iter())
        .unwrap();

    // println!("{:#?}", shallow_decl_provider);

    for fln in &filenames {
        let &(ref ast, ref parsing_errs) = &*ast_provider.get_ast(true, fln).unwrap();
        let fi = Nast::get_defs(ast);
        let (tast, errs) = TypingCheckUtils::type_file::<NReason>(ctx.clone(), fln, &fi);
        if !errs.is_empty() || !parsing_errs.is_empty() {
            unimplemented!()
        }
        print_tast(&options, &tast);
    }
}
