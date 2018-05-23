use libraries::LibraryDependencies;

use std::collections::{HashMap, HashSet};

//const TYPE_NOTYPE: u8 = 0;
//const TYPE_OBJECT: u8 = 1;
//const TYPE_FUNC: u8 = 2;
//const TYPE_SECTION: u8 = 3;
//const TYPE_FILE: u8 = 4;

//const BIND_LOCAL: u8 = 0;
const BIND_GLOBAL: u8 = 1;
//const BIND_WEAK: u8 = 2;

const NDX_UNDEFINED: usize = 0;
//const NDX_ABS: usize = 65521;

//const VIS_DEFAULT: u8 = 0;
const VIS_HIDDEN: u8 = 2;

pub struct SymbolSummary {
    pub exported: HashMap<String, HashSet<String>>,
    pub unresolved: HashMap<String, HashSet<String>>,
    pub defined: HashMap<String, HashSet<String>>,
}

impl SymbolSummary {
    pub fn from_libs(libs: &LibraryDependencies) -> SymbolSummary {
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

                    if !name.is_empty() && sym.st_bind() == BIND_GLOBAL
                        && sym.st_other != VIS_HIDDEN
                        && sym.st_shndx != NDX_UNDEFINED
                    {
                        let entry = summary
                            .exported
                            .entry(name.to_string())
                            .or_insert(HashSet::new());
                        let _ = entry.insert(lib_name.to_string_lossy().to_string());
                    }
                    if !name.is_empty() && sym.st_shndx == NDX_UNDEFINED {
                        let entry = summary
                            .unresolved
                            .entry(name.to_string())
                            .or_insert(HashSet::new());
                        let _ = entry.insert(lib_name.to_string_lossy().to_string());
                    }
                    if !name.is_empty() && sym.st_shndx != NDX_UNDEFINED {
                        let entry = summary
                            .defined
                            .entry(name.to_string())
                            .or_insert(HashSet::new());
                        let _ = entry.insert(lib_name.to_string_lossy().to_string());
                    }
                }
            }
        }
        summary
    }
}
