use crate::clean;
use crate::config::OutputFormat;
use crate::core::DocContext;
use crate::fold::{self, DocFolder};
use crate::passes::Pass;

use rustc_span::symbol::sym;
use rustc_span::FileName;
use serde::Serialize;

use std::collections::BTreeMap;
use std::ops;

pub const CALCULATE_DOC_COVERAGE: Pass = Pass {
    name: "calculate-doc-coverage",
    run: calculate_doc_coverage,
    description: "counts the number of items with and without documentation",
};

fn calculate_doc_coverage(krate: clean::Crate, ctx: &DocContext<'_>) -> clean::Crate {
    let mut calc = CoverageCalculator::new();
    let krate = calc.fold_crate(krate);

    calc.print_results(ctx.renderinfo.borrow().output_format);

    krate
}

#[derive(Default, Copy, Clone, Serialize)]
struct ItemCount {
    total: u64,
    with_docs: u64,
}

impl ItemCount {
    fn count_item(&mut self, has_docs: bool) {
        self.total += 1;

        if has_docs {
            self.with_docs += 1;
        }
    }

    fn percentage(&self) -> Option<f64> {
        if self.total > 0 {
            Some((self.with_docs as f64 * 100.0) / self.total as f64)
        } else {
            None
        }
    }
}

impl ops::Sub for ItemCount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        ItemCount { total: self.total - rhs.total, with_docs: self.with_docs - rhs.with_docs }
    }
}

impl ops::AddAssign for ItemCount {
    fn add_assign(&mut self, rhs: Self) {
        self.total += rhs.total;
        self.with_docs += rhs.with_docs;
    }
}

struct CoverageCalculator {
    items: BTreeMap<FileName, ItemCount>,
}

fn limit_filename_len(filename: String) -> String {
    let nb_chars = filename.chars().count();
    if nb_chars > 35 {
        "...".to_string()
            + &filename[filename.char_indices().nth(nb_chars - 32).map(|x| x.0).unwrap_or(0)..]
    } else {
        filename
    }
}

impl CoverageCalculator {
    fn new() -> CoverageCalculator {
        CoverageCalculator { items: Default::default() }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(
            &self
                .items
                .iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect::<BTreeMap<String, &ItemCount>>(),
        )
        .expect("failed to convert JSON data to string")
    }

    fn print_results(&self, output_format: Option<OutputFormat>) {
        if output_format.map(|o| o.is_json()).unwrap_or_else(|| false) {
            println!("{}", self.to_json());
            return;
        }
        let mut total = ItemCount::default();

        fn print_table_line() {
            println!("+-{0:->35}-+-{0:->10}-+-{0:->10}-+-{0:->10}-+", "");
        }

        fn print_table_record(name: &str, count: ItemCount, percentage: f64) {
            println!(
                "| {:<35} | {:>10} | {:>10} | {:>9.1}% |",
                name, count.with_docs, count.total, percentage
            );
        }

        print_table_line();
        println!(
            "| {:<35} | {:>10} | {:>10} | {:>10} |",
            "File", "Documented", "Total", "Percentage"
        );
        print_table_line();

        for (file, &count) in &self.items {
            if let Some(percentage) = count.percentage() {
                print_table_record(&limit_filename_len(file.to_string()), count, percentage);

                total += count;
            }
        }

        print_table_line();
        print_table_record("Total", total, total.percentage().unwrap_or(0.0));
        print_table_line();
    }
}

impl fold::DocFolder for CoverageCalculator {
    fn fold_item(&mut self, i: clean::Item) -> Option<clean::Item> {
        let has_docs = !i.attrs.doc_strings.is_empty();

        match i.inner {
            _ if !i.def_id.is_local() => {
                // non-local items are skipped because they can be out of the users control,
                // especially in the case of trait impls, which rustdoc eagerly inlines
                return Some(i);
            }
            clean::StrippedItem(..) => {
                // don't count items in stripped modules
                return Some(i);
            }
            clean::ImportItem(..) | clean::ExternCrateItem(..) => {
                // docs on `use` and `extern crate` statements are not displayed, so they're not
                // worth counting
                return Some(i);
            }
            clean::ImplItem(ref impl_)
                if i.attrs
                    .other_attrs
                    .iter()
                    .any(|item| item.has_name(sym::automatically_derived))
                    || impl_.synthetic
                    || impl_.blanket_impl.is_some() =>
            {
                // built-in derives get the `#[automatically_derived]` attribute, and
                // synthetic/blanket impls are made up by rustdoc and can't be documented
                // FIXME(misdreavus): need to also find items that came out of a derive macro
                return Some(i);
            }
            clean::ImplItem(ref impl_) => {
                if let Some(ref tr) = impl_.trait_ {
                    debug!(
                        "impl {:#} for {:#} in {}",
                        tr.print(),
                        impl_.for_.print(),
                        i.source.filename
                    );

                    // don't count trait impls, the missing-docs lint doesn't so we shouldn't
                    // either
                    return Some(i);
                } else {
                    // inherent impls *can* be documented, and those docs show up, but in most
                    // cases it doesn't make sense, as all methods on a type are in one single
                    // impl block
                    debug!("impl {:#} in {}", impl_.for_.print(), i.source.filename);
                }
            }
            _ => {
                debug!("counting {:?} {:?} in {}", i.type_(), i.name, i.source.filename);
                self.items.entry(i.source.filename.clone()).or_default().count_item(has_docs);
            }
        }

        self.fold_item_recur(i)
    }
}
