use std::fs;
use std::path::{Path, PathBuf};
use std::fmt;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::collections::{HashMap};

use goblin::elf::Elf;

const LIBS_D_TAG: u64 = 1;
const RPATH_D_TAG: u64 = 15;
const RUNPATH_D_TAG: u64 = 29;

#[derive(Debug)]
struct DynInfo<'a> {
    rpath: Vec<&'a str>,
    runpath: Vec<&'a str>,
    libs: Vec<&'a str>,
}

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
pub struct LibraryLocations(Vec<(PathBuf, &'static str)>);

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
pub enum LibSearchMethod {
    RPath,
    RunPath,
    LDLibraryPath,
    LDConfig(PathBuf),
    Fixed(PathBuf),
}
#[derive(Debug)]
pub enum NoError { }

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
#[derive(Debug)]
pub struct Library {
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

    pub fn get_elf<'a>(&'a self) -> Elf<'a> {
        Elf::parse(&self.bytes).expect("Invariant: Valid ELF")
    }
}

pub enum LibResolveProblem {
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

pub struct LibraryDependencies {
    pub opened_libs: HashMap<PathBuf, Library>, // Libraries that have been opened and analyzed
    pub resolved: HashMap<OsString, PathBuf>, // A map that shows how librarynames (e.g., libfoo.so) map to actual files (e.g., /usr/local/lib/libfoo.so)
    pub reverse_dependencies: HashMap<PathBuf, Vec<PathBuf>>, // Mapping resolved libraries (paths!) to those libraries (paths!) that depend on them
    pub problems: Vec<LibResolveProblem>, // Collection of all problems that appeared while resolving dependency tree
}

impl LibraryDependencies {
    pub fn try_find_for_elf(elf_path: &Path, search_methods: &[LibSearchMethod]) -> Result<LibraryDependencies, Box<Error>> {
        let mut result = LibraryDependencies {
            resolved: HashMap::new(),
            opened_libs: HashMap::new(),
            reverse_dependencies: HashMap::new(),
            problems: Vec::new(),
        };
        collect_libs(elf_path, search_methods, None, &mut result)?;
        Ok(result)
    }
}

fn collect_libs(lib_path: &Path, search_methods: &[LibSearchMethod], reverse_dependency: Option<PathBuf>, result: &mut LibraryDependencies) -> Result<(), Box<Error>> {

    // Collect the paths to all libraries that the current library (i.e., libpath) depends on
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

    // Call collect_libs for all of those libraries.
    // This way, the library resolution is performed depth first.
    for path in new_lib_paths {
        collect_libs(&path, search_methods, Some(lib_path.to_path_buf()), result)?;
    }

    Ok(())
}
