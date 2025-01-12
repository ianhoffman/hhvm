// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::decl_defs::{DeclTy, DeclTy_, FoldedClass, FoldedElement, ShallowClass, ShallowMethod};
use crate::folded_decl_provider::FoldedDeclCache;
use crate::folded_decl_provider::{inherit::Inherited, subst::Subst};
use crate::pos::{PosId, Symbol};
use crate::reason::Reason;
use crate::shallow_decl_provider::ShallowDeclProvider;

#[derive(Debug)]
pub struct FoldedDeclProvider<R: Reason> {
    cache: Rc<dyn FoldedDeclCache<R = R>>,
    shallow_decl_provider: Rc<ShallowDeclProvider<R>>,
}

impl<R: Reason> FoldedDeclProvider<R> {
    pub fn new(
        cache: Rc<dyn FoldedDeclCache<R = R>>,
        shallow_decl_provider: Rc<ShallowDeclProvider<R>>,
    ) -> Self {
        Self {
            cache,
            shallow_decl_provider,
        }
    }

    pub fn get_shallow_decl_provider(&self) -> &Rc<ShallowDeclProvider<R>> {
        &self.shallow_decl_provider
    }

    pub fn get_folded_class(&self, name: &Symbol) -> Option<Rc<FoldedClass<R>>> {
        let mut stack = HashSet::new();
        self.get_folded_class_impl(&mut stack, name)
    }

    fn detect_cycle(&self, stack: &mut HashSet<Symbol>, pos_id: &PosId<R::P>) -> bool {
        if stack.contains(pos_id.id()) {
            unimplemented!("TODO(hrust): register error");
        }
        false
    }

    fn decl_method(
        &self,
        methods: &mut HashMap<Symbol, FoldedElement>,
        sc: &ShallowClass<R>,
        sm: &ShallowMethod<R>,
    ) {
        let elt = FoldedElement {
            elt_origin: sc.sc_name.id().clone(),
        };
        methods.insert(sm.sm_name.id().clone(), elt);
    }

    fn decl_class_type(
        &self,
        stack: &mut HashSet<Symbol>,
        acc: &mut HashMap<Symbol, Rc<FoldedClass<R>>>,
        ty: &DeclTy<R>,
    ) {
        match &**ty.node() {
            DeclTy_::DTapply(pos_id, _tyl) => {
                if !self.detect_cycle(stack, pos_id) {
                    if let Some(folded_decl) = self.get_folded_class_impl(stack, pos_id.id()) {
                        acc.insert(pos_id.id().clone(), folded_decl);
                    }
                }
            }
            _ => {}
        }
    }

    fn decl_class_parents(
        &self,
        stack: &mut HashSet<Symbol>,
        sc: &ShallowClass<R>,
    ) -> HashMap<Symbol, Rc<FoldedClass<R>>> {
        let mut acc = HashMap::new();
        sc.sc_extends
            .iter()
            .for_each(|ty| self.decl_class_type(stack, &mut acc, ty));
        acc
    }

    fn get_implements(
        &self,
        parents: &HashMap<Symbol, Rc<FoldedClass<R>>>,
        ty: &DeclTy<R>,
        inst: &mut HashMap<Symbol, DeclTy<R>>,
    ) {
        match ty.unwrap_class_type() {
            None => {}
            Some((_, pos_id, tyl)) => match parents.get(pos_id.id()) {
                None => {
                    inst.insert(pos_id.id().clone(), ty.clone());
                }
                Some(cls) => {
                    let subst = Subst::new((), tyl);
                    for (anc_name, anc_ty) in &cls.dc_ancestors {
                        inst.insert(anc_name.clone(), subst.instantiate(anc_ty));
                    }
                    inst.insert(pos_id.id().clone(), ty.clone());
                }
            },
        }
    }

    #[allow(unreachable_code, unused)]
    fn decl_class_impl(
        &self,
        sc: &ShallowClass<R>,
        parents: &HashMap<Symbol, Rc<FoldedClass<R>>>,
    ) -> Rc<FoldedClass<R>> {
        let mut inh = Inherited::make(sc, parents);

        let mut methods = inh.ih_methods;
        sc.sc_methods
            .iter()
            .for_each(|sm| self.decl_method(&mut methods, sc, sm));

        let mut anc = HashMap::new();
        sc.sc_extends
            .iter()
            .for_each(|ty| self.get_implements(parents, ty, &mut anc));

        let tc = FoldedClass {
            dc_name: sc.sc_name.id().clone(),
            dc_pos: sc.sc_name.pos().clone(),
            dc_substs: inh.ih_substs,
            dc_ancestors: anc,
            dc_methods: methods,
        };
        let tc = Rc::new(tc);
        tc
    }

    fn decl_class(&self, stack: &mut HashSet<Symbol>, name: &Symbol) -> Option<Rc<FoldedClass<R>>> {
        let shallow_class = self.get_shallow_decl_provider().get_shallow_class(name)?;
        stack.insert(name.clone());
        let parents = self.decl_class_parents(stack, &shallow_class);
        Some(self.decl_class_impl(&shallow_class, &parents))
    }

    fn get_folded_class_impl(
        &self,
        stack: &mut HashSet<Symbol>,
        name: &Symbol,
    ) -> Option<Rc<FoldedClass<R>>> {
        match self.cache.get_folded_class(name) {
            Some(rc) => Some(rc),
            None => match self.decl_class(stack, name) {
                None => None,
                Some(rc) => {
                    self.cache.put_folded_class(name.clone(), rc.clone());
                    Some(rc)
                }
            },
        }
    }
}
