//! Code to save/load the dep-graph from files.

use rustc_data_structures::fx::FxHashMap;
use rustc::dep_graph::{DepGraph, DepGraphArgs, decode_dep_graph, gc_dep_graph};
use rustc::session::Session;
use rustc::ty::TyCtxt;
use rustc::ty::query::OnDiskCache;
use rustc::util::common::{time, time_ext};
use rustc_serialize::Decodable as RustcDecodable;
use rustc_serialize::opaque::Decoder;
use rustc_serialize::Encodable;
use std::path::Path;
use std::fs::{self, File};
use std::io::{Seek, SeekFrom};

use super::data::*;
use super::fs::*;
use super::file_format;
use super::work_product;
use super::save::save_in;

pub fn dep_graph_tcx_init<'tcx>(tcx: TyCtxt<'tcx>) {
    if !tcx.dep_graph.is_fully_enabled() {
        return
    }

    tcx.allocate_metadata_dep_nodes();
}

pub fn dep_graph_from_future(sess: &Session, future: DepGraphFuture) -> DepGraph {
    let args = time(sess, "blocked while dep-graph loading finishes", || {
        future.open().unwrap_or_else(|e| LoadResult::Error {
            message: format!("could not decode incremental cache: {:?}", e),
        }).open(sess).unwrap_or_else(|| {
            let path = dep_graph_path_from(&sess.incr_comp_session_dir());
            // Write the file header to the temp file
            let file = save_in(sess, &path, |encoder| {
                // Encode the commandline arguments hash
                sess.opts.dep_tracking_hash().encode(encoder).unwrap();
            }).unwrap();

            let use_model = sess.reconstruct_dep_graph() ||
                cfg!(debug_assertions);

            DepGraphArgs {
                prev_graph: Default::default(),
                prev_work_products: Default::default(),
                file: Some(file),
                state: Default::default(),
                invalidated: Vec::new(),
                model: if use_model {
                    Some(Default::default())
                } else {
                    None
                },
            }
        })
    });

    DepGraph::new(args)
}

pub enum LoadResult<T> {
    Ok { data: T },
    DataOutOfDate,
    Error { message: String },
}

impl LoadResult<DepGraphArgs> {
    pub fn open(self, sess: &Session) -> Option<DepGraphArgs> {
        match self {
            LoadResult::Error { message } => {
                sess.warn(&message);
                None
            },
            LoadResult::DataOutOfDate => {
                if let Err(err) = delete_all_session_dir_contents(sess) {
                    sess.err(&format!("Failed to delete invalidated or incompatible \
                                      incremental compilation session directory contents `{}`: {}.",
                                      dep_graph_path(sess).display(), err));
                }
                None
            }
            LoadResult::Ok { data } => Some(data)
        }
    }
}


fn load_data(report_incremental_info: bool, path: &Path) -> LoadResult<(Vec<u8>, usize, File)> {
    match file_format::read_file(report_incremental_info, path) {
        Ok(Some(data_and_pos)) => LoadResult::Ok {
            data: data_and_pos
        },
        Ok(None) => {
            // The file either didn't exist or was produced by an incompatible
            // compiler version. Neither is an error.
            LoadResult::DataOutOfDate
        }
        Err(err) => {
            LoadResult::Error {
                message: format!("could not load dep-graph from `{}`: {}",
                                  path.display(), err)
            }
        }
    }
}

fn delete_dirty_work_product(sess: &Session,
                             swp: SerializedWorkProduct) {
    debug!("delete_dirty_work_product({:?})", swp);
    work_product::delete_workproduct_files(sess, &swp.work_product);
}

/// Either a result that has already be computed or a
/// handle that will let us wait until it is computed
/// by a background thread.
pub enum MaybeAsync<T> {
    Sync(T),
    Async(std::thread::JoinHandle<T>)
}
impl<T> MaybeAsync<T> {
    pub fn open(self) -> std::thread::Result<T> {
        match self {
            MaybeAsync::Sync(result) => Ok(result),
            MaybeAsync::Async(handle) => handle.join()
        }
    }
}

pub type DepGraphFuture = MaybeAsync<LoadResult<DepGraphArgs>>;

fn load_graph_file(
    report_incremental_info: bool,
    path: &Path,
    expected_hash: u64,
) -> LoadResult<(Vec<u8>, usize, File)> {
    match load_data(report_incremental_info, &path) {
        LoadResult::DataOutOfDate => LoadResult::DataOutOfDate,
        LoadResult::Error { message } => LoadResult::Error { message },
        LoadResult::Ok { data: (bytes, start_pos, file) } => {
            let mut decoder = Decoder::new(&bytes, start_pos);
            let prev_commandline_args_hash = u64::decode(&mut decoder)
                .expect("Error reading commandline arg hash from cached dep-graph");

            if prev_commandline_args_hash != expected_hash {
                if report_incremental_info {
                    println!("[incremental] completely ignoring cache because of \
                            differing commandline arguments");
                }
                // We can't reuse the cache, purge it.
                debug!("load_dep_graph_new: differing commandline arg hashes");

                // No need to do any further work
                return LoadResult::DataOutOfDate;
            }
            let pos = decoder.position();
            LoadResult::Ok {
                data: (bytes, pos, file)
            }
        }
    }
}

