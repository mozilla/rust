//! The Rust compiler.
//!
//! # Note
//!
//! This API is completely unstable and subject to change.

#![doc(html_root_url = "https://doc.rust-lang.org/nightly/")]
#![feature(bool_to_option)]
#![feature(const_cstr_unchecked)]
#![feature(crate_visibility_modifier)]
#![feature(extern_types)]
#![feature(in_band_lifetimes)]
#![feature(nll)]
#![feature(trusted_len)]
#![recursion_limit = "256"]

use back::write::{create_informational_target_machine, create_target_machine};
use rustc_span::symbol::Symbol;

pub use llvm_util::target_features;
use rustc::dep_graph::WorkProduct;
use rustc_codegen_ssa::back::lto::{LtoModuleCodegen, SerializedModule, ThinModule};
use rustc_codegen_ssa::back::write::{CodegenContext, FatLTOInput, ModuleConfig};
use rustc_codegen_ssa::traits::*;
use rustc_codegen_ssa::{CodegenResults, CompiledModule};
use rustc_errors::{FatalError, Handler};
use std::any::Any;
use std::ffi::CStr;
use std::fs;
use std::sync::Arc;
use syntax::expand::allocator::AllocatorKind;

use rustc::dep_graph::DepGraph;
use rustc::middle::cstore::{EncodedMetadata, MetadataLoaderDyn};
use rustc::session::config::{OptLevel, OutputFilenames, PrintRequest};
use rustc::session::Session;
use rustc::ty::{self, TyCtxt};
use rustc::util::common::ErrorReported;
use rustc_codegen_ssa::ModuleCodegen;
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use rustc_serialize::json;

mod back {
    pub mod archive;
    pub mod bytecode;
    pub mod lto;
    pub mod write;
}

mod abi;
mod allocator;
mod asm;
mod attributes;
mod base;
mod builder;
mod callee;
mod common;
mod consts;
mod context;
mod debuginfo;
mod declare;
mod intrinsic;

// The following is a work around that replaces `pub mod llvm;` and that fixes issue 53912.
#[path = "llvm/mod.rs"]
mod llvm_;
pub mod llvm {
    pub use super::llvm_::*;
}

mod llvm_util;
mod metadata;
mod mono_item;
mod type_;
mod type_of;
mod va_arg;
mod value;

#[derive(Clone)]
pub struct LlvmCodegenBackend(());

impl ExtraBackendMethods for LlvmCodegenBackend {
    fn new_metadata(&self, tcx: TyCtxt<'_>, mod_name: &str) -> ModuleLlvm {
        ModuleLlvm::new_metadata(tcx, mod_name)
    }

    fn write_compressed_metadata<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        metadata: &EncodedMetadata,
        llvm_module: &mut ModuleLlvm,
    ) {
        base::write_compressed_metadata(tcx, metadata, llvm_module)
    }
    fn codegen_allocator<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        mods: &mut ModuleLlvm,
        kind: AllocatorKind,
    ) {
        unsafe { allocator::codegen(tcx, mods, kind) }
    }
    fn compile_codegen_unit(
        &self,
        tcx: TyCtxt<'_>,
        cgu_name: Symbol,
    ) -> (ModuleCodegen<ModuleLlvm>, u64) {
        base::compile_codegen_unit(tcx, cgu_name)
    }
    fn target_machine_factory(
        &self,
        sess: &Session,
        optlvl: OptLevel,
        find_features: bool,
    ) -> Arc<dyn Fn() -> Result<&'static mut llvm::TargetMachine, String> + Send + Sync> {
        back::write::target_machine_factory(sess, optlvl, find_features)
    }
    fn target_cpu<'b>(&self, sess: &'b Session) -> &'b str {
        llvm_util::target_cpu(sess)
    }
}

