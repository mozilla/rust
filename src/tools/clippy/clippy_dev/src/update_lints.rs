use crate::{
    gather_all, gen_changelog_lint_list, gen_deprecated, gen_lint_group_list, gen_modules_list, gen_register_lint_list,
    replace_region_in_file, Lint, DOCS_LINK,
};
use std::path::Path;

#[derive(Clone, Copy, PartialEq)]
pub enum UpdateMode {
    Check,
    Change,
}

#[allow(clippy::too_many_lines)]
pub fn run(update_mode: UpdateMode) {
    let lint_list: Vec<Lint> = gather_all().collect();

    let internal_lints = Lint::internal_lints(lint_list.clone().into_iter());

    let usable_lints: Vec<Lint> = Lint::usable_lints(lint_list.clone().into_iter()).collect();
    let usable_lint_count = round_to_fifty(usable_lints.len());

    let mut sorted_usable_lints = usable_lints.clone();
    sorted_usable_lints.sort_by_key(|lint| lint.name.clone());

    let mut file_change = replace_region_in_file(
        Path::new("src/lintlist/mod.rs"),
        "begin lint list",
        "end lint list",
        false,
        update_mode == UpdateMode::Change,
        || {
            format!("pub static ref ALL_LINTS: Vec<Lint> = vec!{:#?};", sorted_usable_lints)
                .lines()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        },
    )
    .changed;

    file_change |= replace_region_in_file(
        Path::new("README.md"),
        &format!(
            r#"\[There are over \d+ lints included in this crate!\]\({}\)"#,
            DOCS_LINK
        ),
        "",
        true,
        update_mode == UpdateMode::Change,
        || {
            vec![format!(
                "[There are over {} lints included in this crate!]({})",
                usable_lint_count, DOCS_LINK
            )]
        },
    )
    .changed;

    file_change |= replace_region_in_file(
        Path::new("CHANGELOG.md"),
        "<!-- begin autogenerated links to lint list -->",
        "<!-- end autogenerated links to lint list -->",
        false,
        update_mode == UpdateMode::Change,
        || gen_changelog_lint_list(lint_list.clone()),
    )
    .changed;

    file_change |= replace_region_in_file(
        Path::new("clippy_lints/src/lib.rs"),
        "begin deprecated lints",
        "end deprecated lints",
        false,
        update_mode == UpdateMode::Change,
        || gen_deprecated(&lint_list),
    )
    .changed;

    file_change |= replace_region_in_file(
        Path::new("clippy_lints/src/lib.rs"),
        "begin register lints",
        "end register lints",
        false,
        update_mode == UpdateMode::Change,
        || gen_register_lint_list(&lint_list),
    )
    .changed;

    file_change |= replace_region_in_file(
        Path::new("clippy_lints/src/lib.rs"),
        "begin lints modules",
        "end lints modules",
        false,
        update_mode == UpdateMode::Change,
        || gen_modules_list(lint_list.clone()),
    )
    .changed;

    // Generate lists of lints in the clippy::all lint group
    file_change |= replace_region_in_file(
        Path::new("clippy_lints/src/lib.rs"),
        r#"store.register_group\(true, "clippy::all""#,
        r#"\]\);"#,
        false,
        update_mode == UpdateMode::Change,
        || {
            // clippy::all should only include the following lint groups:
            let all_group_lints = usable_lints
                .clone()
                .into_iter()
                .filter(|l| {
                    l.group == "correctness" || l.group == "style" || l.group == "complexity" || l.group == "perf"
                })
                .collect();

            gen_lint_group_list(all_group_lints)
        },
    )
    .changed;

    // Generate the list of lints for all other lint groups
    for (lint_group, lints) in Lint::by_lint_group(usable_lints.into_iter().chain(internal_lints)) {
        file_change |= replace_region_in_file(
            Path::new("clippy_lints/src/lib.rs"),
            &format!("store.register_group\\(true, \"clippy::{}\"", lint_group),
            r#"\]\);"#,
            false,
            update_mode == UpdateMode::Change,
            || gen_lint_group_list(lints.clone()),
        )
        .changed;
    }

    if update_mode == UpdateMode::Check && file_change {
        println!(
            "Not all lints defined properly. \
             Please run `cargo dev update_lints` to make sure all lints are defined properly."
        );
        std::process::exit(1);
    }
}

pub fn print_lints() {
    let lint_list = gather_all();
    let usable_lints: Vec<Lint> = Lint::usable_lints(lint_list).collect();
    let usable_lint_count = usable_lints.len();
    let grouped_by_lint_group = Lint::by_lint_group(usable_lints.into_iter());

    for (lint_group, mut lints) in grouped_by_lint_group {
        if lint_group == "Deprecated" {
            continue;
        }
        println!("\n## {}", lint_group);

        lints.sort_by_key(|l| l.name.clone());

        for lint in lints {
            println!("* [{}]({}#{}) ({})", lint.name, DOCS_LINK, lint.name, lint.desc);
        }
    }

    println!("there are {} lints", usable_lint_count);
}

fn round_to_fifty(count: usize) -> usize {
    count / 50 * 50
}
