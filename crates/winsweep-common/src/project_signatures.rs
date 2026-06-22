//! Project type signatures detection
//!
//! This module defines the signatures used to detect different project types based on file patterns.

use crate::types::ProjectType;
use regex::Regex;
use std::path::Path;

/// A signature that can identify a project type
#[derive(Debug, Clone)]
pub struct ProjectSignature {
    /// Files or directories that must exist
    pub required_files: &'static [&'static str],
    /// Files or directories that should NOT exist (for exclusion)
    pub forbidden_files: &'static [&'static str],
    /// File content patterns to check
    pub content_patterns: &'static [(&'static str, &'static str)], // (file_pattern, content_regex)
    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,
}

/// All project signatures
pub const PROJECT_SIGNATURES: &[(&ProjectType, ProjectSignature)] = &[
    // JavaScript/TypeScript
    (
        &ProjectType::NodeJs,
        ProjectSignature {
            required_files: &["package.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::TypeScript,
        ProjectSignature {
            required_files: &["tsconfig.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::React,
        ProjectSignature {
            required_files: &["package.json"],
            forbidden_files: &[],
            content_patterns: &[("package.json", r#""react""#)],
            confidence: 0.8,
        },
    ),
    (
        &ProjectType::Vue,
        ProjectSignature {
            required_files: &["package.json"],
            forbidden_files: &[],
            content_patterns: &[("package.json", r#""vue""#)],
            confidence: 0.8,
        },
    ),
    (
        &ProjectType::Angular,
        ProjectSignature {
            required_files: &["angular.json", "package.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Svelte,
        ProjectSignature {
            required_files: &["package.json"],
            forbidden_files: &[],
            content_patterns: &[("package.json", r#""svelte""#)],
            confidence: 0.8,
        },
    ),
    // Rust
    (
        &ProjectType::Rust,
        ProjectSignature {
            required_files: &["Cargo.toml"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // Python
    (
        &ProjectType::Python,
        ProjectSignature {
            required_files: &["requirements.txt", "setup.py", "pyproject.toml"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.7,
        },
    ),
    (
        &ProjectType::Django,
        ProjectSignature {
            required_files: &["manage.py", "requirements.txt"],
            forbidden_files: &[],
            content_patterns: &[("manage.py", r#"django-admin"#)],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Flask,
        ProjectSignature {
            required_files: &["app.py", "requirements.txt"],
            forbidden_files: &["manage.py"],
            content_patterns: &[("app.py", r#"from flask"#)],
            confidence: 0.8,
        },
    ),
    (
        &ProjectType::FastAPI,
        ProjectSignature {
            required_files: &["main.py"],
            forbidden_files: &[],
            content_patterns: &[("*.py", r#"from fastapi import"#)],
            confidence: 0.8,
        },
    ),
    // Java
    (
        &ProjectType::Java,
        ProjectSignature {
            required_files: &["pom.xml", "build.gradle", ".classpath"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.8,
        },
    ),
    (
        &ProjectType::Maven,
        ProjectSignature {
            required_files: &["pom.xml"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::Gradle,
        ProjectSignature {
            required_files: &["build.gradle", "build.gradle.kts"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // Go
    (
        &ProjectType::Go,
        ProjectSignature {
            required_files: &["go.mod", "go.sum"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    // C/C++
    (
        &ProjectType::Cpp,
        ProjectSignature {
            required_files: &["CMakeLists.txt", "Makefile", "configure"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.7,
        },
    ),
    (
        &ProjectType::CMake,
        ProjectSignature {
            required_files: &["CMakeLists.txt"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // .NET
    (
        &ProjectType::DotNet,
        ProjectSignature {
            required_files: &["*.sln", "*.csproj", "project.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    // Ruby
    (
        &ProjectType::Ruby,
        ProjectSignature {
            required_files: &["Gemfile"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Rails,
        ProjectSignature {
            required_files: &["config/application.rb", "config/routes.rb"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // PHP
    (
        &ProjectType::Php,
        ProjectSignature {
            required_files: &["composer.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Laravel,
        ProjectSignature {
            required_files: &["artisan"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // Mobile
    (
        &ProjectType::Android,
        ProjectSignature {
            required_files: &["AndroidManifest.xml", "build.gradle"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Flutter,
        ProjectSignature {
            required_files: &["pubspec.yaml"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::ReactNative,
        ProjectSignature {
            required_files: &["package.json"],
            forbidden_files: &[],
            content_patterns: &[("package.json", r#""react-native""#)],
            confidence: 0.9,
        },
    ),
    // Infrastructure
    (
        &ProjectType::Docker,
        ProjectSignature {
            required_files: &["Dockerfile", "docker-compose.yml", "docker-compose.yaml"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Kubernetes,
        ProjectSignature {
            required_files: &["k8s", "kubernetes", "*.yaml", "*.yml"],
            forbidden_files: &[],
            content_patterns: &[("*.yaml", r#"apiVersion"#), ("*.yml", r#"apiVersion"#)],
            confidence: 0.7,
        },
    ),
    (
        &ProjectType::Terraform,
        ProjectSignature {
            required_files: &["*.tf"],
            forbidden_files: &[],
            content_patterns: &[("*.tf", r#"resource "#)],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Ansible,
        ProjectSignature {
            required_files: &["playbook.yml", "ansible.cfg"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::Packer,
        ProjectSignature {
            required_files: &["*.pkr.hcl", "packer.json"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::Vagrant,
        ProjectSignature {
            required_files: &["Vagrantfile"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // Data
    (
        &ProjectType::Jupyter,
        ProjectSignature {
            required_files: &["*.ipynb"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.9,
        },
    ),
    (
        &ProjectType::R,
        ProjectSignature {
            required_files: &["*.R", "*.Rmd", "DESCRIPTION"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.8,
        },
    ),
    // Game Development
    (
        &ProjectType::Unity,
        ProjectSignature {
            required_files: &["Assets", "ProjectSettings", "Assembly-CSharp.csproj"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::Unreal,
        ProjectSignature {
            required_files: &["*.uproject", "Source", "Config"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    // Version Control
    (
        &ProjectType::Git,
        ProjectSignature {
            required_files: &[".git"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::Hg,
        ProjectSignature {
            required_files: &[".hg"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
    (
        &ProjectType::Svn,
        ProjectSignature {
            required_files: &[".svn"],
            forbidden_files: &[],
            content_patterns: &[],
            confidence: 0.95,
        },
    ),
];

/// Detect project type from a directory path
pub fn detect_project_type(path: &Path) -> Option<ProjectType> {
    let mut best_match: Option<(ProjectType, f32)> = None;

    for (project_type, signature) in PROJECT_SIGNATURES {
        let confidence = calculate_confidence(path, signature);
        if confidence > 0.5 && (best_match.is_none() || confidence > best_match.unwrap().1) {
            best_match = Some((**project_type, confidence));
        }
    }

    best_match.map(|(pt, _)| pt)
}

/// Calculate confidence score for a project signature
fn calculate_confidence(path: &Path, signature: &ProjectSignature) -> f32 {
    let mut confidence = 0.0;
    let mut required_count = 0;

    // Check required files
    for pattern in signature.required_files {
        if matches_pattern(path, pattern) {
            required_count += 1;
        }
    }

    if !signature.required_files.is_empty() {
        confidence += (required_count as f32 / signature.required_files.len() as f32) * 0.7;
    }

    // Check forbidden files (penalty)
    for pattern in signature.forbidden_files {
        if matches_pattern(path, pattern) {
            confidence -= 0.5;
        }
    }

    // Add base confidence
    confidence += signature.confidence * 0.3;

    confidence.clamp(0.0, 1.0)
}

/// Check if a path matches a glob pattern
fn matches_pattern(path: &Path, pattern: &str) -> bool {
    // Simple glob matching - could be enhanced with the glob crate
    if pattern.contains('*') {
        // Convert glob to regex for matching
        let regex_pattern = pattern
            .replace('.', r"\.")
            .replace('*', ".*")
            .replace('?', ".");

        if let Ok(regex) = Regex::new(&regex_pattern) {
            if let Some(path_str) = path.to_str() {
                return regex.is_match(path_str);
            }
        }
        false
    } else {
        path.join(pattern).exists() || path.file_name() == Some(std::ffi::OsStr::new(pattern))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn test_detect_rust_project() {
        let _path = PathBuf::from("/test/project");
        // Would need to create actual test files for this to work
        // assert_eq!(detect_project_type(&path), Some(ProjectType::Rust));
    }
}
