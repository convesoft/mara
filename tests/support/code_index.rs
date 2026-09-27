// Deterministic SCIP protocol fixtures. Explicit definitions are authored by each
// test; this helper does not discover symbols or simulate a language indexer.
use protobuf::Message;
use scip::types;
use std::{fs, path::Path};

pub fn configure(root: &Path, language: &str, extensions: &[&str], grammar: bool) {
    let config_path = root.join(".mara/project.toml");
    let mut config = fs::read_to_string(&config_path).unwrap().replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    );
    config.push_str(&format!("\n[[code.languages]]\nname = {language:?}\ncommand = [\"cp\", \".mara/{language}.scip\", \"{{output}}\"]\nposition_encoding = \"utf8\"\nextensions = {extensions:?}\n"));
    if grammar {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let assets = manifest.join(if matches!(language, "python" | "typescript") {
            "tests/fixtures/code"
        } else {
            ".mara/code"
        });
        fs::create_dir_all(root.join(".mara/code")).unwrap();
        for extension in ["wasm", "scm"] {
            let name = format!("{language}.{extension}");
            fs::copy(assets.join(&name), root.join(".mara/code").join(name)).unwrap();
        }
        config.push_str(&format!(
            "grammar = \".mara/code/{language}.wasm\"\nquery = \".mara/code/{language}.scm\"\n"
        ));
    }
    fs::write(config_path, config).unwrap();
    write_index(root, language, &[]);
}

pub fn definition(source: &str, name: &str, descriptor: &str) -> (usize, usize, String) {
    let start = source
        .find(name)
        .expect("explicit definition exists in fixture");
    (start, start + name.len(), descriptor.to_owned())
}

pub type Definitions<'a> = (&'a str, &'a [(usize, usize, String)]);
pub fn write_index(root: &Path, language: &str, documents: &[Definitions<'_>]) {
    let mut index = types::Index::new();
    let mut metadata = types::Metadata::new();
    metadata.project_root = url::Url::from_directory_path(root).unwrap().to_string();
    index.metadata = protobuf::MessageField::some(metadata);
    for (path, definitions) in documents {
        let source = fs::read_to_string(root.join(path)).unwrap();
        let mut document = types::Document::new();
        document.relative_path = (*path).into();
        document.position_encoding =
            types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart.into();
        for (start, end, descriptor) in *definitions {
            let position = |offset: usize| {
                let prefix = &source[..offset];
                let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
                let column = offset - prefix.rfind('\n').map_or(0, |i| i + 1);
                (line as i32, column as i32)
            };
            let (sl, sc) = position(*start);
            let (el, ec) = position(*end);
            let mut occurrence = types::Occurrence::new();
            occurrence.symbol = format!("fixture test test 1 {descriptor}");
            occurrence.symbol_roles = 1;
            occurrence.range = vec![sl, sc, el, ec];
            document.occurrences.push(occurrence);
        }
        index.documents.push(document);
    }
    fs::write(
        root.join(format!(".mara/{language}.scip")),
        index.write_to_bytes().unwrap(),
    )
    .unwrap();
}

pub fn write_single(root: &Path, language: &str, path: &str, name: &str, descriptor: &str) {
    let source = fs::read_to_string(root.join(path)).unwrap();
    let start = source.rfind(name).expect("explicit single definition");
    write_index(
        root,
        language,
        &[(path, &[(start, start + name.len(), descriptor.to_owned())])],
    );
}
