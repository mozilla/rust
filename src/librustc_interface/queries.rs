use crate::interface::{Compiler, Result};
use crate::passes::{self, BoxedResolver, QueryContext};

use rustc::arena::Arena;
use rustc::dep_graph::DepGraph;
use rustc::session::config::{OutputFilenames, OutputType};
use rustc::session::Session;
use rustc::ty::steal::Steal;
use rustc::ty::{GlobalCtxt, ResolverOutputs};
use rustc::util::common::ErrorReported;
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use rustc_data_structures::sync::{Lrc, Once, WorkerLocal};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::Crate;
use rustc_incremental::DepGraphFuture;
use rustc_lint::LintStore;
use std::any::Any;
use std::cell::{Ref, RefCell, RefMut};
use std::mem;
use std::rc::Rc;
use syntax::{self, ast};

/// Represent the result of a query.
/// This result can be stolen with the `take` method and generated with the `compute` method.
pub struct Query<T> {
    result: RefCell<Option<Result<T>>>,
}

impl<T> Query<T> {
    fn compute<F: FnOnce() -> Result<T>>(&self, f: F) -> Result<&Query<T>> {
        let mut result = self.result.borrow_mut();
        if result.is_none() {
            *result = Some(f());
        }
        result.as_ref().unwrap().as_ref().map(|_| self).map_err(|err| *err)
    }

    /// Takes ownership of the query result. Further attempts to take or peek the query
    /// result will panic unless it is generated by calling the `compute` method.
    pub fn take(&self) -> T {
        self.result.borrow_mut().take().expect("missing query result").unwrap()
    }

    /// Borrows the query result using the RefCell. Panics if the result is stolen.
    pub fn peek(&self) -> Ref<'_, T> {
        Ref::map(self.result.borrow(), |r| {
            r.as_ref().unwrap().as_ref().expect("missing query result")
        })
    }

    /// Mutably borrows the query result using the RefCell. Panics if the result is stolen.
    pub fn peek_mut(&self) -> RefMut<'_, T> {
        RefMut::map(self.result.borrow_mut(), |r| {
            r.as_mut().unwrap().as_mut().expect("missing query result")
        })
    }
}

impl<T> Default for Query<T> {
    fn default() -> Self {
        Query { result: RefCell::new(None) }
    }
}

pub struct Queries<'tcx> {
    compiler: &'tcx Compiler,
    gcx: Once<GlobalCtxt<'tcx>>,

    arena: WorkerLocal<Arena<'tcx>>,

    dep_graph_future: Query<Option<DepGraphFuture>>,
    parse: Query<ast::Crate>,
    crate_name: Query<String>,
    register_plugins: Query<(ast::Crate, Lrc<LintStore>)>,
    expansion: Query<(ast::Crate, Steal<Rc<RefCell<BoxedResolver>>>, Lrc<LintStore>)>,
    dep_graph: Query<DepGraph>,
    lower_to_hir: Query<(&'tcx Crate<'tcx>, Steal<ResolverOutputs>)>,
    prepare_outputs: Query<OutputFilenames>,
    global_ctxt: Query<QueryContext<'tcx>>,
    ongoing_codegen: Query<Box<dyn Any>>,
}

