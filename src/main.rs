#[macro_use] extern crate structopt;
extern crate goblin;
extern crate cpp_demangle;
extern crate itertools;
extern crate groupable;
extern crate term;

use cpp_demangle::Symbol;

use structopt::StructOpt;
use std::fs;
use std::path::{Path, PathBuf};
use std::fmt;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::collections::{HashMap, HashSet};
use itertools::Itertools;
use groupable::Groupable;

use goblin::{strtab::Strtab, elf::{Elf, Sym}};

const TYPE_NOTYPE: u8 = 0;
const TYPE_OBJECT: u8 = 1;
const TYPE_FUNC: u8 = 2;
const TYPE_SECTION: u8 = 3;
const TYPE_FILE: u8 = 4;
fn format_type(sym: &Sym) -> String {
    match sym.st_type() {
        TYPE_NOTYPE => "NOTYPE".to_owned(),
        TYPE_OBJECT => "OBJECT".to_owned(),
        TYPE_FUNC => "FUNC".to_owned(),
        TYPE_SECTION => "SECTION".to_owned(),
        TYPE_FILE => "FILE".to_owned(),
        o => format!("???{}", o),
    }
}

const BIND_LOCAL: u8 = 0;
const BIND_GLOBAL: u8 = 1;
const BIND_WEAK: u8 = 2;
fn format_bind(sym: &Sym) -> String {
    match sym.st_bind() {
        BIND_LOCAL => "LOCAL".to_owned(),
        BIND_GLOBAL => "GLOBAL".to_owned(),
        BIND_WEAK => "WEAK".to_owned(),
        o => format!("???{}", o),
    }
}

const NDX_UNDEFINED: usize = 0;
const NDX_ABS: usize = 65521;
fn format_ndx(sym: &Sym) -> String {
    match sym.st_shndx {
        NDX_UNDEFINED => "UND".to_owned(),
        NDX_ABS => "ABS".to_owned(),
        o => format!("{}", o),
    }
}

const VIS_DEFAULT: u8 = 0;
const VIS_HIDDEN: u8 = 2;
fn format_vis(sym: &Sym) -> String {
    match sym.st_other {
        VIS_DEFAULT => "DEFAULT".to_owned(),
        VIS_HIDDEN => "HIDDEN".to_owned(),
        o => format!("???{}", o),
    }
}

#[allow(unused)]
struct PrintSymbol<'a, 'b: 'a>(&'b Strtab<'b>, &'a Sym);
impl<'a, 'b: 'a> fmt::Display for PrintSymbol<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let strtab = self.0;
        let sym = self.1;
        write!(f, "{value:016x}\t{size}\t{type}\t{bind}\t{vis}\t{ndx}\t{name}",
                 value=sym.st_value,
                 size=sym.st_size,
                 type=format_type(&sym),
                 bind=format_bind(&sym),
                 vis=format_vis(&sym),
                 ndx=format_ndx(&sym),
                 name=strtab.get(sym.st_name).unwrap_or(Ok("")).unwrap())
    }
}

#[derive(Debug)]
struct DynInfo<'a> {
    rpath: Vec<&'a str>,
    runpath: Vec<&'a str>,
    libs: Vec<&'a str>,
}

const LIBS_D_TAG: u64 = 1;
const RPATH_D_TAG: u64 = 15;
const RUNPATH_D_TAG: u64 = 29;

impl<'a> DynInfo<'a> {
    fn new() -> Self {
        DynInfo {
            rpath: Vec::new(),
            runpath: Vec::new(),
            libs: Vec::new(),
        }
    }

