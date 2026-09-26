//! Disposable Rust free-function resolver for the MARA-69 evaluation.
use std::{
    env, fs,
    path::{Component, Path},
};
use syn::{Expr, Item, Lit, Meta, spanned::Spanned};

fn main() {
    let args: Vec<String> = env::args().collect();
    let [_, root, link] = args.as_slice() else {
        eprintln!("usage: mara-69-pilot <project-root> code:<path>[::<free-function>]");
        std::process::exit(2);
    };
    let Some(address) = link.strip_prefix("code:") else {
        eprintln!("invalid code link: {link}");
        std::process::exit(2);
    };
    let (path, selector) = match address.split_once("::") {
        Some((path, selector)) if !path.is_empty() && !selector.is_empty() => {
            (path, Some(selector))
        }
        None if !address.is_empty() => (address, None),
        _ => {
            eprintln!("invalid code link: {link}");
            std::process::exit(2);
        }
    };
    if selector.is_some_and(|selector| selector.contains("::")) {
        println!("unsupported selector: {link}");
        return;
    }
    let relative_path = Path::new(path);
    if relative_path
        .extension()
        .is_none_or(|extension| extension != "rs")
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        println!("unsupported Rust path: {path}");
        return;
    }
    let source_path = Path::new(root).join(path);
    let Ok(source) = fs::read_to_string(&source_path) else {
        println!("missing file: {path}");
        return;
    };
    let Some(selected) = selector else {
        println!("resolved {link} at {path}:1");
        return;
    };
    let Ok(parsed) = syn::parse_file(&source) else {
        println!("unparseable Rust file: {path}");
        return;
    };
    let mut matches = Vec::new();
    for item in parsed.items {
        let Item::Fn(function) = item else { continue };
        let symbol = function.sig.ident.to_string();
        let line = function.sig.ident.span().start().line;
        for attr in &function.attrs {
            if !attr.path().is_ident("doc") {
                continue;
            }
            let Meta::NameValue(value) = &attr.meta else {
                continue;
            };
            let Expr::Lit(expr) = &value.value else {
                continue;
            };
            let Lit::Str(text) = &expr.lit else {
                continue;
            };
            let doc = text.value();
            let Some(marker) = doc.trim().strip_prefix("@mara ") else {
                continue;
            };
            let parts: Vec<_> = marker.split_whitespace().collect();
            if let [relation, item] = parts.as_slice() {
                println!(
                    "marker {relation} {item} -> code:{path}::{symbol} at {path}:{}",
                    attr.span().start().line
                );
            }
        }
        if symbol == selected {
            matches.push(line);
        }
    }
    match matches.as_slice() {
        [] => println!("missing {link}"),
        [line] => println!("resolved {link} at {path}:{line}"),
        many => println!("ambiguous {link}: {} definitions", many.len()),
    }
}
