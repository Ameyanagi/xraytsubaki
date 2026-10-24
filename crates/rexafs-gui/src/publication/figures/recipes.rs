//! Named recipe storage, validation and migration from `publication.composite`.
use super::{FigureOptions, FigureSettings, composer::CompositeRecipe};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Maximum recipe-name length in Unicode characters, after trimming whitespace.
const MAX_NAME_CHARS: usize = 60;

/// A project-local name and its independent multi-panel contents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct NamedRecipe {
    /// Trimmed name of 1–60 Unicode characters, unique under lowercase comparison.
    pub name: String,
    /// Panel order and shared appearance; naming does not change these settings.
    pub recipe: CompositeRecipe,
}

impl NamedRecipe {
    /// A lowercase file stem for figure and caption saves. Runs of punctuation
    /// or whitespace become one hyphen; letters and digits (including Unicode)
    /// are kept. Empty stems use `figure`; Windows device names gain `figure-`.
    pub fn file_stem(&self) -> String {
        let mut stem = String::new();
        let mut separator = false;
        for ch in self.name.chars().flat_map(char::to_lowercase) {
            if ch.is_alphanumeric() {
                if separator && !stem.is_empty() {
                    stem.push('-');
                }
                stem.push(ch);
                separator = false;
            } else {
                separator = true;
            }
        }
        if stem.is_empty() {
            return "figure".into();
        }
        if matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
            || ["com", "lpt"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|n| {
                    matches!(
                        n,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            stem.insert_str(0, "figure-");
        }
        stem
    }
}

/// Input-only representation. Missing recipe fields migrate to Figure 1;
/// `composite` is accepted for old projects but is never serialized again.
#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct SavedFigureSettings {
    figures: BTreeMap<String, FigureOptions>,
    table_captions: BTreeMap<String, String>,
    recipes: Option<Vec<NamedRecipe>>,
    active_recipe: usize,
    composite: Option<CompositeRecipe>,
}

impl From<SavedFigureSettings> for FigureSettings {
    /// Saved names that break the rules (for example after a hand edit) are
    /// repaired rather than rejected, so the project still opens: names are
    /// trimmed and shortened to 60 characters, empty names become the first
    /// unused Figure N, and repeated names gain " 2", " 3", ….
    fn from(saved: SavedFigureSettings) -> Self {
        let mut recipes = saved.recipes.unwrap_or_else(|| {
            vec![NamedRecipe {
                name: "Figure 1".into(),
                recipe: saved.composite.unwrap_or_default(),
            }]
        });
        if recipes.is_empty() {
            recipes = Self::default().recipes;
        }
        let mut names = BTreeSet::new();
        for entry in &mut recipes {
            let base: String = entry.name.trim().chars().take(MAX_NAME_CHARS).collect();
            let mut candidates = (1usize..).map(|n| match (base.is_empty(), n) {
                (true, n) => format!("Figure {n}"),
                (false, 1) => base.clone(),
                (false, n) => {
                    let suffix = format!(" {n}");
                    let kept: String = base
                        .chars()
                        .take(MAX_NAME_CHARS - suffix.chars().count())
                        .collect();
                    format!("{}{suffix}", kept.trim_end())
                }
            });
            let name = candidates
                .find(|name| !names.contains(&name.to_lowercase()))
                .unwrap();
            names.insert(name.to_lowercase());
            entry.name = name;
        }
        Self {
            figures: saved.figures,
            table_captions: saved.table_captions,
            active_recipe: saved.active_recipe.min(recipes.len() - 1),
            recipes,
        }
    }
}

impl Default for FigureSettings {
    fn default() -> Self {
        Self {
            figures: BTreeMap::new(),
            table_captions: BTreeMap::new(),
            recipes: vec![NamedRecipe {
                name: "Figure 1".into(),
                recipe: CompositeRecipe::default(),
            }],
            active_recipe: 0,
        }
    }
}

/// Trim and validate the shared name rules. Uniqueness is checked separately.
fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a recipe name.".into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "Use at most {MAX_NAME_CHARS} characters for the recipe name."
        ));
    }
    Ok(name.into())
}

impl FigureSettings {
    /// All named recipes in insertion order; at least one recipe always exists.
    pub fn recipes(&self) -> &[NamedRecipe] {
        &self.recipes
    }

