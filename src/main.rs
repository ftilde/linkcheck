#[macro_use] extern crate structopt;
extern crate goblin;
extern crate cpp_demangle;
extern crate itertools;
extern crate groupable;
extern crate term;
extern crate glob;

use cpp_demangle::Symbol;

mod libraries;
mod symbols;

use symbols::*;
use libraries::*;

use structopt::StructOpt;
use std::path::{PathBuf};
use std::error::Error;
use std::collections::{HashMap, HashSet};
use itertools::Itertools;
use groupable::Groupable;

/// The methods which GNU ld.so uses (if not specified otherwise) to locate libraries. At least
/// according to https://en.wikipedia.org/wiki/Rpath
fn gnuld_default_search_methods() -> Vec<LibSearchMethod> {
    vec![
        LibSearchMethod::RPath,
        LibSearchMethod::RunPath,
        LibSearchMethod::LDLibraryPath,
        LibSearchMethod::LDConfig(PathBuf::from("/etc/ld.so.conf")),
        LibSearchMethod::Fixed(PathBuf::from("/usr/lib")),
        LibSearchMethod::Fixed(PathBuf::from("/lib")),
    ]
}


/// Show potential dynamic linking problems of ELF files.
#[derive(Debug, StructOpt)]
struct Options {
    /// Library search locations (in order specified). Special options are: rpath, runpath,
    /// ld_library_path, ldconfig:<path_to_ld.so.conf>. All other options are interpreted as fixed
    /// paths to library locations. If nothing is specified, the default resolution behavior of GNU
    /// ld.so is mimicked.
    #[structopt(short="l", long="lib")]
    search_methods: Vec<LibSearchMethod>,

    /// Show unresolved symbols
    #[structopt(short="u", long="unresolved-symbols")]
    show_unresolved_symbols: bool,

    /// Show used duplicate symbols
    #[structopt(short="d", long="duplicate-symbols")]
    show_duplicate_symbols: bool,

    /// Show library resolution problems
    #[structopt(short="r", long="lib-resolution")]
    show_lib_resolution_problems: bool,

    /// Perform full analysis (default if neither -u, -d, nor -r are specified)
    #[structopt(short="f", long="full analysis")]
    full_analysis: bool,

    /// ELF file to be analyzed
    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

fn libs_to_key(lib_names: &HashSet<String>) -> String {
    let mut libs = lib_names.iter().collect::<Vec<_>>();
    libs.sort();
    libs.iter().map(|s| s.to_string()).join(", ")
}

fn symbols_to_key(symbols: &[&String]) -> String {
    let mut pretty_symbols = symbols.iter().map(|symbol| {
        if let Ok(dsym) = Symbol::new(&symbol) {
            dsym.to_string()
        } else {
            symbol.to_string()
        }
    }).collect::<Vec<_>>();
    pretty_symbols.sort();
    pretty_symbols.join(", ")
}


fn run(mut options: Options) -> Result<(), Box<Error>> {

    let search_methods = if options.search_methods.is_empty() {
        eprintln!("No search location specified. Assuming default locations for GNU ld");
        gnuld_default_search_methods()
    } else {
        options.search_methods
    };

    if !options.show_duplicate_symbols && !options.show_unresolved_symbols && !options.show_lib_resolution_problems || options.full_analysis {
        options.show_duplicate_symbols = true;
        options.show_unresolved_symbols = true;
        options.show_lib_resolution_problems = true;
    }

    let libs = LibraryDependencies::try_find_for_elf(&options.file, &search_methods)?;

    let symbol_summary = SymbolSummary::from_libs(&libs);

    let duplicate_groups = symbol_summary.exported.iter()
        .filter(|(symbol, libs)| libs.len() >= 2 && symbol_summary.unresolved.get(symbol.as_str()).is_some() )
        .map(|(symbol, libs)| (libs_to_key(libs), symbol))
        .group::<HashMap<_, Vec<_>>>();

    let unresolved_groups = symbol_summary.unresolved.iter()
        .filter(|(symbol, libs)| libs.len() >= 1 && symbol_summary.defined.get(symbol.as_str()).is_none() )
        .map(|(symbol, libs)| (libs_to_key(libs), symbol))
        .group::<HashMap<_, Vec<_>>>();

    let mut t = term::stdout().unwrap();


    if options.show_lib_resolution_problems && !libs.problems.is_empty() {
        t.fg(term::color::RED).unwrap();
        t.attr(term::Attr::Bold).unwrap();
        writeln!(t, "Library resolving problems:").unwrap();
        t.reset().unwrap();

        for problem in libs.problems.iter() {
            writeln!(t, "\t{}", problem).unwrap();
        }
    }


    if options.show_unresolved_symbols && !unresolved_groups.is_empty() {
        t.fg(term::color::RED).unwrap();
        t.attr(term::Attr::Bold).unwrap();
        writeln!(t, "Unresolved symbols:").unwrap();
        t.reset().unwrap();

        for (libs, unresolved_symbols) in unresolved_groups {
            t.attr(term::Attr::Bold).unwrap();
            write!(t, "\t{}:", libs).unwrap();
            t.reset().unwrap();
            writeln!(t, " [{}]\n", symbols_to_key(unresolved_symbols.as_slice())).unwrap();
        }
    }

    if options.show_duplicate_symbols && !duplicate_groups.is_empty() {
        t.fg(term::color::RED).unwrap();
        t.attr(term::Attr::Bold).unwrap();
        writeln!(t, "Exported duplicate symbols:").unwrap();
        t.reset().unwrap();

        for (libs, duplicate_symbols) in duplicate_groups {
            t.attr(term::Attr::Bold).unwrap();
            write!(t, "\t{}:", libs).unwrap();
            t.reset().unwrap();
            writeln!(t, " [{}]\n", symbols_to_key(duplicate_symbols.as_slice())).unwrap();
        }
    }

    Ok(())
}


fn main() {
    let options = Options::from_args();
    if let Err(err) = run(options) {
        println!("{}", err);
    }
}
