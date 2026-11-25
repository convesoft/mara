//! The corpus generator (`support/corpus.rs`) and its checks.

#[path = "../support/corpus.rs"]
mod generator;

pub use generator::*;

mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use mara_engine::document;
    use mara_engine::work::WorkCounts;

    use super::*;
    use crate::{PULL_REQUEST_SIZES, SEED};

    #[test]
    fn a_seed_gives_the_same_files() {
        let first = files(Corpus::generate(1, SEED).root());
        assert_eq!(first, files(Corpus::generate(1, SEED).root()));
        assert_ne!(first, files(Corpus::generate(1, SEED + 1).root()));
    }

    #[test]
    fn larger_corpora_have_more_clusters_not_deeper_ones() {
        let shapes: Vec<[usize; 4]> = PULL_REQUEST_SIZES
            .iter()
            .map(|&size| {
                let corpus = Corpus::generate(size, SEED);
                let per_size = [corpus.items / size, corpus.documents / size];
                [
                    per_size[0],
                    per_size[1],
                    corpus.max_chain_depth,
                    corpus.max_fan_out,
                ]
            })
            .collect();
        assert_eq!(shapes[0][0], 200, "items at 1×: {shapes:?}");
        assert!(
            shapes[0][2] > 0 && shapes[0][3] > 1,
            "the 1× corpus has chains: {shapes:?}"
        );
        assert!(shapes.iter().all(|shape| *shape == shapes[0]), "{shapes:?}");
    }

    #[test]
    fn the_configuration_and_the_schema_load_without_diagnostics() {
        let corpus = Corpus::generate(1, SEED);
        let root = mara::project::select(corpus.root(), None).expect("the corpus is a project");
        let project = mara::project::load(root, &mut WorkCounts::default());
        assert_eq!(project.diagnostics, []);
        let include: Vec<&str> = project
            .config
            .include
            .iter()
            .flatten()
            .map(|pattern| pattern.path.as_str())
            .collect();
        assert_eq!(include, ["docs/**/*.mara.md"]);
        let schema = project.schema.expect("the schema loads");
        assert_eq!((schema.types.len(), schema.relations.len()), (11, 14));
        assert!(schema.types["term"].fields["aliases"].repeatable);
        assert!(schema.relations["supersedes"].same_type && schema.relations["supersedes"].acyclic);
    }

    /// The generator writes the current item format: every declared document parses without
    /// diagnostics into the items generated for it, with one parse each.
    #[test]
    fn writes_the_current_item_format() {
        for size in PULL_REQUEST_SIZES {
            let corpus = Corpus::generate(size, SEED);
            let mut work = WorkCounts::default();
            let (mut documents, mut items) = (0, 0);
            for (path, contents) in files(corpus.root()) {
                // `docs/**/*.mara.md`, except what `.gitignore` excludes.
                let name = path.to_string_lossy();
                if !name.starts_with("docs/")
                    || name.starts_with("docs/generated/")
                    || !name.ends_with(".mara.md")
                {
                    continue;
                }
                let parsed = document::parse(&contents, &mut work);
                let diagnostics: Vec<String> =
                    parsed.diagnostics(&name).map(|d| d.to_string()).collect();
                assert!(diagnostics.is_empty(), "{}", diagnostics.join("\n"));
                // With lone-CR line endings the document reads alike (DES-REGIONS).
                let lone_cr: Vec<u8> = contents
                    .iter()
                    .map(|&byte| if byte == b'\n' { b'\r' } else { byte })
                    .collect();
                let reread = document::parse(&lone_cr, &mut WorkCounts::default());
                assert_eq!(reread, parsed, "{name} with lone CR");
                documents += 1;
                items += parsed.items.len();
            }
            let expected = (corpus.documents, corpus.items);
            assert_eq!(
                (documents, items),
                expected,
                "documents and items at {size}×"
            );
            let parses = WorkCounts {
                parses: documents as u64,
                ..WorkCounts::default()
            };
            assert_eq!(work, parses, "work at {size}×");
        }
    }

    /// Every file below `root` with its contents, by relative path.
    fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut directories = vec![root.to_path_buf()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).expect("read a corpus directory") {
                let path = entry.expect("read a corpus directory entry").path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    let contents = fs::read(&path).expect("read a corpus file");
                    let relative = path
                        .strip_prefix(root)
                        .expect("below the root")
                        .to_path_buf();
                    files.push((relative, contents));
                }
            }
        }
        files.sort();
        files
    }
}
