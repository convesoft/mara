use crate::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    format_version: u32,
    project: ProjectSection,
    content: ContentSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    rules: Option<TemplateRules>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query_policy: Option<TemplateQueryPolicy>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateRules {
    format_version: u32,
    files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateQueryPolicy {
    format_version: u32,
    files: Vec<String>,
    definition_files: Vec<String>,
    shape_files: Vec<String>,
    max_rows: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectSection {
    name: String,
    schema: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentSection {
    include: Vec<String>,
}

fn project_template(name: &str, template: Template) -> Result<String, Error> {
    toml::to_string_pretty(&ProjectFile {
        format_version: match template {
            Template::Engineering => 3,
            Template::EngineeringLegacy => 2,
            Template::Minimal | Template::Empty => 1,
        },
        project: ProjectSection {
            name: name.into(),
            schema: SCHEMA_FILE.into(),
        },
        content: ContentSection {
            include: vec!["**/*.mara.md".into()],
        },
        rules: matches!(
            template,
            Template::Engineering | Template::EngineeringLegacy
        )
        .then(|| TemplateRules {
            format_version: 1,
            files: vec![".mara/engineering-rules.yaml".into()],
        }),
        query_policy: (template == Template::Engineering).then(|| TemplateQueryPolicy {
            format_version: 1,
            files: vec![".mara/engineering-policy.yaml".into()],
            definition_files: vec![".mara/engineering-queries.json".into()],
            shape_files: vec![".mara/engineering-shapes.ttl".into()],
            max_rows: 10_000,
        }),
    })
    .map_err(|source| Error::InvalidProject {
        path: PathBuf::from(PROJECT_FILE),
        message: source.to_string(),
    })
}

fn schema_template(template: Template) -> &'static str {
    match template {
        Template::Minimal => include_str!("../../../templates/minimal-schema.yaml"),
        Template::Empty => include_str!("../../../templates/empty-schema.yaml"),
        Template::Engineering => include_str!("../../../templates/engineering-schema.yaml"),
        Template::EngineeringLegacy => {
            include_str!("../../../templates/engineering-legacy-schema.yaml")
        }
    }
}

pub(crate) fn initial_sources(name: &str, template: Template) -> Result<Vec<SourceFile>, Error> {
    let mut files = vec![SourceFile {
        path: SCHEMA_FILE.into(),
        content: schema_template(template).into(),
    }];
    let assets: &[(&str, &str)] = match template {
        Template::EngineeringLegacy => &[
            (
                "engineering-rules.yaml",
                include_str!("../../../templates/engineering-legacy-rules.yaml"),
            ),
            (
                "engineering-checks.yaml",
                include_str!("../../../templates/engineering-legacy-checks.yaml"),
            ),
            (
                "engineering-execution.yaml",
                include_str!("../../../templates/engineering-legacy-execution.yaml"),
            ),
        ],
        Template::Engineering => &[
            (
                "engineering-rules.yaml",
                include_str!("../../../templates/engineering-rules.yaml"),
            ),
            (
                "engineering-policy.yaml",
                include_str!("../../../templates/engineering-policy.yaml"),
            ),
            (
                "engineering-queries.json",
                include_str!("../../../templates/engineering-queries.json"),
            ),
            (
                "engineering-shapes.ttl",
                include_str!("../../../templates/engineering-shapes.ttl"),
            ),
            (
                "engineering-gates.yaml",
                include_str!("../../../templates/engineering-gates.yaml"),
            ),
            (
                "engineering-risk-matrix.json",
                include_str!("../../../templates/engineering-risk-matrix.json"),
            ),
            (
                "engineering-views.json",
                include_str!("../../../templates/engineering-views.json"),
            ),
        ],
        Template::Minimal | Template::Empty => &[],
    };
    files.extend(assets.iter().map(|(name, content)| SourceFile {
        path: format!(".mara/{name}"),
        content: (*content).into(),
    }));
    files.push(SourceFile {
        path: PROJECT_FILE.into(),
        content: project_template(name, template)?,
    });
    Ok(files)
}
