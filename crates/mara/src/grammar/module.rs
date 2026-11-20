//! The exports of a grammar's WebAssembly module, checked before tree-sitter's store sees it:
//! the store asserts, and so ends the process, when the function that returns the language, or
//! an initializer it calls, has another signature (DES-GRAMMAR-ASSETS).

/// The initializers that tree-sitter's store calls without arguments or results.
const INITIALIZERS: &[&str] = &[
    "_initialize",
    "__wasm_apply_data_relocs",
    "__wasm_call_ctors",
];

const I32: u8 = 0x7F;

/// A function type: its parameter and result value types.
type FunctionType = (Vec<u8>, Vec<u8>);

/// Requires that `wasm` is a WebAssembly module that exports `symbol` as a function without
/// parameters returning an `i32`, and each initializer it exports as a function without
/// parameters or results. A module this reader cannot follow is refused.
pub fn check(wasm: &[u8], symbol: &str) -> Result<(), String> {
    let module = Module::read(wasm).ok_or("its module cannot be read")?;
    let found = module.function(symbol);
    let language = found.ok_or_else(|| format!("it exports no function `{symbol}`"))?;
    if *language != (vec![], vec![I32]) {
        return Err(format!("`{symbol}` does not return a language"));
    }
    for name in INITIALIZERS {
        if module.exports.iter().any(|(export, ..)| export == name)
            && module.function(name) != Some(&(vec![], vec![]))
        {
            return Err(format!("`{name}` is no initializer"));
        }
    }
    Ok(())
}

/// What the check needs of a module.
#[derive(Default)]
struct Module {
    types: Vec<FunctionType>,
    /// The type index of each function, imported ones first.
    functions: Vec<u32>,
    /// Each export's name, kind and index.
    exports: Vec<(String, u8, u32)>,
}

impl Module {
    fn read(wasm: &[u8]) -> Option<Module> {
        let mut bytes = Reader(wasm.strip_prefix(b"\0asm\x01\0\0\0")?);
        let mut module = Module::default();
        while !bytes.0.is_empty() {
            let id = bytes.byte()?;
            let size = bytes.u32()? as usize;
            let mut section = Reader(bytes.take(size)?);
            match id {
                1 => module.types = section.vector(Reader::function_type)?,
                2 => {
                    for _ in 0..section.u32()? {
                        section.name()?;
                        section.name()?;
                        match section.byte()? {
                            0 => module.functions.push(section.u32()?),
                            1 => {
                                section.reference_type()?;
                                section.limits()?;
                            }
                            2 => section.limits()?,
                            3 => {
                                section.value_type()?;
                                section.byte()?;
                            }
                            4 => {
                                section.byte()?;
                                section.u32()?;
                            }
                            _ => return None,
                        }
                    }
                }
                3 => {
                    let defined = section.vector(Reader::u32)?;
                    module.functions.extend(defined);
                }
                7 => {
                    module.exports = section
                        .vector(|reader| Some((reader.name()?, reader.byte()?, reader.u32()?)))?
                }
                _ => {}
            }
        }
        Some(module)
    }

