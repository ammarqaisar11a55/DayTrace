//! Automatic classification of activity into categories.

mod preview;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::{invalid, Result};
use crate::models::{Application, Exclusion, ExclusionKind};

pub use preview::{apply, preview, RuleChange, RulePreview};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConditionField {
    Application,
    ExecutablePath,
    WindowTitle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConditionOperator {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: ConditionField,
    pub operator: ConditionOperator,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub conditions: Vec<Condition>,
    pub target_category_id: i64,
    pub priority: i64,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub name: String,
    pub conditions: Vec<Condition>,
    pub target_category_id: i64,
    pub enabled: bool,
}

impl RuleInput {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return invalid("A rule needs a name");
        }
        if self.conditions.is_empty() {
            return invalid("A rule needs at least one condition");
        }
        if self.conditions.iter().any(|condition| condition.value.trim().is_empty()) {
            return invalid("Every condition needs a value");
        }
        Ok(())
    }
}

/// What a rule can see about a piece of activity.
#[derive(Clone, Copy, Debug, Default)]
pub struct Subject<'a> {
    pub identifier: &'a str,
    pub name: &'a str,
    pub display_name: Option<&'a str>,
    pub executable_path: Option<&'a str>,
    pub window_title: Option<&'a str>,
}

fn text_matches(operator: ConditionOperator, text: &str, expected: &str) -> bool {
    let (text, expected) = (text.to_lowercase(), expected.trim().to_lowercase());
    match operator {
        ConditionOperator::Equals => text == expected,
        ConditionOperator::Contains => text.contains(&expected),
        ConditionOperator::StartsWith => text.starts_with(&expected),
        ConditionOperator::EndsWith => text.ends_with(&expected),
    }
}

impl Condition {
    fn matches(&self, subject: &Subject<'_>) -> bool {
        let check =
            |text: Option<&str>| text.is_some_and(|text| text_matches(self.operator, text, &self.value));
        match self.field {
            ConditionField::Application => {
                check(Some(subject.identifier)) || check(Some(subject.name)) || check(subject.display_name)
            }
            ConditionField::ExecutablePath => check(subject.executable_path),
            ConditionField::WindowTitle => check(subject.window_title),
        }
    }
}

impl Rule {
    pub fn matches(&self, subject: &Subject<'_>) -> bool {
        self.enabled
            && !self.conditions.is_empty()
            && self.conditions.iter().all(|condition| condition.matches(subject))
    }
}

/// Applies the documented precedence: enabled rules by ascending priority, then
/// the application's default. The first match decides.
#[derive(Clone, Debug, Default)]
pub struct Classifier {
    rules: Vec<Rule>,
    application_categories: HashMap<i64, i64>,
}

impl Classifier {
    pub fn new(mut rules: Vec<Rule>, applications: &[Application]) -> Self {
        rules.retain(|rule| rule.enabled);
        rules.sort_by_key(|rule| (rule.priority, rule.id));
        Self {
            rules,
            application_categories: applications
                .iter()
                .filter_map(|application| Some((application.id, application.default_category_id?)))
                .collect(),
        }
    }

    pub fn classify(&self, application_id: Option<i64>, subject: &Subject<'_>) -> Option<i64> {
        self.rules
            .iter()
            .find(|rule| rule.matches(subject))
            .map(|rule| rule.target_category_id)
            .or_else(|| application_id.and_then(|id| self.application_categories.get(&id).copied()))
    }
}

/// Privacy exclusions. Excluded applications are matched before anything is
/// stored, so they never appear in the database at all.
#[derive(Clone, Debug, Default)]
pub struct ExclusionSet {
    applications: HashSet<String>,
    paths: Vec<String>,
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

impl ExclusionSet {
    pub fn new(exclusions: &[Exclusion]) -> Self {
        let mut set = Self::default();
        for exclusion in exclusions {
            match exclusion.kind {
                ExclusionKind::Application => {
                    set.applications.insert(exclusion.value.to_lowercase());
                }
                ExclusionKind::Path => set.paths.push(normalize_path(&exclusion.value)),
            }
        }
        set
    }

