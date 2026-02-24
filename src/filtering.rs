//! Project filtering functionality.
//!
//! This module provides functions for filtering projects based on various criteria
//! such as size and modification time.

use anyhow::Result;
use chrono::{DateTime, Local};
use rayon::prelude::*;
use std::fs;
use std::time::SystemTime;

use crate::config::filter::SortCriteria;
use crate::config::{FilterOptions, SortOptions};
use crate::project::{Project, ProjectType};
use crate::utils::parse_size;

/// Filter projects based on size and modification time criteria.
///
/// This function applies parallel filtering to remove projects that don't meet
/// the specified criteria:
/// - Projects smaller than the minimum size threshold
/// - Projects modified more recently than the specified number of days
///
/// # Arguments
///
/// * `projects` - Vector of projects to filter
/// * `filter_opts` - Filtering options containing size and time criteria
///
/// # Returns
///
/// - `Ok(Vec<Project>)` - Filtered list of projects that meet all criteria
/// - `Err(anyhow::Error)` - If size parsing fails, or file system errors occur
///
/// # Errors
///
/// This function can return errors if:
/// - The size string in `filter_opts.keep_size` cannot be parsed (invalid format)
/// - Size value overflow occurs during parsing
///
/// # Examples
///
/// ```no_run
/// # use clean_dev_dirs::{filtering::filter_projects, config::FilterOptions, project::Project};
/// # use anyhow::Result;
/// # fn example(projects: Vec<Project>) -> Result<()> {
/// let filter_opts = FilterOptions {
///     keep_size: "100MB".to_string(),
///     keep_days: 30,
/// };
/// let filtered = filter_projects(projects, &filter_opts)?;
/// # Ok(())
/// # }
/// ```
pub fn filter_projects(
    projects: Vec<Project>,
    filter_opts: &FilterOptions,
) -> Result<Vec<Project>> {
    let keep_size_bytes = parse_size(&filter_opts.keep_size)?;
    let keep_days = filter_opts.keep_days;

    Ok(projects
        .into_par_iter()
        .filter(|project| meets_size_criteria(project, keep_size_bytes))
        .filter(|project| meets_time_criteria(project, keep_days))
        .collect())
}

/// Check if a project meets the size criteria.
const fn meets_size_criteria(project: &Project, min_size: u64) -> bool {
    project.build_arts.size >= min_size
}

/// Check if a project meets the time criteria.
fn meets_time_criteria(project: &Project, keep_days: u32) -> bool {
    if keep_days == 0 {
        return true;
    }

    is_project_old_enough(project, keep_days)
}

/// Check if a project is old enough based on its modification time.
fn is_project_old_enough(project: &Project, keep_days: u32) -> bool {
    let Result::Ok(metadata) = fs::metadata(&project.build_arts.path) else {
        return true; // If we can't read metadata, don't filter it out
    };

    let Result::Ok(modified) = metadata.modified() else {
        return true; // If we can't read modification time, don't filter it out
    };

    let modified_time: DateTime<Local> = modified.into();
    let cutoff_time = Local::now() - chrono::Duration::days(i64::from(keep_days));

    modified_time <= cutoff_time
}

/// Sort projects in place according to the given sorting options.
///
/// When `sort_opts.criteria` is `None`, the list is left in its current order.
/// Each criterion has a natural default direction:
/// - `Size`: largest first (descending)
/// - `Age`: oldest first (ascending)
/// - `Name`: alphabetical, case-insensitive (ascending)
/// - `Type`: grouped by type name alphabetically
///
/// Setting `sort_opts.reverse` to `true` flips the resulting order.
///
/// For the `Age` criterion a Schwartzian transform is used to avoid
/// repeated filesystem calls inside the comparator.
///
/// # Arguments
///
/// * `projects` - Mutable reference to the vector of projects to sort
/// * `sort_opts` - Sorting options specifying criterion and direction
///
/// # Examples
///
/// ```no_run
/// # use clean_dev_dirs::{filtering::sort_projects, config::{SortOptions, SortCriteria}};
/// # use clean_dev_dirs::project::Project;
/// # fn example(mut projects: Vec<Project>) {
/// let sort_opts = SortOptions {
///     criteria: Some(SortCriteria::Size),
///     reverse: false,
/// };
/// sort_projects(&mut projects, &sort_opts);
/// # }
/// ```
pub fn sort_projects(projects: &mut Vec<Project>, sort_opts: &SortOptions) {
    let Some(criteria) = sort_opts.criteria else {
        return;
    };

    match criteria {
        SortCriteria::Size => {
            projects.sort_by(|a, b| b.build_arts.size.cmp(&a.build_arts.size));
        }
        SortCriteria::Age => {
            sort_by_age(projects);
        }
        SortCriteria::Name => {
            projects.sort_by(|a, b| {
                let name_a = a.name.as_deref().unwrap_or("");
                let name_b = b.name.as_deref().unwrap_or("");
                name_a.to_lowercase().cmp(&name_b.to_lowercase())
            });
        }
        SortCriteria::Type => {
            projects.sort_by(|a, b| type_order(&a.kind).cmp(&type_order(&b.kind)));
        }
    }

    if sort_opts.reverse {
        projects.reverse();
    }
}

