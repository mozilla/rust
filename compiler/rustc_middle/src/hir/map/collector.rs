use crate::arena::Arena;
use crate::hir::map::Map;
use crate::hir::{AttributeMap, IndexedHir, OwnerNodes, ParentedNode};
use crate::ich::StableHashingContext;
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_hir as hir;
use rustc_hir::definitions;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::*;
use rustc_index::vec::{Idx, IndexVec};
use rustc_session::Session;
use rustc_span::def_id::{DefPathHash, LocalDefId, CRATE_DEF_INDEX};
use rustc_span::source_map::SourceMap;
use rustc_span::{Span, DUMMY_SP};

use std::iter::repeat;

/// A visitor that walks over the HIR and collects `Node`s into a HIR map.
pub(super) struct NodeCollector<'a, 'hir> {
    arena: &'hir Arena<'hir>,

    /// The crate
    krate: &'hir Crate<'hir>,

    /// Source map
    source_map: &'a SourceMap,

    map: IndexVec<LocalDefId, Option<&'hir mut OwnerNodes<'hir>>>,
    parenting: FxHashMap<LocalDefId, HirId>,

    /// The parent of this node
    parent_node: hir::HirId,

    current_dep_node_owner: LocalDefId,

    definitions: &'a definitions::Definitions,

    hcx: StableHashingContext<'a>,
}

fn insert_vec_map<K: Idx, V: Clone>(map: &mut IndexVec<K, Option<V>>, k: K, v: V) {
    let i = k.index();
    let len = map.len();
    if i >= len {
        map.extend(repeat(None).take(i - len + 1));
    }
    debug_assert!(map[k].is_none());
    map[k] = Some(v);
}

fn hash_body(
    hcx: &mut StableHashingContext<'_>,
    item_like: impl for<'a> HashStable<StableHashingContext<'a>>,
) -> Fingerprint {
    let mut stable_hasher = StableHasher::new();
    hcx.while_hashing_hir_bodies(true, |hcx| {
        item_like.hash_stable(hcx, &mut stable_hasher);
    });
    stable_hasher.finish()
}

/// Represents an entry and its parent `HirId`.
#[derive(Copy, Clone, Debug)]
pub struct Entry<'hir> {
    parent: HirId,
    node: Node<'hir>,
}

impl<'a, 'hir> NodeCollector<'a, 'hir> {
    pub(super) fn root(
        sess: &'a Session,
        arena: &'hir Arena<'hir>,
        krate: &'hir Crate<'hir>,
        definitions: &'a definitions::Definitions,
        mut hcx: StableHashingContext<'a>,
    ) -> NodeCollector<'a, 'hir> {
        let hash = {
            let Crate {
                ref item,
                // These fields are handled separately:
                exported_macros: _,
                non_exported_macro_attrs: _,
                items: _,
                trait_items: _,
                impl_items: _,
                foreign_items: _,
                bodies: _,
                trait_impls: _,
                body_ids: _,
                modules: _,
                proc_macros: _,
                trait_map: _,
                attrs: _,
            } = *krate;

            hash_body(&mut hcx, item)
        };

        let mut collector = NodeCollector {
            arena,
            krate,
            source_map: sess.source_map(),
            parent_node: hir::CRATE_HIR_ID,
            current_dep_node_owner: LocalDefId { local_def_index: CRATE_DEF_INDEX },
            definitions,
            hcx,
            map: IndexVec::from_fn_n(|_| None, definitions.def_index_count()),
            parenting: FxHashMap::default(),
        };
        collector.insert_entry(
            hir::CRATE_HIR_ID,
            Entry { parent: hir::CRATE_HIR_ID, node: Node::Crate(&krate.item) },
            hash,
        );

        collector
    }

    pub(super) fn finalize_and_compute_crate_hash(self) -> IndexedHir<'hir> {
        let NodeCollector { krate, mut map, parenting, mut hcx, definitions, .. } = self;