    /// The selected recipe's zero-based position in [`Self::recipes`].
    pub fn active_recipe_index(&self) -> usize {
        self.active_recipe
    }

    /// The selected recipe's name and contents, ready for preview or export.
    pub fn active_recipe(&self) -> &NamedRecipe {
        &self.recipes[self.active_recipe]
    }

    /// Edit only the active recipe's contents, preserving names and selection.
    pub fn active_composite_mut(&mut self) -> &mut CompositeRecipe {
        &mut self.recipes[self.active_recipe].recipe
    }

    /// Select an existing recipe by index. An invalid index leaves it unchanged.
    pub fn select_recipe(&mut self, index: usize) -> bool {
        if index >= self.recipes.len() {
            return false;
        }
        self.active_recipe = index;
        true
    }

    /// Append and select a default recipe named with the first unused Figure N.
    /// The composer can populate its empty panels from the available figures.
    pub fn new_recipe(&mut self) {
        let name = (1usize..)
            .map(|n| format!("Figure {n}"))
            .find(|name| !self.has_recipe_name(name))
            .unwrap();
        self.recipes.push(NamedRecipe {
            name,
            recipe: CompositeRecipe::default(),
        });
        self.active_recipe = self.recipes.len() - 1;
    }

    /// Append and select an independent copy of the active recipe. Names use
    /// ` copy`, then ` copy 2`, etc.; the original name is shortened to fit.
    pub fn duplicate_recipe(&mut self) {
        let source = self.active_recipe();
        let name = (1usize..)
            .map(|n| {
                let suffix = if n == 1 {
                    " copy".into()
                } else {
                    format!(" copy {n}")
                };
                let base: String = source
                    .name
                    .chars()
                    .take(MAX_NAME_CHARS - suffix.chars().count())
                    .collect();
                format!("{}{suffix}", base.trim_end())
            })
            .find(|name| !self.has_recipe_name(name))
            .unwrap();
        let recipe = source.recipe.clone();
        self.recipes.push(NamedRecipe { name, recipe });
        self.active_recipe = self.recipes.len() - 1;
    }

    /// Validate a proposed rename and return its trimmed form. Names must be
    /// non-empty, at most 60 Unicode characters and unique ignoring case.
    /// The active recipe's current name is allowed, including a case-only edit.
    pub fn validate_recipe_name(&self, name: &str) -> Result<String, String> {
        let name = validate_name(name)?;
        if self.recipes.iter().enumerate().any(|(index, entry)| {
            index != self.active_recipe && entry.name.to_lowercase() == name.to_lowercase()
        }) {
            return Err("A recipe with this name already exists. Choose a unique name.".into());
        }
        Ok(name)
    }

    /// Rename the active recipe without changing its contents or position.
    /// Invalid names return an inline-ready error and leave the recipe intact.
    pub fn rename_recipe(&mut self, name: &str) -> Result<(), String> {
        let name = self.validate_recipe_name(name)?;
        self.recipes[self.active_recipe].name = name;
        Ok(())
    }

    /// Delete the active recipe, selecting the next recipe or, at the end, the
    /// previous one. Returns false without changes when only one recipe remains.
    /// The UI must confirm with the user before calling this operation.
    pub fn delete_recipe(&mut self) -> bool {
        if self.recipes.len() == 1 {
            return false;
        }
        self.recipes.remove(self.active_recipe);
        self.active_recipe = self.active_recipe.min(self.recipes.len() - 1);
        true
    }

