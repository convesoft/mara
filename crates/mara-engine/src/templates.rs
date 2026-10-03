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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateRules {
    format_version: u32,
    files: Vec<String>,
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
        format_version: if template == Template::Engineering {
            2
        } else {
            1
        },
        project: ProjectSection {
            name: name.into(),
            schema: SCHEMA_FILE.into(),
        },
        content: ContentSection {
            include: vec!["**/*.mara.md".into()],
        },
        rules: (template == Template::Engineering).then(|| TemplateRules {
            format_version: 1,
            files: vec![".mara/engineering-rules.yaml".into()],
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
    }
}

pub(crate) fn initial_sources(name: &str, template: Template) -> Result<Vec<SourceFile>, Error> {
    let mut files = vec![SourceFile {
        path: SCHEMA_FILE.into(),
        content: schema_template(template).into(),
    }];
    if template == Template::Engineering {
        files.extend([
            SourceFile {
                path: ".mara/engineering-rules.yaml".into(),
                content: include_str!("../../../templates/engineering-rules.yaml").into(),
            },
            SourceFile {
                path: ".mara/engineering-checks.yaml".into(),
                content: include_str!("../../../templates/engineering-checks.yaml").into(),
            },
            SourceFile {
                path: ".mara/engineering-execution.yaml".into(),
                content: include_str!("../../../templates/engineering-execution.yaml").into(),
            },
        ]);
    }
    files.push(SourceFile {
        path: PROJECT_FILE.into(),
        content: project_template(name, template)?,
    });
    Ok(files)
}