/// Launch a thread and load the dependency graph in the background.
pub fn load_dep_graph(sess: &Session) -> DepGraphFuture {
    // Since `sess` isn't `Sync`, we perform all accesses to `sess`
    // before we fire the background thread.

    let time_passes = sess.time_passes();

    let use_model = sess.reconstruct_dep_graph() || cfg!(debug_assertions);

    if sess.opts.incremental.is_none() {
        let args = DepGraphArgs {
            prev_graph: Default::default(),
            prev_work_products: Default::default(),
            file: None,
            state: Default::default(),
            invalidated: Vec::new(),
            model: if use_model {
                Some(Default::default())
            } else {
                None
            },
        };

        // No incremental compilation.
        return MaybeAsync::Sync(LoadResult::Ok {
            data: args,
        });
    }

    // Calling `sess.incr_comp_session_dir()` will panic if `sess.opts.incremental.is_none()`.
    // Fortunately, we just checked that this isn't the case.
    let dir = &sess.incr_comp_session_dir();

    let path = dep_graph_path_from(dir);
    {
        let temp_path = path.with_extension("tmp");
        if path.exists() {
            fs::copy(&path, &temp_path).unwrap();
            fs::remove_file(&path).unwrap();
            fs::rename(&temp_path, &path).unwrap();
        }
    }

    let results_path = dir.join(DEP_GRAPH_RESULTS_FILENAME);

    let report_incremental_info = sess.opts.debugging_opts.incremental_info;
    let expected_hash = sess.opts.dep_tracking_hash();

    let mut prev_work_products = FxHashMap::default();

    // If we are only building with -Zquery-dep-graph but without an actual
    // incr. comp. session directory, we skip this. Otherwise we'd fail
    // when trying to load work products.
    if sess.incr_comp_session_dir_opt().is_some() {
        let work_products_path = work_products_path(sess);
        let load_result = load_data(report_incremental_info, &work_products_path);

        if let LoadResult::Ok { data: (work_products_data, start_pos, _) } = load_result {
            // Decode the list of work_products
            let mut work_product_decoder = Decoder::new(&work_products_data[..], start_pos);
            let work_products: Vec<SerializedWorkProduct> =
                RustcDecodable::decode(&mut work_product_decoder).unwrap_or_else(|e| {
                    let msg = format!("Error decoding `work-products` from incremental \
                                    compilation session directory: {}", e);
                    sess.fatal(&msg[..])
                });

            for swp in work_products {
                let mut all_files_exist = true;
                for &(_, ref file_name) in swp.work_product.saved_files.iter() {
                    let path = in_incr_comp_dir_sess(sess, file_name);
                    if !path.exists() {
                        all_files_exist = false;

                        if sess.opts.debugging_opts.incremental_info {
                            eprintln!("incremental: could not find file for work \
                                    product: {}", path.display());
                        }
                    }
                }

                if all_files_exist {
                    debug!("reconcile_work_products: all files for {:?} exist", swp);
                    prev_work_products.insert(swp.id, swp.work_product);
                } else {
                    debug!("reconcile_work_products: some file for {:?} does not exist", swp);
                    delete_dirty_work_product(sess, swp);
                }
            }
        }
    }

    MaybeAsync::Async(std::thread::spawn(move || {
        time_ext(time_passes, None, "background load prev dep-graph", move || {
            let (bytes, pos, mut file) = match load_graph_file(
                report_incremental_info,
                &path,
                expected_hash
            ) {
                LoadResult::DataOutOfDate => return LoadResult::DataOutOfDate,
                LoadResult::Error { message } => return LoadResult::Error { message },
                LoadResult::Ok { data } => data,
            };

            let (results_bytes, results_pos, _) = match load_graph_file(
                report_incremental_info,
                &results_path,
                expected_hash,
            ) {
                LoadResult::DataOutOfDate => return LoadResult::DataOutOfDate,
                LoadResult::Error { message } => return LoadResult::Error { message },
                LoadResult::Ok { data } => data,
            };

            let mut decoder = Decoder::new(&bytes, pos);
            let mut results_decoder = Decoder::new(&results_bytes, results_pos);

            let mut result = time_ext(time_passes, None, "decode prev dep-graph", || {
                decode_dep_graph(
                    time_passes,
                    &mut decoder,
                    &mut results_decoder,
                ).expect("Error reading cached dep-graph")
            });

            if result.needs_gc {
                // Reset the file to just the header
                file.seek(SeekFrom::Start(pos as u64)).unwrap();
                file.set_len(pos as u64).unwrap();

                time_ext(time_passes, None, "garbage collect prev dep-graph", || {
                    let mut decoder = Decoder::new(&bytes, pos);
                    gc_dep_graph(time_passes, &mut decoder, &result, &mut file);
                });
            }

            if use_model && result.model.is_none() {
                result.model = Some(Default::default());
            }

            LoadResult::Ok {
                data: DepGraphArgs {
                    prev_graph: result.prev_graph,
                    prev_work_products,
                    file: Some(file),
                    state: result.state,
                    invalidated: result.invalidated,
                    model: result.model,
                }
            }
        })
    }))
}

pub fn load_query_result_cache<'sess>(sess: &'sess Session) -> OnDiskCache<'sess> {
    if sess.opts.incremental.is_none() ||
       !sess.opts.debugging_opts.incremental_queries {
        return OnDiskCache::new_empty(sess.source_map());
    }

    match load_data(sess.opts.debugging_opts.incremental_info, &query_cache_path(sess)) {
        LoadResult::Ok{ data: (bytes, start_pos, _) } => OnDiskCache::new(sess, bytes, start_pos),
        _ => OnDiskCache::new_empty(sess.source_map())
    }
}