    /// Compare using Unicode lowercase, consistently with rename validation.
    fn has_recipe_name(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.recipes
            .iter()
            .any(|entry| entry.name.to_lowercase() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publication::figures::composer::CompositePanel;
    use serde_json::json;

    #[test]
    fn legacy_composite_migrates_without_changing_contents_or_figure_settings() {
        let recipe = CompositeRecipe {
            panels: vec!["chi-k".into(), "xanes".into()],
            samples: Vec::new(),
            columns: 1,
            width_mm: 83.,
            panel_height_mm: Some(60.),
            font_size: 7.,
            line_width: Some(1.2),
            series_colors: BTreeMap::new(),
            dpi: 300.,
            ..Default::default()
        };
        let settings: FigureSettings = serde_json::from_value(json!({
            "composite": recipe,
            "figures": {"chi-k": {"caption": "My caption", "legend": false}},
            "table_captions": {"parameters": "My parameters"}
        }))
        .unwrap();
        assert_eq!(settings.recipes().len(), 1);
        assert_eq!(settings.active_recipe().name, "Figure 1");
        assert_eq!(settings.active_recipe().recipe, recipe);
        assert_eq!(
            settings.options("chi-k").caption.as_deref(),
            Some("My caption")
        );
        assert!(!settings.options("chi-k").legend);
        assert_eq!(settings.table_captions["parameters"], "My parameters");
        let saved = serde_json::to_value(&settings).unwrap();
        assert!(saved.get("composite").is_none());
        assert_eq!(
            saved["recipes"][0]["recipe"],
            serde_json::to_value(recipe).unwrap()
        );
        assert_eq!(
            serde_json::from_value::<FigureSettings>(saved).unwrap(),
            settings
        );
    }

    #[test]
    fn absent_or_empty_recipes_always_load_one_default_recipe() {
        for input in [
            json!({}),
            json!({"figures": {}}),
            json!({"recipes": [], "active_recipe": 42}),
        ] {
            let settings: FigureSettings = serde_json::from_value(input).unwrap();
            assert_eq!(settings, FigureSettings::default());
        }
    }

    #[test]
    fn named_recipes_round_trip_in_order_with_active_selection() {
        let mut settings = FigureSettings::default();
        settings.active_composite_mut().panels.push("xanes".into());
        settings.new_recipe();
        settings.rename_recipe("  SI Figure S4  ").unwrap();
        settings.active_composite_mut().panels.push("chi-r".into());
        let saved = serde_json::to_value(&settings).unwrap();
        assert_eq!(saved["active_recipe"], 1);
        assert_eq!(saved["recipes"][1]["name"], "SI Figure S4");
        assert_eq!(
            serde_json::from_value::<FigureSettings>(saved).unwrap(),
            settings
        );
        assert_eq!(
            settings.recipes()[0].recipe.panels,
            [CompositePanel::from("xanes")]
        );
    }

    #[test]
    fn loading_trims_names_clamps_selection_and_prefers_new_format() {
        let settings: FigureSettings = serde_json::from_value(json!({
            "recipes": [{"name": "  New name  ", "recipe": {"panels": ["xanes"]}}],
            "active_recipe": 10,
            "composite": {"panels": ["chi-k"]}
        }))
        .unwrap();
        assert_eq!(settings.active_recipe_index(), 0);
        assert_eq!(settings.active_recipe().name, "New name");
        assert_eq!(
            settings.active_recipe().recipe.panels,
            [CompositePanel::from("xanes")]
        );
    }

    #[test]
    fn rename_validates_trimmed_length_and_case_insensitive_uniqueness() {
        let mut settings = FigureSettings::default();
        for invalid in ["", " \t\n", &"x".repeat(61)] {
            let before = settings.clone();
            assert!(settings.rename_recipe(invalid).is_err());
            assert_eq!(settings, before);
        }
        settings.rename_recipe("  FIGURE 1  ").unwrap();
        assert_eq!(settings.active_recipe().name, "FIGURE 1");
        settings.new_recipe();
        assert!(settings.rename_recipe("figure 1").is_err());
        let name = "銅".repeat(60);
        settings.rename_recipe(&format!(" {name} ")).unwrap();
        assert_eq!(settings.active_recipe().name.chars().count(), 60);
        assert!(settings.rename_recipe(&"銅".repeat(61)).is_err());
        settings.rename_recipe("Échantillon").unwrap();
        settings.new_recipe();
        assert!(settings.rename_recipe("éCHANTILLON").is_err());
    }

    #[test]
    fn invalid_saved_names_are_repaired_so_the_project_opens() {
        for (names, expected) in [
            (vec![""], vec!["Figure 1"]),
            (vec!["   "], vec!["Figure 1"]),
            (
                vec!["Figure 1", " figure 1 "],
                vec!["Figure 1", "figure 1 2"],
            ),
            (vec!["É", "é"], vec!["É", "é 2"]),
            (vec!["Figure 1", ""], vec!["Figure 1", "Figure 2"]),
        ] {
            let recipes: Vec<_> = names
                .into_iter()
                .map(|name| json!({"name": name, "recipe": {}}))
                .collect();
            let settings: FigureSettings =
                serde_json::from_value(json!({"recipes": recipes})).unwrap();
            let loaded: Vec<_> = settings.recipes().iter().map(|r| r.name.as_str()).collect();
            assert_eq!(loaded, expected);
        }
        let settings: FigureSettings = serde_json::from_value(json!({
            "recipes": [{"name": "x".repeat(61), "recipe": {}}]
        }))
        .unwrap();
        assert_eq!(settings.active_recipe().name, "x".repeat(60));
    }

    #[test]
    fn new_uses_first_unused_number_ignoring_case() {
        let mut settings = FigureSettings::default();
        settings.rename_recipe("figure 1").unwrap();
        settings.new_recipe();
        assert_eq!(settings.active_recipe().name, "Figure 2");
        settings.rename_recipe("FIGURE 3").unwrap();
        settings.new_recipe();
        assert_eq!(settings.active_recipe().name, "Figure 2");
        assert_eq!(settings.active_recipe().recipe, CompositeRecipe::default());
    }

    #[test]
    fn duplicate_copies_contents_and_numbers_conflicting_names() {
        let mut settings = FigureSettings::default();
        settings.active_composite_mut().panels.push("chi-k".into());
        settings.duplicate_recipe();
        assert_eq!(settings.active_recipe().name, "Figure 1 copy");
        assert_eq!(
            settings.active_recipe().recipe,
            settings.recipes()[0].recipe
        );
        settings.rename_recipe("FIGURE 1 COPY").unwrap();
        settings.select_recipe(0);
        settings.duplicate_recipe();
        assert_eq!(settings.active_recipe().name, "Figure 1 copy 2");
        settings.active_composite_mut().panels.push("chi-r".into());
        assert_eq!(
            settings.recipes()[0].recipe.panels,
            [CompositePanel::from("chi-k")]
        );
        settings.select_recipe(0);
        settings.duplicate_recipe();
        assert_eq!(settings.active_recipe().name, "Figure 1 copy 3");
    }

    #[test]
    fn duplicate_shortens_long_unicode_names_to_leave_room_for_suffix() {
        let mut settings = FigureSettings::default();
        settings.rename_recipe(&"銅".repeat(60)).unwrap();
        settings.duplicate_recipe();
        assert_eq!(
            settings.active_recipe().name,
            format!("{} copy", "銅".repeat(55))
        );
        settings.select_recipe(0);
        settings.duplicate_recipe();
        assert_eq!(
            settings.active_recipe().name,
            format!("{} copy 2", "銅".repeat(53))
        );
    }

    #[test]
    fn delete_selects_next_then_previous_and_preserves_last_recipe() {
        let mut settings = FigureSettings::default();
        assert!(!settings.delete_recipe());
        assert!(!settings.select_recipe(1));
        settings.new_recipe();
        settings.new_recipe();
        settings.select_recipe(1);
        assert!(settings.delete_recipe());
        assert_eq!(settings.active_recipe_index(), 1);
        assert_eq!(settings.active_recipe().name, "Figure 3");
        assert_eq!(settings.recipes()[0].name, "Figure 1");
        assert!(settings.delete_recipe());
        assert_eq!(settings.active_recipe_index(), 0);
        assert_eq!(settings.active_recipe().name, "Figure 1");
        assert!(!settings.delete_recipe());
        assert_eq!(settings.recipes().len(), 1);
        settings.new_recipe();
        settings.select_recipe(0);
        assert!(settings.delete_recipe());
        assert_eq!(settings.active_recipe_index(), 0);
        assert_eq!(settings.active_recipe().name, "Figure 2");
    }

    #[test]
    fn file_stems_sanitize_separators_punctuation_unicode_and_device_names() {
        for (name, expected) in [
            ("Figure 2", "figure-2"),
            (" SI Figure S4 ", "si-figure-s4"),
            ("../../Figure / 2\\:*?\"<>|", "figure-2"),
            ("A...B___C---D", "a-b-c-d"),
            ("...", "figure"),
            ("  ", "figure"),
            ("Échantillon 銅", "échantillon-銅"),
            ("CON", "figure-con"),
            ("LPT1", "figure-lpt1"),
            ("COM¹", "figure-com¹"),
            ("COM10", "com10"),
        ] {
            let recipe = NamedRecipe {
                name: name.into(),
                recipe: CompositeRecipe::default(),
            };
            assert_eq!(recipe.file_stem(), expected, "{name}");
        }
    }
}