    pub fn excludes_application(&self, identifier: &str, name: &str, path: Option<&str>) -> bool {
        if self.applications.contains(&identifier.to_lowercase())
            || self.applications.contains(&name.to_lowercase())
        {
            return true;
        }
        path.is_some_and(|path| {
            let path = normalize_path(path);
            self.paths.iter().any(|excluded| path.starts_with(excluded))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition(field: ConditionField, operator: ConditionOperator, value: &str) -> Condition {
        Condition { field, operator, value: value.into() }
    }

    fn rule(id: i64, priority: i64, conditions: Vec<Condition>, category: i64) -> Rule {
        Rule {
            id,
            name: format!("rule {id}"),
            conditions,
            target_category_id: category,
            priority,
            enabled: true,
        }
    }

    fn application(id: i64, category: Option<i64>) -> Application {
        Application {
            id,
            platform_identifier: "code".into(),
            name: "Code".into(),
            display_name: None,
            executable_path: None,
            icon_path: None,
            default_category_id: category,
            created_at: 0,
        }
    }

    fn code_subject() -> Subject<'static> {
        Subject {
            identifier: "code",
            name: "Visual Studio Code",
            executable_path: Some("/home/me/Notes-Saver/.bin/code"),
            ..Default::default()
        }
    }

    fn is_code() -> Condition {
        condition(ConditionField::Application, ConditionOperator::Equals, "visual studio code")
    }

    #[test]
    fn all_conditions_must_match() {
        let matching = rule(
            1,
            10,
            vec![
                is_code(),
                condition(ConditionField::ExecutablePath, ConditionOperator::Contains, "/Notes-Saver/"),
            ],
            7,
        );
        let failing = rule(
            2,
            10,
            vec![
                is_code(),
                condition(ConditionField::ExecutablePath, ConditionOperator::Contains, "/Other/"),
            ],
            8,
        );
        assert!(matching.matches(&code_subject()));
        assert!(!failing.matches(&code_subject()));
    }

    #[test]
    fn lower_priority_number_wins() {
        let rules = vec![rule(1, 20, vec![is_code()], 1), rule(2, 10, vec![is_code()], 2)];
        let classifier = Classifier::new(rules, &[]);
        assert_eq!(classifier.classify(None, &code_subject()), Some(2));
    }

    #[test]
    fn equal_priority_falls_back_to_rule_id() {
        let rules = vec![rule(5, 10, vec![is_code()], 5), rule(3, 10, vec![is_code()], 3)];
        let classifier = Classifier::new(rules, &[]);
        assert_eq!(classifier.classify(None, &code_subject()), Some(3));
    }

    #[test]
    fn disabled_rules_are_ignored() {
        let mut disabled = rule(1, 10, vec![is_code()], 1);
        disabled.enabled = false;
        let classifier = Classifier::new(vec![disabled], &[]);
        assert_eq!(classifier.classify(None, &code_subject()), None);
    }

    #[test]
    fn rules_beat_application_defaults() {
        let applications = [application(1, Some(200))];

        let classifier = Classifier::new(vec![], &applications);
        assert_eq!(classifier.classify(Some(1), &code_subject()), Some(200));

        let rules = vec![rule(1, 10, vec![is_code()], 50)];
        let classifier = Classifier::new(rules, &applications);
        assert_eq!(classifier.classify(Some(1), &code_subject()), Some(50));
    }

    #[test]
    fn an_application_without_a_default_has_no_category() {
        let classifier = Classifier::new(vec![], &[application(1, None)]);
        assert_eq!(classifier.classify(Some(1), &code_subject()), None);
    }

    #[test]
    fn missing_fields_never_match() {
        let title =
            rule(1, 10, vec![condition(ConditionField::WindowTitle, ConditionOperator::Contains, "x")], 1);
        assert!(!title.matches(&code_subject()));
    }

    #[test]
    fn operators_are_case_insensitive() {
        assert!(text_matches(ConditionOperator::StartsWith, "/Home/Me", "/home"));
        assert!(text_matches(ConditionOperator::EndsWith, "report.PDF", ".pdf"));
        assert!(text_matches(ConditionOperator::Equals, "Firefox", " firefox "));
        assert!(!text_matches(ConditionOperator::Equals, "Firefox Beta", "firefox"));
    }

    #[test]
    fn rule_input_validation() {
        let mut input = RuleInput {
            name: "Rule".into(),
            conditions: vec![is_code()],
            target_category_id: 1,
            enabled: true,
        };
        assert!(input.validate().is_ok());
        input.conditions.clear();
        assert!(input.validate().is_err());
    }

    #[test]
    fn exclusions_match_identifier_name_and_path_prefix() {
        let exclusions = [
            Exclusion { id: 1, kind: ExclusionKind::Application, value: "KeePassXC".into(), created_at: 0 },
            Exclusion { id: 2, kind: ExclusionKind::Path, value: r"C:\Private".into(), created_at: 0 },
        ];
        let set = ExclusionSet::new(&exclusions);
        assert!(set.excludes_application("org.keepassxc.keepassxc", "keepassxc", None));
        assert!(set.excludes_application("x", "y", Some("c:/private/tool.exe")));
        assert!(!set.excludes_application("x", "y", Some("/usr/bin/private")));
    }
}