impl<'tcx> Queries<'tcx> {
    pub fn new(compiler: &'tcx Compiler) -> Queries<'tcx> {
        Queries {
            compiler,
            gcx: Once::new(),
            arena: WorkerLocal::new(|_| Arena::default()),
            dep_graph_future: Default::default(),
            parse: Default::default(),
            crate_name: Default::default(),
            register_plugins: Default::default(),
            expansion: Default::default(),
            dep_graph: Default::default(),
            lower_to_hir: Default::default(),
            prepare_outputs: Default::default(),
            global_ctxt: Default::default(),
            ongoing_codegen: Default::default(),
        }
    }

    fn session(&self) -> &Lrc<Session> {
        &self.compiler.sess
    }
    fn codegen_backend(&self) -> &Lrc<Box<dyn CodegenBackend>> {
        &self.compiler.codegen_backend()
    }

    pub fn dep_graph_future(&self) -> Result<&Query<Option<DepGraphFuture>>> {
        self.dep_graph_future.compute(|| {
            Ok(self
                .session()
                .opts
                .build_dep_graph()
                .then(|| rustc_incremental::load_dep_graph(self.session())))
        })
    }

    pub fn parse(&self) -> Result<&Query<ast::Crate>> {
        self.parse.compute(|| {
            passes::parse(self.session(), &self.compiler.input).map_err(|mut parse_error| {
                parse_error.emit();
                ErrorReported
            })
        })
    }

    pub fn register_plugins(&self) -> Result<&Query<(ast::Crate, Lrc<LintStore>)>> {
        self.register_plugins.compute(|| {
            let crate_name = self.crate_name()?.peek().clone();
            let krate = self.parse()?.take();

            let empty: &(dyn Fn(&Session, &mut LintStore) + Sync + Send) = &|_, _| {};
            let result = passes::register_plugins(
                self.session(),
                &*self.codegen_backend().metadata_loader(),
                self.compiler.register_lints.as_ref().map(|p| &**p).unwrap_or_else(|| empty),
                krate,
                &crate_name,
            );

            // Compute the dependency graph (in the background). We want to do
            // this as early as possible, to give the DepGraph maximum time to
            // load before dep_graph() is called, but it also can't happen
            // until after rustc_incremental::prepare_session_directory() is
            // called, which happens within passes::register_plugins().
            self.dep_graph_future().ok();

            result
        })
    }

    pub fn crate_name(&self) -> Result<&Query<String>> {
        self.crate_name.compute(|| {
            Ok(match self.compiler.crate_name {
                Some(ref crate_name) => crate_name.clone(),
                None => {
                    let parse_result = self.parse()?;
                    let krate = parse_result.peek();
                    rustc_codegen_utils::link::find_crate_name(
                        Some(self.session()),
                        &krate.attrs,
                        &self.compiler.input,
                    )
                }
            })
        })
    }

    pub fn expansion(
        &self,
    ) -> Result<&Query<(ast::Crate, Steal<Rc<RefCell<BoxedResolver>>>, Lrc<LintStore>)>> {
        self.expansion.compute(|| {
            let crate_name = self.crate_name()?.peek().clone();
            let (krate, lint_store) = self.register_plugins()?.take();
            let _timer = self.session().timer("configure_and_expand");
            passes::configure_and_expand(
                self.session().clone(),
                lint_store.clone(),
                self.codegen_backend().metadata_loader(),
                krate,
                &crate_name,
            )
            .map(|(krate, resolver)| {
                (krate, Steal::new(Rc::new(RefCell::new(resolver))), lint_store)
            })
        })
    }

    pub fn dep_graph(&self) -> Result<&Query<DepGraph>> {
        self.dep_graph.compute(|| {
            Ok(match self.dep_graph_future()?.take() {
                None => DepGraph::new_disabled(),
                Some(future) => {
                    let (prev_graph, prev_work_products) =
                        self.session().time("blocked_on_dep_graph_loading", || {
                            future
                                .open()
                                .unwrap_or_else(|e| rustc_incremental::LoadResult::Error {
                                    message: format!("could not decode incremental cache: {:?}", e),
                                })
                                .open(self.session())
                        });
                    DepGraph::new(prev_graph, prev_work_products)
                }
            })
        })
    }

    pub fn lower_to_hir(&'tcx self) -> Result<&Query<(&'tcx Crate<'tcx>, Steal<ResolverOutputs>)>> {
        self.lower_to_hir.compute(|| {
            let expansion_result = self.expansion()?;
            let peeked = expansion_result.peek();
            let krate = &peeked.0;
            let resolver = peeked.1.steal();
            let lint_store = &peeked.2;
            let hir = resolver.borrow_mut().access(|resolver| {
                Ok(passes::lower_to_hir(
                    self.session(),
                    lint_store,
                    resolver,
                    &*self.dep_graph()?.peek(),
                    &krate,
                    &self.arena,
                ))
            })?;
            let hir = self.arena.alloc(hir);
            Ok((hir, Steal::new(BoxedResolver::to_resolver_outputs(resolver))))
        })
    }

    pub fn prepare_outputs(&self) -> Result<&Query<OutputFilenames>> {
        self.prepare_outputs.compute(|| {
            let expansion_result = self.expansion()?;
            let (krate, boxed_resolver, _) = &*expansion_result.peek();
            let crate_name = self.crate_name()?;
            let crate_name = crate_name.peek();
            passes::prepare_outputs(
                self.session(),
                self.compiler,
                &krate,
                &boxed_resolver,
                &crate_name,
            )
        })
    }

    pub fn global_ctxt(&'tcx self) -> Result<&Query<QueryContext<'tcx>>> {
        self.global_ctxt.compute(|| {
            let crate_name = self.crate_name()?.peek().clone();
            let outputs = self.prepare_outputs()?.peek().clone();
            let lint_store = self.expansion()?.peek().2.clone();
            let hir = self.lower_to_hir()?.peek();
            let dep_graph = self.dep_graph()?.peek().clone();
            let (ref krate, ref resolver_outputs) = &*hir;
            let _timer = self.session().timer("create_global_ctxt");
            Ok(passes::create_global_ctxt(
                self.compiler,
                lint_store,
                krate,
                dep_graph,
                resolver_outputs.steal(),
                outputs,
                &crate_name,
                &self.gcx,
                &self.arena,
            ))
        })
    }

    pub fn ongoing_codegen(&'tcx self) -> Result<&Query<Box<dyn Any>>> {
        self.ongoing_codegen.compute(|| {
            let outputs = self.prepare_outputs()?;
            self.global_ctxt()?.peek_mut().enter(|tcx| {
                tcx.analysis(LOCAL_CRATE).ok();

                // Don't do code generation if there were any errors
                self.session().compile_status()?;

                Ok(passes::start_codegen(&***self.codegen_backend(), tcx, &*outputs.peek()))
            })
        })
    }

    pub fn linker(&'tcx self) -> Result<Linker> {
        let dep_graph = self.dep_graph()?;
        let prepare_outputs = self.prepare_outputs()?;
        let ongoing_codegen = self.ongoing_codegen()?;

        let sess = self.session().clone();
        let codegen_backend = self.codegen_backend().clone();

        Ok(Linker {
            sess,
            dep_graph: dep_graph.peek().clone(),
            prepare_outputs: prepare_outputs.take(),
            ongoing_codegen: ongoing_codegen.take(),
            codegen_backend,
        })
    }
}

pub struct Linker {
    sess: Lrc<Session>,
    dep_graph: DepGraph,
    prepare_outputs: OutputFilenames,
    ongoing_codegen: Box<dyn Any>,
    codegen_backend: Lrc<Box<dyn CodegenBackend>>,
}

impl Linker {
    pub fn link(self) -> Result<()> {
        let codegen_results =
            self.codegen_backend.join_codegen(self.ongoing_codegen, &self.sess, &self.dep_graph)?;
        let prof = self.sess.prof.clone();
        let dep_graph = self.dep_graph;
        prof.generic_activity("drop_dep_graph").run(move || drop(dep_graph));

        if !self
            .sess
            .opts
            .output_types
            .keys()
            .any(|&i| i == OutputType::Exe || i == OutputType::Metadata)
        {
            return Ok(());
        }
        self.codegen_backend.link(&self.sess, codegen_results, &self.prepare_outputs)
    }
}

impl Compiler {
    pub fn enter<F, T>(&self, f: F) -> T
    where
        F: for<'tcx> FnOnce(&'tcx Queries<'tcx>) -> T,
    {
        let mut _timer = None;
        let queries = Queries::new(&self);
        let ret = f(&queries);

        if self.session().opts.debugging_opts.query_stats {
            if let Ok(gcx) = queries.global_ctxt() {
                gcx.peek().print_stats();
            }
        }

        _timer = Some(self.session().timer("free_global_ctxt"));

        ret
    }

    // This method is different to all the other methods in `Compiler` because
    // it lacks a `Queries` entry. It's also not currently used. It does serve
    // as an example of how `Compiler` can be used, with additional steps added
    // between some passes. And see `rustc_driver::run_compiler` for a more
    // complex example.
    pub fn compile(&self) -> Result<()> {
        let linker = self.enter(|queries| {
            queries.prepare_outputs()?;

            if self.session().opts.output_types.contains_key(&OutputType::DepInfo)
                && self.session().opts.output_types.len() == 1
            {
                return Ok(None);
            }

            queries.global_ctxt()?;

            // Drop AST after creating GlobalCtxt to free memory.
            mem::drop(queries.expansion()?.take());

            queries.ongoing_codegen()?;

            let linker = queries.linker()?;
            Ok(Some(linker))
        })?;

        if let Some(linker) = linker {
            linker.link()?
        }

        Ok(())
    }
}