    fn from_elf(elf: &'a Elf) -> Option<Self> {
        if let Some(ref dynamic) = &elf.dynamic {
            let mut dyninfo = DynInfo::new();
            for dyn in dynamic.dyns.iter() {
                match dyn.d_tag {
                    RPATH_D_TAG => {
                        let rpath_str = elf.dynstrtab.get(dyn.d_val as usize).expect("RPATH should be in string table").expect("rpath must be utf8");
                        dyninfo.rpath.extend(rpath_str.split(":"))
                    }
                    RUNPATH_D_TAG => {
                        let runpath_str = elf.dynstrtab.get(dyn.d_val as usize).expect("RUNPATH should be in string table").expect("runpath must be utf8");
                        dyninfo.runpath.extend(runpath_str.split(":"))
                    }
                    LIBS_D_TAG => {
                        let lib_str = elf.dynstrtab.get(dyn.d_val as usize).expect("RPATH should be in string table").expect("lib must be utf8");
                        dyninfo.libs.push(lib_str)
                    }
                    _ => {},
                }
            }
            Some(dyninfo)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
struct LibraryLocations(Vec<(PathBuf, &'static str)>);

impl LibraryLocations {
    fn try_find_library(&self, lib_name: &str) -> Option<PathBuf> {
        self.0.iter().filter_map(|(dir, _)| {
            let potential_lib_path = dir.join(lib_name);
            if potential_lib_path.exists() {
                Some(potential_lib_path)
            } else {
                None
            }
        }).next()
    }
}

impl fmt::Display for LibraryLocations {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[")?;
        for (lib, origin) in self.0.iter() {
            writeln!(f, "\t{:?} ({})", lib, origin)?;
        }
        writeln!(f, "]")
    }
}

#[derive(Debug)]
enum LibSearchMethod {
    RPath,
    RunPath,
    LDLibraryPath,
    LDConfig(PathBuf),
    Fixed(PathBuf),
}

fn gnuld_search_methods() -> Vec<LibSearchMethod> {
    vec![
        LibSearchMethod::RPath,
        LibSearchMethod::RunPath,
        LibSearchMethod::LDLibraryPath,
        LibSearchMethod::LDConfig(PathBuf::from("/etc/ld.so.conf")),
        LibSearchMethod::Fixed(PathBuf::from("/usr/lib")),
        LibSearchMethod::Fixed(PathBuf::from("/lib")),
    ]
}

#[derive(Debug)]
enum NoError { }

impl ::std::string::ToString for NoError {
    fn to_string(&self) -> String {
        panic!("cannot create NoError");
    }
}

impl ::std::str::FromStr for LibSearchMethod {
    type Err = NoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const LD_CONFIG_PREFIX: & 'static str = "ldconfig:";
        Ok(match s {
            "rpath" => LibSearchMethod::RPath,
            "runpath" => LibSearchMethod::RunPath,
            "ld_library_path" => LibSearchMethod::LDLibraryPath,
            other => if other.starts_with(LD_CONFIG_PREFIX) {
                LibSearchMethod::LDConfig(PathBuf::from(other[LD_CONFIG_PREFIX.len()..].to_owned()))
            } else {
                LibSearchMethod::Fixed(PathBuf::from(other))
            },
        })
    }
}

struct Libraries {
    resolved: HashMap<OsString, PathBuf>,
    opened_libs: HashMap<PathBuf, Library>,
    reverse_dependencies: HashMap<PathBuf, Vec<PathBuf>>,
    problems: Vec<LibResolveProblem>,
}

impl Libraries {
    fn new() -> Self {
        Libraries {
            resolved: HashMap::new(),
            opened_libs: HashMap::new(),
            reverse_dependencies: HashMap::new(),
            problems: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct Library {
    path: PathBuf,
    bytes: Vec<u8>, //Invariant: Valid ELF!
}

impl Library {
    fn try_from_path(path: PathBuf) -> Result<Self, Box<Error>> {
        let bytes = fs::read(&path)?;

        // Try once to see if it's a valid Elf file, but we do not actually use it here
        {
            let _elf = Elf::parse(&bytes)?;
        }

        Ok(Library {
            path: path,
            bytes: bytes,
        })
    }

    fn get_name(&self) -> &OsStr {
        self.path.file_name().expect("Cannot be empty because we read from the file")
    }

    fn get_elf<'a>(&'a self) -> Elf<'a> {
        Elf::parse(&self.bytes).expect("Invariant: Valid ELF")
    }
}

enum LibResolveProblem {
    Unresolved {
        dependent_lib: PathBuf,
        lib_name: String,
        locations: LibraryLocations,
    },
    UnresolvedButPreviouslyResolved {
        dependent_lib: PathBuf,
        lib_name: String,
        locations: LibraryLocations,
        prev_resolved_path: PathBuf,
        first_resolver: PathBuf,
    },
    ResolveConflict {
        dependent_lib: PathBuf,
        lib_name: String,
        resolve_path: PathBuf,
        locations: LibraryLocations,
        prev_resolved_path: PathBuf,
        first_resolver: PathBuf,
    },
}

impl fmt::Display for LibResolveProblem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &LibResolveProblem::Unresolved {
                ref dependent_lib,
                ref lib_name,
                ref locations,
            } => {
                write!(f, "{:?}: Could not resolve dependency to library {:?}. Search locations are: {}",
                       dependent_lib,
                       lib_name,
                       locations)
            },
            &LibResolveProblem::UnresolvedButPreviouslyResolved {
                ref dependent_lib,
                ref lib_name,
                ref locations,
                ref prev_resolved_path,
                ref first_resolver,
            } => {
                write!(f, "{:?}: Could not resolve dependency {:?}, but it is already resolved to {:?} by {:?}. Search locations are: {}",
                       dependent_lib,
                       lib_name,
                       prev_resolved_path,
                       first_resolver,
                       locations)
            },
            &LibResolveProblem::ResolveConflict {
                ref dependent_lib,
                ref lib_name,
                ref resolve_path,
                ref locations,
                ref prev_resolved_path,
                ref first_resolver,
            } => {
                write!(f, "{:?}: Would resolve dependency {:?} to {:?}, but it is already resolved to {:?} by {:?}. Search locations are: {}",
                       dependent_lib,
                       lib_name,
                       resolve_path,
                       prev_resolved_path,
                       first_resolver,
                       locations)
            },
        }
    }
}


fn find_dependencies(lib_path: &Path, search_methods: &[LibSearchMethod]) -> Result<Libraries, Box<Error>> {
    let mut result = Libraries::new();
    collect_libs(lib_path, search_methods, None, &mut result)?;
    Ok(result)
}

fn collect_libs(lib_path: &Path, search_methods: &[LibSearchMethod], reverse_dependency: Option<PathBuf>, result: &mut Libraries) -> Result<(), Box<Error>> {

    let new_lib_paths = {

        if result.opened_libs.get(lib_path).is_some() {
            // Lib already analyzed
            return Ok(());
        }

        let lib = result.opened_libs.entry(lib_path.to_path_buf()).or_insert(Library::try_from_path(lib_path.to_owned())?);

        let lib_name = lib.get_name();

        if result.resolved.get(lib_name).is_none() {
            let _ = result.resolved.insert(lib_name.to_owned(), lib_path.to_path_buf());
        }

        if let Some(reverse_dependency) = reverse_dependency {
            let res = result.reverse_dependencies.insert(lib_path.to_path_buf(), vec![reverse_dependency]);
            assert!(res.is_none(), "Overwrote reverse dependency entry");
        }

        let elf = lib.get_elf();

        let dyninfo = DynInfo::from_elf(&elf).expect("file has no dyninfo");

        let origin = lib_path.parent().unwrap_or(Path::new("/")).to_str().expect("Path not valid utf8");

        let mut lib_locations = LibraryLocations(Vec::new());

        for method in search_methods.iter() {
            match method {
                LibSearchMethod::RPath => {
                    lib_locations.0.extend(dyninfo.rpath.iter().map(|path| (PathBuf::from(path.replace("$ORIGIN", origin).to_owned()), "rpath")))
                },
                LibSearchMethod::RunPath => {
                    lib_locations.0.extend(dyninfo.runpath.iter().map(|path| (PathBuf::from(path.replace("$ORIGIN", origin).to_owned()), "runpath")))
                },
                LibSearchMethod::LDLibraryPath => {
                    if let Some(ld_lib_path) = ::std::env::var_os("LD_LIBRARY_PATH") {
                        use ::std::os::unix::ffi::OsStrExt;
                        lib_locations.0.extend(ld_lib_path.as_bytes().split(|b| *b == b':').map(|slice| (PathBuf::from(OsStr::from_bytes(slice)), "LD_LIBRARY_PATH")))
                    }
                },
                LibSearchMethod::LDConfig(_cache_file) => {
                    //TODO
                },
                LibSearchMethod::Fixed(p) => {
                    lib_locations.0.push((p.clone(), "fixed"));
                },
            }
        }

        let resolved = &mut result.resolved;
        let reverse_dependencies = &mut result.reverse_dependencies;
        let problems = &mut result.problems;

        dyninfo.libs.iter().filter_map(|&dependency_lib_name| {

            let dependency_lib_path = lib_locations.try_find_library(dependency_lib_name);

            let os_dep_lib_name = OsString::from(dependency_lib_name);

            let maybe_resolved_lib_path = { resolved.get(&os_dep_lib_name).map(|p| p.to_path_buf()) };
            if let Some(resolved_lib_path) = maybe_resolved_lib_path {
                let reverse_dependencies = reverse_dependencies.get_mut(&resolved_lib_path).unwrap();
                if let Some(ref dependency_lib_path) = &dependency_lib_path {
                    if *dependency_lib_path != resolved_lib_path {
                        problems.push(LibResolveProblem::ResolveConflict {
                            dependent_lib: lib_path.to_path_buf(),
                            lib_name: dependency_lib_name.to_owned(),
                            resolve_path: dependency_lib_path.to_path_buf(),
                            locations: lib_locations.clone(),
                            prev_resolved_path: resolved_lib_path,
                            first_resolver: reverse_dependencies.first().expect("Already resolved means that there is at least one reverse dependency").to_path_buf(),
                        });
                    } else {
                        reverse_dependencies.push(lib_path.to_path_buf());
                    }
                } else {
                    problems.push(LibResolveProblem::UnresolvedButPreviouslyResolved {
                        dependent_lib: lib_path.to_path_buf(),
                        lib_name: dependency_lib_name.to_owned(),
                        locations: lib_locations.clone(),
                        prev_resolved_path: resolved_lib_path.to_path_buf(),
                        first_resolver: reverse_dependencies.first().expect("Already resolved means that there is at least one reverse dependency").to_path_buf(),
                    });
                }

                None
            } else {
                if dependency_lib_path.is_none() {
                    problems.push(LibResolveProblem::Unresolved {
                        dependent_lib: lib_path.to_path_buf(),
                        lib_name: dependency_lib_name.to_owned(),
                        locations: lib_locations.clone(),
                    });
                }

                dependency_lib_path
            }
        }).collect::<Vec<_>>()
    };

    for path in new_lib_paths {
        collect_libs(&path, search_methods, Some(lib_path.to_path_buf()), result)?;
    }

    Ok(())
}

struct SymbolSummary {
    exported: HashMap<String, HashSet<String>>,
    unresolved: HashMap<String, HashSet<String>>,
    defined: HashMap<String, HashSet<String>>,
}

fn get_symbol_summary(libs: &Libraries) -> SymbolSummary {

    let mut summary = SymbolSummary {
        exported: HashMap::new(),
        unresolved: HashMap::new(),
        defined: HashMap::new(),
    };
    for (lib_name, lib_path) in libs.resolved.iter() {
        let elf = libs.opened_libs.get(lib_path).unwrap().get_elf();
        for sym in elf.dynsyms.iter() {
            if let Some(name) = elf.dynstrtab.get(sym.st_name) {
                let name = name.expect("Symbol is not valid utf8");

                if !name.is_empty() && sym.st_bind() == BIND_GLOBAL && sym.st_other != VIS_HIDDEN && sym.st_shndx != NDX_UNDEFINED {
                    let entry = summary.exported.entry(name.to_string()).or_insert(HashSet::new());
                    let _ = entry.insert(lib_name.to_string_lossy().to_string());
                }
                if !name.is_empty() && sym.st_shndx == NDX_UNDEFINED {
                    let entry = summary.unresolved.entry(name.to_string()).or_insert(HashSet::new());
                    let _ = entry.insert(lib_name.to_string_lossy().to_string());
                }
                if !name.is_empty() && sym.st_shndx != NDX_UNDEFINED {
                    let entry = summary.defined.entry(name.to_string()).or_insert(HashSet::new());
                    let _ = entry.insert(lib_name.to_string_lossy().to_string());
                }
            }
        }
    }
    summary
}

/// Command line options
#[derive(Debug, StructOpt)]
struct Options {

    /// ELF-file to be analysed
    #[structopt(parse(from_os_str))]
    file: PathBuf,

    /// Library search locations (in order specified)
    #[structopt(short="l", long="lib")]
    search_methods: Vec<LibSearchMethod>,
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


fn main() -> Result<(), Box<Error>> {
    let options = Options::from_args();

    let search_methods = if options.search_methods.is_empty() {
        eprintln!("No search location specified. Assuming default locations for GNU ld");
        gnuld_search_methods()
    } else {
        options.search_methods
    };

    let libs = find_dependencies(&options.file, &search_methods)?;

    let symbol_summary = get_symbol_summary(&libs);

    let duplicate_groups = symbol_summary.exported.iter()
        .filter(|(symbol, libs)| libs.len() >= 2 && symbol_summary.unresolved.get(symbol.as_str()).is_some() )
        .map(|(symbol, libs)| (libs_to_key(libs), symbol))
        .group::<HashMap<_, Vec<_>>>();

    let unresolved_groups = symbol_summary.unresolved.iter()
        .filter(|(symbol, libs)| libs.len() >= 1 && symbol_summary.defined.get(symbol.as_str()).is_none() )
        .map(|(symbol, libs)| (libs_to_key(libs), symbol))
        .group::<HashMap<_, Vec<_>>>();

    let mut t = term::stdout().unwrap();


    if !libs.problems.is_empty() {
        t.fg(term::color::RED).unwrap();
        t.attr(term::Attr::Bold).unwrap();
        writeln!(t, "Library resolving problems:").unwrap();
        t.reset().unwrap();

        for problem in libs.problems.iter() {
            writeln!(t, "\t{}", problem).unwrap();
        }
    }


    if !unresolved_groups.is_empty() {
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

    if !duplicate_groups.is_empty() {
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