impl WriteBackendMethods for LlvmCodegenBackend {
    type Module = ModuleLlvm;
    type ModuleBuffer = back::lto::ModuleBuffer;
    type Context = llvm::Context;
    type TargetMachine = &'static mut llvm::TargetMachine;
    type ThinData = back::lto::ThinData;
    type ThinBuffer = back::lto::ThinBuffer;
    fn print_pass_timings(&self) {
        unsafe {
            llvm::LLVMRustPrintPassTimings();
        }
    }
    fn run_fat_lto(
        cgcx: &CodegenContext<Self>,
        modules: Vec<FatLTOInput<Self>>,
        cached_modules: Vec<(SerializedModule<Self::ModuleBuffer>, WorkProduct)>,
    ) -> Result<LtoModuleCodegen<Self>, FatalError> {
        back::lto::run_fat(cgcx, modules, cached_modules)
    }
    fn run_thin_lto(
        cgcx: &CodegenContext<Self>,
        modules: Vec<(String, Self::ThinBuffer)>,
        cached_modules: Vec<(SerializedModule<Self::ModuleBuffer>, WorkProduct)>,
    ) -> Result<(Vec<LtoModuleCodegen<Self>>, Vec<WorkProduct>), FatalError> {
        back::lto::run_thin(cgcx, modules, cached_modules)
    }
    unsafe fn optimize(
        cgcx: &CodegenContext<Self>,
        diag_handler: &Handler,
        module: &ModuleCodegen<Self::Module>,
        config: &ModuleConfig,
    ) -> Result<(), FatalError> {
        back::write::optimize(cgcx, diag_handler, module, config)
    }
    unsafe fn optimize_thin(
        cgcx: &CodegenContext<Self>,
        thin: &mut ThinModule<Self>,
    ) -> Result<ModuleCodegen<Self::Module>, FatalError> {
        back::lto::optimize_thin_module(thin, cgcx)
    }
    unsafe fn codegen(
        cgcx: &CodegenContext<Self>,
        diag_handler: &Handler,
        module: ModuleCodegen<Self::Module>,
        config: &ModuleConfig,
    ) -> Result<CompiledModule, FatalError> {
        back::write::codegen(cgcx, diag_handler, module, config)
    }
    fn prepare_thin(module: ModuleCodegen<Self::Module>) -> (String, Self::ThinBuffer) {
        back::lto::prepare_thin(module)
    }
    fn serialize_module(module: ModuleCodegen<Self::Module>) -> (String, Self::ModuleBuffer) {
        (module.name, back::lto::ModuleBuffer::new(module.module_llvm.llmod()))
    }
    fn run_lto_pass_manager(
        cgcx: &CodegenContext<Self>,
        module: &ModuleCodegen<Self::Module>,
        config: &ModuleConfig,
        thin: bool,
    ) {
        back::lto::run_pass_manager(cgcx, module, config, thin)
    }
}

unsafe impl Send for LlvmCodegenBackend {} // Llvm is on a per-thread basis
unsafe impl Sync for LlvmCodegenBackend {}

impl LlvmCodegenBackend {
    pub fn new() -> Box<dyn CodegenBackend> {
        Box::new(LlvmCodegenBackend(()))
    }
}

impl CodegenBackend for LlvmCodegenBackend {
    fn init(&self, sess: &Session) {
        llvm_util::init(sess); // Make sure llvm is inited
    }

    fn print(&self, req: PrintRequest, sess: &Session) {
        match req {
            PrintRequest::RelocationModels => {
                println!("Available relocation models:");
                for &(name, _) in back::write::RELOC_MODEL_ARGS.iter() {
                    println!("    {}", name);
                }
                println!();
            }
            PrintRequest::CodeModels => {
                println!("Available code models:");
                for &(name, _) in back::write::CODE_GEN_MODEL_ARGS.iter() {
                    println!("    {}", name);
                }
                println!();
            }
            PrintRequest::TlsModels => {
                println!("Available TLS models:");
                for &(name, _) in back::write::TLS_MODEL_ARGS.iter() {
                    println!("    {}", name);
                }
                println!();
            }
            req => llvm_util::print(req, sess),
        }
    }

    fn print_passes(&self) {
        llvm_util::print_passes();
    }

    fn print_version(&self) {
        llvm_util::print_version();
    }

    fn target_features(&self, sess: &Session) -> Vec<Symbol> {
        target_features(sess)
    }

    fn metadata_loader(&self) -> Box<MetadataLoaderDyn> {
        Box::new(metadata::LlvmMetadataLoader)
    }

    fn provide(&self, providers: &mut ty::query::Providers<'_>) {
        attributes::provide(providers);
    }

    fn provide_extern(&self, providers: &mut ty::query::Providers<'_>) {
        attributes::provide_extern(providers);
    }