/// Sort projects by build artifacts modification time (oldest first).
///
/// Uses a Schwartzian transform: each project is paired with its modification
/// time (fetched once), sorted, then the timestamps are discarded.
fn sort_by_age(projects: &mut Vec<Project>) {
    let mut decorated: Vec<(Project, SystemTime)> = projects
        .drain(..)
        .map(|p| {
            let mtime = fs::metadata(&p.build_arts.path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (p, mtime)
        })
        .collect();

    decorated.sort_by(|a, b| a.1.cmp(&b.1));

    projects.extend(decorated.into_iter().map(|(p, _)| p));
}

/// Map a `ProjectType` to an ordering index for type-based sorting.
///
/// Types are ordered alphabetically by their display name:
/// C/C++, Deno, .NET, Elixir, Go, Java, Node, Python, Ruby, Rust, Swift
const fn type_order(kind: &ProjectType) -> u8 {
    match kind {
        ProjectType::Cpp => 0,
        ProjectType::Deno => 1,
        ProjectType::DotNet => 2,
        ProjectType::Elixir => 3,
        ProjectType::Go => 4,
        ProjectType::Java => 5,
        ProjectType::Node => 6,
        ProjectType::Python => 7,
        ProjectType::Ruby => 8,
        ProjectType::Rust => 9,
        ProjectType::Swift => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{BuildArtifacts, Project, ProjectType};
    use std::path::PathBuf;

    /// Helper function to create a test project
    fn create_test_project(
        kind: ProjectType,
        root_path: &str,
        build_path: &str,
        size: u64,
        name: Option<String>,
    ) -> Project {
        Project::new(
            kind,
            PathBuf::from(root_path),
            BuildArtifacts {
                path: PathBuf::from(build_path),
                size,
            },
            name,
        )
    }

    #[test]
    fn test_meets_size_criteria() {
        let project = create_test_project(
            ProjectType::Rust,
            "/test",
            "/test/target",
            1_000_000, // 1MB
            Some("test".to_string()),
        );

        assert!(meets_size_criteria(&project, 500_000)); // 0.5MB - should pass
        assert!(meets_size_criteria(&project, 1_000_000)); // Exactly 1MB - should pass
        assert!(!meets_size_criteria(&project, 2_000_000)); // 2MB - should fail
    }

    #[test]
    fn test_meets_time_criteria_disabled() {
        let project = create_test_project(
            ProjectType::Rust,
            "/test",
            "/test/target",
            1_000_000,
            Some("test".to_string()),
        );

        // When keep_days is 0, should always return true
        assert!(meets_time_criteria(&project, 0));
    }

    // ── Sorting tests ───────────────────────────────────────────────────

    #[test]
    fn test_sort_by_size_descending() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/a",
                "/a/target",
                100,
                Some("small".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                300,
                Some("large".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                200,
                Some("medium".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Size),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].build_arts.size, 300);
        assert_eq!(projects[1].build_arts.size, 200);
        assert_eq!(projects[2].build_arts.size, 100);
    }

    #[test]
    fn test_sort_by_size_reversed() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/a",
                "/a/target",
                100,
                Some("small".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                300,
                Some("large".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                200,
                Some("medium".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Size),
            reverse: true,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].build_arts.size, 100);
        assert_eq!(projects[1].build_arts.size, 200);
        assert_eq!(projects[2].build_arts.size, 300);
    }

    #[test]
    fn test_sort_by_name_alphabetical() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                100,
                Some("charlie".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/a",
                "/a/target",
                100,
                Some("alpha".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                100,
                Some("bravo".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Name),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].name.as_deref(), Some("alpha"));
        assert_eq!(projects[1].name.as_deref(), Some("bravo"));
        assert_eq!(projects[2].name.as_deref(), Some("charlie"));
    }

    #[test]
    fn test_sort_by_name_case_insensitive() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                100,
                Some("Charlie".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/a",
                "/a/target",
                100,
                Some("alpha".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                100,
                Some("Bravo".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Name),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].name.as_deref(), Some("alpha"));
        assert_eq!(projects[1].name.as_deref(), Some("Bravo"));
        assert_eq!(projects[2].name.as_deref(), Some("Charlie"));
    }

    #[test]
    fn test_sort_by_name_none_names_first() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                100,
                Some("charlie".into()),
            ),
            create_test_project(ProjectType::Rust, "/a", "/a/target", 100, None),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                100,
                Some("alpha".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Name),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        // None name sorts as "" which comes before any alphabetical name
        assert_eq!(projects[0].name.as_deref(), None);
        assert_eq!(projects[1].name.as_deref(), Some("alpha"));
        assert_eq!(projects[2].name.as_deref(), Some("charlie"));
    }

    #[test]
    fn test_sort_by_type() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/r",
                "/r/target",
                100,
                Some("rust-proj".into()),
            ),
            create_test_project(
                ProjectType::Go,
                "/g",
                "/g/vendor",
                100,
                Some("go-proj".into()),
            ),
            create_test_project(
                ProjectType::Python,
                "/p",
                "/p/__pycache__",
                100,
                Some("py-proj".into()),
            ),
            create_test_project(
                ProjectType::Node,
                "/n",
                "/n/node_modules",
                100,
                Some("node-proj".into()),
            ),
            create_test_project(
                ProjectType::Java,
                "/j",
                "/j/target",
                100,
                Some("java-proj".into()),
            ),
            create_test_project(
                ProjectType::Cpp,
                "/c",
                "/c/build",
                100,
                Some("cpp-proj".into()),
            ),
            create_test_project(
                ProjectType::Swift,
                "/s",
                "/s/.build",
                100,
                Some("swift-proj".into()),
            ),
            create_test_project(
                ProjectType::DotNet,
                "/d",
                "/d/obj",
                100,
                Some("dotnet-proj".into()),
            ),
            create_test_project(
                ProjectType::Ruby,
                "/rb",
                "/rb/vendor/bundle",
                100,
                Some("ruby-proj".into()),
            ),
            create_test_project(
                ProjectType::Elixir,
                "/ex",
                "/ex/_build",
                100,
                Some("elixir-proj".into()),
            ),
            create_test_project(
                ProjectType::Deno,
                "/dn",
                "/dn/vendor",
                100,
                Some("deno-proj".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Type),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].kind, ProjectType::Cpp);
        assert_eq!(projects[1].kind, ProjectType::Deno);
        assert_eq!(projects[2].kind, ProjectType::DotNet);
        assert_eq!(projects[3].kind, ProjectType::Elixir);
        assert_eq!(projects[4].kind, ProjectType::Go);
        assert_eq!(projects[5].kind, ProjectType::Java);
        assert_eq!(projects[6].kind, ProjectType::Node);
        assert_eq!(projects[7].kind, ProjectType::Python);
        assert_eq!(projects[8].kind, ProjectType::Ruby);
        assert_eq!(projects[9].kind, ProjectType::Rust);
        assert_eq!(projects[10].kind, ProjectType::Swift);
    }

    #[test]
    fn test_sort_by_type_reversed() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Go,
                "/g",
                "/g/vendor",
                100,
                Some("go-proj".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/r",
                "/r/target",
                100,
                Some("rust-proj".into()),
            ),
            create_test_project(
                ProjectType::Node,
                "/n",
                "/n/node_modules",
                100,
                Some("node-proj".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Type),
            reverse: true,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects[0].kind, ProjectType::Rust);
        assert_eq!(projects[1].kind, ProjectType::Node);
        assert_eq!(projects[2].kind, ProjectType::Go);
    }

    #[test]
    fn test_sort_none_criteria_preserves_order() {
        let mut projects = vec![
            create_test_project(
                ProjectType::Rust,
                "/c",
                "/c/target",
                100,
                Some("charlie".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/a",
                "/a/target",
                300,
                Some("alpha".into()),
            ),
            create_test_project(
                ProjectType::Rust,
                "/b",
                "/b/target",
                200,
                Some("bravo".into()),
            ),
        ];

        let sort_opts = SortOptions {
            criteria: None,
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        // Order should be unchanged
        assert_eq!(projects[0].name.as_deref(), Some("charlie"));
        assert_eq!(projects[1].name.as_deref(), Some("alpha"));
        assert_eq!(projects[2].name.as_deref(), Some("bravo"));
    }

    #[test]
    fn test_sort_empty_list() {
        let mut projects: Vec<Project> = vec![];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Size),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert!(projects.is_empty());
    }

    #[test]
    fn test_sort_single_element() {
        let mut projects = vec![create_test_project(
            ProjectType::Rust,
            "/a",
            "/a/target",
            100,
            Some("only".into()),
        )];

        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Name),
            reverse: false,
        };
        sort_projects(&mut projects, &sort_opts);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("only"));
    }

    #[test]
    fn test_type_order_values() {
        assert!(type_order(&ProjectType::Cpp) < type_order(&ProjectType::Deno));
        assert!(type_order(&ProjectType::Deno) < type_order(&ProjectType::DotNet));
        assert!(type_order(&ProjectType::DotNet) < type_order(&ProjectType::Elixir));
        assert!(type_order(&ProjectType::Elixir) < type_order(&ProjectType::Go));
        assert!(type_order(&ProjectType::Go) < type_order(&ProjectType::Java));
        assert!(type_order(&ProjectType::Java) < type_order(&ProjectType::Node));
        assert!(type_order(&ProjectType::Node) < type_order(&ProjectType::Python));
        assert!(type_order(&ProjectType::Python) < type_order(&ProjectType::Ruby));
        assert!(type_order(&ProjectType::Ruby) < type_order(&ProjectType::Rust));
        assert!(type_order(&ProjectType::Rust) < type_order(&ProjectType::Swift));
    }
}