        // Insert bodies into the map
        for (id, body) in krate.bodies.iter() {
            let bodies = &mut map[id.hir_id.owner].as_mut().unwrap().bodies;
            assert!(bodies.insert(id.hir_id.local_id, body).is_none());
        }

        let mut hir_body_nodes: Vec<(DefPathHash, Fingerprint)> = map
            .iter_enumerated()
            .filter_map(|(def_id, hod)| {
                let def_path_hash = definitions.def_path_hash(def_id);
                let mut hasher = StableHasher::new();
                hod.as_ref()?.hash_stable(&mut hcx, &mut hasher);
                AttributeMap { map: &krate.attrs, prefix: def_id }
                    .hash_stable(&mut hcx, &mut hasher);
                Some((def_path_hash, hasher.finish()))
            })
            .collect();
        hir_body_nodes.sort_unstable_by_key(|bn| bn.0);

        let mut stable_hasher = StableHasher::new();
        hir_body_nodes.hash_stable(&mut hcx, &mut stable_hasher);
        krate.non_exported_macro_attrs.hash_stable(&mut hcx, &mut stable_hasher);
        let nodes_hash = stable_hasher.finish();

        IndexedHir { map, parenting, nodes_hash }
    }

    fn insert_entry(&mut self, id: HirId, entry: Entry<'hir>, hash: Fingerprint) {
        let i = id.local_id.as_u32() as usize;

        let arena = self.arena;

        let data = &mut self.map[id.owner];

        if i == 0 {
            debug_assert!(data.is_none());
            *data = Some(arena.alloc(OwnerNodes {
                hash,
                nodes: IndexVec::new(),
                bodies: FxHashMap::default(),
            }));

            let dk_parent = self.definitions.def_key(id.owner).parent;
            if let Some(dk_parent) = dk_parent {
                let dk_parent = LocalDefId { local_def_index: dk_parent };
                let dk_parent = self.definitions.local_def_id_to_hir_id(dk_parent);
                if dk_parent.owner != entry.parent.owner {
                    panic!(
                        "Different parents for {:?} => dk_parent={:?} actual={:?}",
                        id.owner, dk_parent, entry.parent,
                    )
                }

                debug_assert_eq!(self.parenting.get(&id.owner), Some(&entry.parent));
            }
        } else {
            debug_assert_eq!(entry.parent.owner, id.owner);
        }

        let data = data.as_mut().unwrap();

        insert_vec_map(
            &mut data.nodes,
            id.local_id,
            ParentedNode { parent: entry.parent.local_id, node: entry.node },
        );
    }

    fn insert(&mut self, span: Span, hir_id: HirId, node: Node<'hir>) {
        self.insert_with_hash(span, hir_id, node, Fingerprint::ZERO)
    }

    fn insert_with_hash(&mut self, span: Span, hir_id: HirId, node: Node<'hir>, hash: Fingerprint) {
        let entry = Entry { parent: self.parent_node, node };

        // Make sure that the DepNode of some node coincides with the HirId
        // owner of that node.
        if cfg!(debug_assertions) {
            if hir_id.owner != self.current_dep_node_owner {
                let node_str = match self.definitions.opt_hir_id_to_local_def_id(hir_id) {
                    Some(def_id) => self.definitions.def_path(def_id).to_string_no_crate_verbose(),
                    None => format!("{:?}", node),
                };

                span_bug!(
                    span,
                    "inconsistent DepNode at `{:?}` for `{}`: \
                     current_dep_node_owner={} ({:?}), hir_id.owner={} ({:?})",
                    self.source_map.span_to_diagnostic_string(span),
                    node_str,
                    self.definitions
                        .def_path(self.current_dep_node_owner)
                        .to_string_no_crate_verbose(),
                    self.current_dep_node_owner,
                    self.definitions.def_path(hir_id.owner).to_string_no_crate_verbose(),
                    hir_id.owner,
                )
            }
        }

        self.insert_entry(hir_id, entry, hash);
    }

    fn with_parent<F: FnOnce(&mut Self)>(&mut self, parent_node_id: HirId, f: F) {
        let parent_node = self.parent_node;
        self.parent_node = parent_node_id;
        f(self);
        self.parent_node = parent_node;
    }

    fn with_dep_node_owner<
        T: for<'b> HashStable<StableHashingContext<'b>>,
        F: FnOnce(&mut Self, Fingerprint),
    >(
        &mut self,
        dep_node_owner: LocalDefId,
        item_like: &T,
        f: F,
    ) {
        let prev_owner = self.current_dep_node_owner;
        let hash = hash_body(&mut self.hcx, item_like);

        self.current_dep_node_owner = dep_node_owner;
        f(self, hash);
        self.current_dep_node_owner = prev_owner;
    }

    fn insert_nested(&mut self, item: LocalDefId) {
        #[cfg(debug_assertions)]
        {
            let dk_parent = self.definitions.def_key(item).parent.unwrap();
            let dk_parent = LocalDefId { local_def_index: dk_parent };
            let dk_parent = self.definitions.local_def_id_to_hir_id(dk_parent);
            debug_assert_eq!(
                dk_parent.owner, self.parent_node.owner,
                "Different parents for {:?}",
                item
            )
        }

        assert_eq!(self.parenting.insert(item, self.parent_node), None);
    }
}