    fn codegen_crate<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        metadata: EncodedMetadata,
        need_metadata_module: bool,
    ) -> Box<dyn Any> {
        Box::new(rustc_codegen_ssa::base::codegen_crate(
            LlvmCodegenBackend(()),
            tcx,
            metadata,
            need_metadata_module,
        ))
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        sess: &Session,
        dep_graph: &DepGraph,
    ) -> Result<Box<dyn Any>, ErrorReported> {
        let (codegen_results, work_products) = ongoing_codegen
            .downcast::<rustc_codegen_ssa::back::write::OngoingCodegen<LlvmCodegenBackend>>()
            .expect("Expected LlvmCodegenBackend's OngoingCodegen, found Box<Any>")
            .join(sess);
        if sess.opts.debugging_opts.incremental_info {
            rustc_codegen_ssa::back::write::dump_incremental_data(&codegen_results);
        }

        sess.time("serialize_work_products", move || {
            rustc_incremental::save_work_product_index(sess, &dep_graph, work_products)
        });

        sess.compile_status()?;

        Ok(Box::new(codegen_results))
    }

    fn link(
        &self,
        sess: &Session,
        codegen_results: Box<dyn Any>,
        outputs: &OutputFilenames,
    ) -> Result<(), ErrorReported> {
        let codegen_results = codegen_results
            .downcast::<CodegenResults>()
            .expect("Expected CodegenResults, found Box<Any>");

        if sess.opts.debugging_opts.no_link {
            // FIXME: use a binary format to encode the `.rlink` file
            let rlink_data = json::encode(&codegen_results).map_err(|err| {
                sess.fatal(&format!("failed to encode rlink: {}", err));
            })?;
            let rlink_file = outputs.with_extension("rlink");
            fs::write(&rlink_file, rlink_data).map_err(|err| {
                sess.fatal(&format!("failed to write file {}: {}", rlink_file.display(), err));
            })?;
            return Ok(());
        }

        // Run the linker on any artifacts that resulted from the LLVM run.
        // This should produce either a finished executable or library.
        sess.time("link_crate", || {
            use crate::back::archive::LlvmArchiveBuilder;
            use rustc_codegen_ssa::back::link::link_binary;

            let target_cpu = crate::llvm_util::target_cpu(sess);
            link_binary::<LlvmArchiveBuilder<'_>>(
                sess,
                &codegen_results,
                outputs,
                &codegen_results.crate_name.as_str(),
                target_cpu,
            );
        });

        // Now that we won't touch anything in the incremental compilation directory
        // any more, we can finalize it (which involves renaming it)
        rustc_incremental::finalize_session_directory(sess, codegen_results.crate_hash);

        sess.time("llvm_dump_timing_file", || {
            if sess.opts.debugging_opts.llvm_time_trace {
                llvm_util::time_trace_profiler_finish("llvm_timings.json");
            }
        });

        Ok(())
    }
}

pub struct ModuleLlvm {
    llcx: &'static mut llvm::Context,
    llmod_raw: *const llvm::Module,
    tm: &'static mut llvm::TargetMachine,
}

unsafe impl Send for ModuleLlvm {}
unsafe impl Sync for ModuleLlvm {}

impl ModuleLlvm {
    fn new(tcx: TyCtxt<'_>, mod_name: &str) -> Self {
        unsafe {
            let llcx = llvm::LLVMRustContextCreate(tcx.sess.fewer_names());
            let llmod_raw = context::create_module(tcx, llcx, mod_name) as *const _;
            ModuleLlvm { llmod_raw, llcx, tm: create_target_machine(tcx, false) }
        }
    }

    fn new_metadata(tcx: TyCtxt<'_>, mod_name: &str) -> Self {
        unsafe {
            let llcx = llvm::LLVMRustContextCreate(tcx.sess.fewer_names());
            let llmod_raw = context::create_module(tcx, llcx, mod_name) as *const _;
            ModuleLlvm {
                llmod_raw,
                llcx,
                tm: create_informational_target_machine(&tcx.sess, false),
            }
        }
    }

    fn parse(
        cgcx: &CodegenContext<LlvmCodegenBackend>,
        name: &CStr,
        buffer: &[u8],
        handler: &Handler,
    ) -> Result<Self, FatalError> {
        unsafe {
            let llcx = llvm::LLVMRustContextCreate(cgcx.fewer_names);
            let llmod_raw = back::lto::parse_module(llcx, name, buffer, handler)?;
            let tm = match (cgcx.tm_factory.0)() {
                Ok(m) => m,
                Err(e) => {
                    handler.struct_err(&e).emit();
                    return Err(FatalError);
                }
            };

            Ok(ModuleLlvm { llmod_raw, llcx, tm })
        }
    }

    fn llmod(&self) -> &llvm::Module {
        unsafe { &*self.llmod_raw }
    }
}

impl Drop for ModuleLlvm {
    fn drop(&mut self) {
        unsafe {
            llvm::LLVMContextDispose(&mut *(self.llcx as *mut _));
            llvm::LLVMRustDisposeTargetMachine(&mut *(self.tm as *mut _));
        }
    }
}