    /// The type of the function exported as `name`.
    fn function(&self, name: &str) -> Option<&FunctionType> {
        let (_, kind, index) = self.exports.iter().find(|(export, ..)| export == name)?;
        if *kind != 0 {
            return None;
        }
        let type_index = *self.functions.get(*index as usize)?;
        self.types.get(type_index as usize)
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Option<u8> {
        let (&first, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(first)
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        if length > self.0.len() {
            return None;
        }
        let (taken, rest) = self.0.split_at(length);
        self.0 = rest;
        Some(taken)
    }

    /// An unsigned LEB128 number of at most 64 bits.
    fn u64(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn u32(&mut self) -> Option<u32> {
        u32::try_from(self.u64()?).ok()
    }

    fn vector<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let count = self.u32()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(item(self)?);
        }
        Some(items)
    }

    fn name(&mut self) -> Option<String> {
        let length = self.u32()? as usize;
        String::from_utf8(self.take(length)?.to_vec()).ok()
    }

    /// A number, vector or reference type of one byte.
    fn value_type(&mut self) -> Option<u8> {
        let byte = self.byte()?;
        matches!(byte, 0x7B..=0x7F | 0x70 | 0x6F).then_some(byte)
    }

    fn reference_type(&mut self) -> Option<u8> {
        let byte = self.byte()?;
        matches!(byte, 0x70 | 0x6F).then_some(byte)
    }

    fn function_type(&mut self) -> Option<FunctionType> {
        if self.byte()? != 0x60 {
            return None;
        }
        Some((
            self.vector(Reader::value_type)?,
            self.vector(Reader::value_type)?,
        ))
    }

    /// A table's or memory's limits: flags, a minimum and, with flag 1, a maximum.
    fn limits(&mut self) -> Option<()> {
        let flags = self.byte()?;
        if flags > 0x07 {
            return None;
        }
        self.u64()?;
        if flags & 1 == 1 {
            self.u64()?;
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module with the types `types`, functions of those type indices `functions`, and
    /// `exports` of functions by index.
    fn module(types: &[(&[u8], &[u8])], functions: &[u32], exports: &[(&str, u32)]) -> Vec<u8> {
        fn vector(items: Vec<Vec<u8>>) -> Vec<u8> {
            let mut out = vec![items.len() as u8];
            out.extend(items.concat());
            out
        }
        fn section(id: u8, payload: Vec<u8>) -> Vec<u8> {
            let mut out = vec![id, payload.len() as u8];
            out.extend(payload);
            out
        }
        let types = types.iter().map(|(params, results)| {
            let mut out = vec![0x60, params.len() as u8];
            out.extend_from_slice(params);
            out.push(results.len() as u8);
            out.extend_from_slice(results);
            out
        });
        let exports = exports.iter().map(|(name, index)| {
            let mut out = vec![name.len() as u8];
            out.extend_from_slice(name.as_bytes());
            out.extend([0, *index as u8]);
            out
        });
        let functions = functions.iter().map(|index| vec![*index as u8]).collect();
        let mut wasm = b"\0asm\x01\0\0\0".to_vec();
        wasm.extend(section(1, vector(types.collect())));
        wasm.extend(section(3, vector(functions)));
        wasm.extend(section(7, vector(exports.collect())));
        wasm
    }

    #[test]
    fn takes_a_grammar_and_refuses_other_modules() {
        let rust = include_bytes!("../../tests/fixtures/grammars/rust/tree-sitter-rust.wasm");
        assert_eq!(check(rust, "tree_sitter_rust"), Ok(()));
        let python = include_bytes!("../../tests/fixtures/grammars/python/tree-sitter-python.wasm");
        assert_eq!(check(python, "tree_sitter_python"), Ok(()));
        assert!(check(rust, "tree_sitter_python").is_err());

        let language: (&[u8], &[u8]) = (&[], &[I32]);
        let empty: (&[u8], &[u8]) = (&[], &[]);
        let fine = module(
            &[language, empty],
            &[0, 1],
            &[("tree_sitter_x", 0), ("_initialize", 1)],
        );
        assert_eq!(check(&fine, "tree_sitter_x"), Ok(()));
        // The module of the review: the language function returns nothing.
        let no_result = module(&[empty], &[0], &[("tree_sitter_x", 0)]);
        assert!(check(&no_result, "tree_sitter_x").is_err());
        let initializer = module(
            &[language],
            &[0, 0],
            &[("tree_sitter_x", 0), ("_initialize", 1)],
        );
        assert!(check(&initializer, "tree_sitter_x").is_err());
        assert!(check(b"\0asm\x01\0\0", "tree_sitter_x").is_err());
        assert!(check(&fine[..fine.len() - 1], "tree_sitter_x").is_err());
    }
}