impl<'a, 'hir> Visitor<'hir> for NodeCollector<'a, 'hir> {
    type Map = Map<'hir>;

    /// Because we want to track parent items and so forth, enable
    /// deep walking so that we walk nested items in the context of
    /// their outer items.

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        panic!("`visit_nested_xxx` must be manually implemented in this visitor");
    }

    fn visit_nested_item(&mut self, item: ItemId) {
        debug!("visit_nested_item: {:?}", item);
        self.insert_nested(item.def_id);
        self.visit_item(self.krate.item(item));
    }

    fn visit_nested_trait_item(&mut self, item_id: TraitItemId) {
        self.insert_nested(item_id.def_id);
        self.visit_trait_item(self.krate.trait_item(item_id));
    }

    fn visit_nested_impl_item(&mut self, item_id: ImplItemId) {
        self.insert_nested(item_id.def_id);
        self.visit_impl_item(self.krate.impl_item(item_id));
    }

    fn visit_nested_foreign_item(&mut self, foreign_id: ForeignItemId) {
        self.insert_nested(foreign_id.def_id);
        self.visit_foreign_item(self.krate.foreign_item(foreign_id));
    }

    fn visit_nested_body(&mut self, id: BodyId) {
        self.visit_body(self.krate.body(id));
    }

    fn visit_param(&mut self, param: &'hir Param<'hir>) {
        let node = Node::Param(param);
        self.insert(param.pat.span, param.hir_id, node);
        self.with_parent(param.hir_id, |this| {
            intravisit::walk_param(this, param);
        });
    }

    fn visit_item(&mut self, i: &'hir Item<'hir>) {
        debug!("visit_item: {:?}", i);
        self.with_dep_node_owner(i.def_id, i, |this, hash| {
            let hir_id = i.hir_id();
            this.insert_with_hash(i.span, hir_id, Node::Item(i), hash);
            this.with_parent(hir_id, |this| {
                if let ItemKind::Struct(ref struct_def, _) = i.kind {
                    // If this is a tuple or unit-like struct, register the constructor.
                    if let Some(ctor_hir_id) = struct_def.ctor_hir_id() {
                        this.insert(i.span, ctor_hir_id, Node::Ctor(struct_def));
                    }
                }
                intravisit::walk_item(this, i);
            });
        });
    }

    fn visit_foreign_item(&mut self, fi: &'hir ForeignItem<'hir>) {
        self.with_dep_node_owner(fi.def_id, fi, |this, hash| {
            this.insert_with_hash(fi.span, fi.hir_id(), Node::ForeignItem(fi), hash);

            this.with_parent(fi.hir_id(), |this| {
                intravisit::walk_foreign_item(this, fi);
            });
        });
    }

    fn visit_generic_param(&mut self, param: &'hir GenericParam<'hir>) {
        self.insert(param.span, param.hir_id, Node::GenericParam(param));
        intravisit::walk_generic_param(self, param);
    }

    fn visit_const_param_default(&mut self, param: HirId, ct: &'hir AnonConst) {
        self.with_parent(param, |this| intravisit::walk_const_param_default(this, ct))
    }

    fn visit_trait_item(&mut self, ti: &'hir TraitItem<'hir>) {
        self.with_dep_node_owner(ti.def_id, ti, |this, hash| {
            this.insert_with_hash(ti.span, ti.hir_id(), Node::TraitItem(ti), hash);

            this.with_parent(ti.hir_id(), |this| {
                intravisit::walk_trait_item(this, ti);
            });
        });
    }

    fn visit_impl_item(&mut self, ii: &'hir ImplItem<'hir>) {
        self.with_dep_node_owner(ii.def_id, ii, |this, hash| {
            this.insert_with_hash(ii.span, ii.hir_id(), Node::ImplItem(ii), hash);

            this.with_parent(ii.hir_id(), |this| {
                intravisit::walk_impl_item(this, ii);
            });
        });
    }

    fn visit_pat(&mut self, pat: &'hir Pat<'hir>) {
        let node =
            if let PatKind::Binding(..) = pat.kind { Node::Binding(pat) } else { Node::Pat(pat) };
        self.insert(pat.span, pat.hir_id, node);

        self.with_parent(pat.hir_id, |this| {
            intravisit::walk_pat(this, pat);
        });
    }

    fn visit_arm(&mut self, arm: &'hir Arm<'hir>) {
        let node = Node::Arm(arm);

        self.insert(arm.span, arm.hir_id, node);

        self.with_parent(arm.hir_id, |this| {
            intravisit::walk_arm(this, arm);
        });
    }

    fn visit_anon_const(&mut self, constant: &'hir AnonConst) {
        self.insert(DUMMY_SP, constant.hir_id, Node::AnonConst(constant));

        self.with_parent(constant.hir_id, |this| {
            intravisit::walk_anon_const(this, constant);
        });
    }

    fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
        self.insert(expr.span, expr.hir_id, Node::Expr(expr));

        self.with_parent(expr.hir_id, |this| {
            intravisit::walk_expr(this, expr);
        });
    }

    fn visit_stmt(&mut self, stmt: &'hir Stmt<'hir>) {
        self.insert(stmt.span, stmt.hir_id, Node::Stmt(stmt));

        self.with_parent(stmt.hir_id, |this| {
            intravisit::walk_stmt(this, stmt);
        });
    }

    fn visit_path_segment(&mut self, path_span: Span, path_segment: &'hir PathSegment<'hir>) {
        if let Some(hir_id) = path_segment.hir_id {
            self.insert(path_span, hir_id, Node::PathSegment(path_segment));
        }
        intravisit::walk_path_segment(self, path_span, path_segment);
    }

    fn visit_ty(&mut self, ty: &'hir Ty<'hir>) {
        self.insert(ty.span, ty.hir_id, Node::Ty(ty));

        self.with_parent(ty.hir_id, |this| {
            intravisit::walk_ty(this, ty);
        });
    }

    fn visit_trait_ref(&mut self, tr: &'hir TraitRef<'hir>) {
        self.insert(tr.path.span, tr.hir_ref_id, Node::TraitRef(tr));

        self.with_parent(tr.hir_ref_id, |this| {
            intravisit::walk_trait_ref(this, tr);
        });
    }

    fn visit_fn(
        &mut self,
        fk: intravisit::FnKind<'hir>,
        fd: &'hir FnDecl<'hir>,
        b: BodyId,
        s: Span,
        id: HirId,
    ) {
        assert_eq!(self.parent_node, id);
        intravisit::walk_fn(self, fk, fd, b, s, id);
    }

    fn visit_block(&mut self, block: &'hir Block<'hir>) {
        self.insert(block.span, block.hir_id, Node::Block(block));
        self.with_parent(block.hir_id, |this| {
            intravisit::walk_block(this, block);
        });
    }

    fn visit_local(&mut self, l: &'hir Local<'hir>) {
        self.insert(l.span, l.hir_id, Node::Local(l));
        self.with_parent(l.hir_id, |this| intravisit::walk_local(this, l))
    }

    fn visit_lifetime(&mut self, lifetime: &'hir Lifetime) {
        self.insert(lifetime.span, lifetime.hir_id, Node::Lifetime(lifetime));
    }

    fn visit_vis(&mut self, visibility: &'hir Visibility<'hir>) {
        match visibility.node {
            VisibilityKind::Public | VisibilityKind::Crate(_) | VisibilityKind::Inherited => {}
            VisibilityKind::Restricted { hir_id, .. } => {
                self.insert(visibility.span, hir_id, Node::Visibility(visibility));
                self.with_parent(hir_id, |this| {
                    intravisit::walk_vis(this, visibility);
                });
            }
        }
    }

    fn visit_macro_def(&mut self, macro_def: &'hir MacroDef<'hir>) {
        // Exported macros are visited directly from the crate root,
        // so they do not have `parent_node` set.
        // Find the correct enclosing module from their DefKey.
        let def_key = self.definitions.def_key(macro_def.def_id);
        let parent = def_key.parent.map_or(hir::CRATE_HIR_ID, |local_def_index| {
            self.definitions.local_def_id_to_hir_id(LocalDefId { local_def_index })
        });
        self.with_parent(parent, |this| {
            this.insert_nested(macro_def.def_id);
            this.with_dep_node_owner(macro_def.def_id, macro_def, |this, hash| {
                this.insert_with_hash(
                    macro_def.span,
                    macro_def.hir_id(),
                    Node::MacroDef(macro_def),
                    hash,
                );
            })
        });
    }

    fn visit_variant(&mut self, v: &'hir Variant<'hir>, g: &'hir Generics<'hir>, item_id: HirId) {
        self.insert(v.span, v.id, Node::Variant(v));
        self.with_parent(v.id, |this| {
            // Register the constructor of this variant.
            if let Some(ctor_hir_id) = v.data.ctor_hir_id() {
                this.insert(v.span, ctor_hir_id, Node::Ctor(&v.data));
            }
            intravisit::walk_variant(this, v, g, item_id);
        });
    }

    fn visit_field_def(&mut self, field: &'hir FieldDef<'hir>) {
        self.insert(field.span, field.hir_id, Node::Field(field));
        self.with_parent(field.hir_id, |this| {
            intravisit::walk_field_def(this, field);
        });
    }

    fn visit_trait_item_ref(&mut self, ii: &'hir TraitItemRef) {
        // Do not visit the duplicate information in TraitItemRef. We want to
        // map the actual nodes, not the duplicate ones in the *Ref.
        let TraitItemRef { id, ident: _, kind: _, span: _, defaultness: _ } = *ii;

        self.visit_nested_trait_item(id);
    }

    fn visit_impl_item_ref(&mut self, ii: &'hir ImplItemRef<'hir>) {
        // Do not visit the duplicate information in ImplItemRef. We want to
        // map the actual nodes, not the duplicate ones in the *Ref.
        let ImplItemRef { id, ident: _, kind: _, span: _, vis: _, defaultness: _ } = *ii;

        self.visit_nested_impl_item(id);
    }

    fn visit_foreign_item_ref(&mut self, fi: &'hir ForeignItemRef<'hir>) {
        // Do not visit the duplicate information in ForeignItemRef. We want to
        // map the actual nodes, not the duplicate ones in the *Ref.
        let ForeignItemRef { id, ident: _, span: _, vis: _ } = *fi;

        self.visit_nested_foreign_item(id);
    }
}
