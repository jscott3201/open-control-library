use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::de::{self, Deserialize, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};

const POINT_SCHEMA_V1: &str = "cxf-library/points/v1";
const POINT_SCHEMA_V2: &str = "cxf-library/points/v2";

struct StrictValue(serde_json::Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor)
    }
}

struct StrictValueVisitor;

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictValue(value.into()))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictValue(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictValue(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .map(StrictValue)
            .ok_or_else(|| E::custom("non-finite number is forbidden"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string(value.to_string())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictValue(value.into()))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_none()
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictValue::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictValue>()? {
            values.push(value.0);
        }
        Ok(StrictValue(serde_json::Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate object key {key:?}")));
            }
            let value = map.next_value::<StrictValue>()?;
            values.insert(key, value.0);
        }
        Ok(StrictValue(serde_json::Value::Object(values)))
    }
}

#[derive(Clone)]
struct Import {
    index: usize,
    path: String,
}

#[derive(Clone)]
struct Alias {
    index: usize,
    name: String,
    target: String,
}

struct PointDictionary {
    points: BTreeSet<String>,
    imports: Vec<Import>,
    aliases: Vec<Alias>,
}

impl PointDictionary {
    fn alias(&self, name: &str) -> Option<&Alias> {
        self.aliases.iter().find(|alias| alias.name == name)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ResolvedPoint {
    pub(crate) path: String,
    pub(crate) name: String,
}

pub(crate) struct PointResolver {
    dictionaries: BTreeMap<String, PointDictionary>,
}

impl PointResolver {
    pub(crate) fn load(repo_root: &Path) -> Result<Self, String> {
        let points_root = repo_root.join("points");
        let entries = std::fs::read_dir(&points_root)
            .map_err(|error| format!("{}: {error}", points_root.display()))?;
        let mut documents = Vec::new();
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.ends_with(".points.json") {
                continue;
            }
            let bytes =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            documents.push((format!("points/{name}"), bytes));
        }
        Self::from_documents(documents)
    }

    fn from_documents(mut documents: Vec<(String, Vec<u8>)>) -> Result<Self, String> {
        documents.sort_by(|left, right| left.0.cmp(&right.0));
        let mut errors = Vec::new();
        let mut dictionaries = BTreeMap::new();
        if documents.is_empty() {
            errors.push("points: no *.points.json dictionaries found".to_string());
        }

        for (path, bytes) in documents {
            if dictionary_family(&path).is_none() {
                errors.push(format!("{path}: malformed dictionary path"));
                continue;
            }
            if dictionaries.contains_key(&path) {
                errors.push(format!("{path}: duplicate dictionary path"));
                continue;
            }
            let value = match serde_json::from_slice::<StrictValue>(&bytes) {
                Ok(value) => value.0,
                Err(error) => {
                    errors.push(format!("{path}: {error}"));
                    continue;
                }
            };
            let Some(object) = value.as_object() else {
                errors.push(format!("{path}: must contain a JSON object"));
                continue;
            };
            let schema = object.get("schema").and_then(serde_json::Value::as_str);
            match schema {
                Some(POINT_SCHEMA_V1) => check_keys(
                    object,
                    &["schema", "equipment", "namespaces", "points"],
                    &["notes"],
                    &path,
                    &mut errors,
                ),
                Some(POINT_SCHEMA_V2) => check_keys(
                    object,
                    &[
                        "schema",
                        "equipment",
                        "namespaces",
                        "imports",
                        "aliases",
                        "points",
                    ],
                    &["notes"],
                    &path,
                    &mut errors,
                ),
                _ => errors.push(format!(
                    "{path}: schema must be {POINT_SCHEMA_V1:?} or {POINT_SCHEMA_V2:?}"
                )),
            }

            validate_identity(object, &path, &mut errors);
            validate_namespaces(object.get("namespaces"), &path, &mut errors);
            let points = validate_points(object.get("points"), &path, &mut errors);
            let (imports, aliases) = if schema == Some(POINT_SCHEMA_V2) {
                (
                    validate_imports(object.get("imports"), &path, &mut errors),
                    validate_aliases(object.get("aliases"), &points, &path, &mut errors),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            dictionaries.insert(
                path,
                PointDictionary {
                    points,
                    imports,
                    aliases,
                },
            );
        }

        validate_cross_dictionary_contract(&dictionaries, &mut errors);
        errors.extend(import_cycle_errors(&dictionaries));
        errors.sort();
        errors.dedup();
        if errors.is_empty() {
            Ok(Self { dictionaries })
        } else {
            Err(errors.join("\n"))
        }
    }

    pub(crate) fn resolve_bare(
        &self,
        dictionary_path: &str,
        name: &str,
    ) -> Result<ResolvedPoint, String> {
        if dictionary_family(dictionary_path).is_none() {
            return Err(format!("malformed dictionary path {dictionary_path:?}"));
        }
        if !valid_name(name) {
            return Err(format!("malformed bare point name {name:?}"));
        }
        let dictionary = self
            .dictionaries
            .get(dictionary_path)
            .ok_or_else(|| format!("point dictionary {dictionary_path:?} is missing"))?;
        if dictionary.points.contains(name) {
            return Ok(ResolvedPoint {
                path: dictionary_path.to_string(),
                name: name.to_string(),
            });
        }
        if let Some(alias) = dictionary.alias(name) {
            let (path, name) = parse_ref(&alias.target)
                .ok_or_else(|| format!("alias {:?} has a malformed target", alias.name))?;
            let target = self
                .dictionaries
                .get(path)
                .ok_or_else(|| format!("alias {:?} target dictionary is missing", alias.name))?;
            if !target.points.contains(name) {
                return Err(format!(
                    "alias {:?} does not target a concrete point",
                    alias.name
                ));
            }
            return Ok(ResolvedPoint {
                path: path.to_string(),
                name: name.to_string(),
            });
        }
        Err(format!(
            "{dictionary_path} has no local point or alias {name:?}"
        ))
    }

    #[cfg(test)]
    fn resolve_ref(&self, reference: &str) -> Result<ResolvedPoint, String> {
        let (path, name) = parse_ref(reference)
            .ok_or_else(|| format!("malformed point reference {reference:?}"))?;
        self.resolve_bare(path, name)
    }
}

fn valid_name(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('a'..='z'))
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !value.contains("__")
        && !value.ends_with('_')
}

fn dictionary_family(path: &str) -> Option<&str> {
    let family = path.strip_prefix("points/")?.strip_suffix(".points.json")?;
    if valid_name(family) {
        Some(family)
    } else {
        None
    }
}

fn parse_ref(reference: &str) -> Option<(&str, &str)> {
    let (path, name) = reference.split_once('#')?;
    if dictionary_family(path).is_some() && valid_name(name) {
        Some((path, name))
    } else {
        None
    }
}

fn check_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    required: &[&str],
    optional: &[&str],
    label: &str,
    errors: &mut Vec<String>,
) {
    let required: BTreeSet<&str> = required.iter().copied().collect();
    let optional: BTreeSet<&str> = optional.iter().copied().collect();
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    for key in required.difference(&actual) {
        errors.push(format!("{label}: missing required key {key:?}"));
    }
    for key in actual
        .difference(&required)
        .filter(|key| !optional.contains(*key))
    {
        errors.push(format!("{label}: unexpected key {key:?}"));
    }
}

fn nonempty_string(value: Option<&serde_json::Value>) -> bool {
    value
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

fn nullable_string(value: Option<&serde_json::Value>) -> bool {
    value.is_some_and(|value| value.is_null() || nonempty_string(Some(value)))
}

fn validate_identity(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    let equipment = object.get("equipment").and_then(serde_json::Value::as_str);
    if !equipment.is_some_and(valid_name) {
        errors.push(format!(
            "{path}: equipment must be a lower-case snake_case identifier"
        ));
    } else if equipment != dictionary_family(path) {
        errors.push(format!("{path}: equipment must match filename stem"));
    }
    if object.get("notes").is_some_and(|notes| !notes.is_string()) {
        errors.push(format!("{path}: notes must be a string"));
    }
}

fn validate_namespaces(value: Option<&serde_json::Value>, path: &str, errors: &mut Vec<String>) {
    let Some(namespaces) = value.and_then(serde_json::Value::as_object) else {
        errors.push(format!("{path}: namespaces must be an object"));
        return;
    };
    check_keys(
        namespaces,
        &["brick", "s223", "quantitykind", "unit"],
        &["s223_g36"],
        &format!("{path}: namespaces"),
        errors,
    );
    for name in ["brick", "quantitykind", "s223", "s223_g36", "unit"] {
        let Some(record) = namespaces.get(name) else {
            continue;
        };
        let label = format!("{path}: namespaces.{name}");
        let Some(record) = record.as_object() else {
            errors.push(format!("{label}: must be an object"));
            continue;
        };
        check_keys(record, &["iri", "verified_version"], &[], &label, errors);
        for field in ["iri", "verified_version"] {
            if !nonempty_string(record.get(field)) {
                errors.push(format!("{label}.{field}: must be a nonempty string"));
            }
        }
    }
}

fn validate_points(
    value: Option<&serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(points) = value.and_then(serde_json::Value::as_array) else {
        errors.push(format!("{path}: points must be an array"));
        return BTreeSet::new();
    };
    let mut names = BTreeSet::new();
    let mut first_indices = BTreeMap::new();
    for (index, point) in points.iter().enumerate() {
        let label = format!("{path}: points[{index}]");
        let Some(point) = point.as_object() else {
            errors.push(format!("{label}: must be an object"));
            continue;
        };
        check_keys(
            point,
            &[
                "name",
                "description",
                "kind",
                "unit",
                "qudt_unit",
                "brick",
                "s223",
            ],
            &["notes", "derived", "provisional"],
            &label,
            errors,
        );
        let name = point.get("name").and_then(serde_json::Value::as_str);
        if !name.is_some_and(valid_name) {
            errors.push(format!(
                "{label}.name: must be a lower-case snake_case identifier"
            ));
        } else if let Some(first) = first_indices.get(name.unwrap()) {
            errors.push(format!(
                "{label}.name: duplicate {:?}; first used at index {first}",
                name.unwrap()
            ));
        } else {
            let name = name.unwrap().to_string();
            first_indices.insert(name.clone(), index);
            names.insert(name);
        }
        for field in ["description", "unit"] {
            if !nonempty_string(point.get(field)) {
                errors.push(format!("{label}.{field}: must be a nonempty string"));
            }
        }
        if !point
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| matches!(kind, "real" | "int" | "bool"))
        {
            errors.push(format!("{label}.kind: must be 'real', 'int', or 'bool'"));
        }
        for field in ["brick", "qudt_unit"] {
            if !nullable_string(point.get(field)) {
                errors.push(format!(
                    "{label}.{field}: must be a nonempty string or null"
                ));
            }
        }
        validate_s223(point.get("s223"), &label, errors);
        if point.get("notes").is_some_and(|notes| !notes.is_string()) {
            errors.push(format!("{label}.notes: must be a string"));
        }
        for field in ["derived", "provisional"] {
            if point.get(field).is_some_and(|value| !value.is_boolean()) {
                errors.push(format!("{label}.{field}: must be a boolean"));
            }
        }
    }
    names
}

fn validate_s223(value: Option<&serde_json::Value>, label: &str, errors: &mut Vec<String>) {
    let Some(value) = value else {
        errors.push(format!("{label}.s223: missing required value"));
        return;
    };
    if value.is_null() {
        return;
    }
    let Some(s223) = value.as_object() else {
        errors.push(format!("{label}.s223: must be an object or null"));
        return;
    };
    let s223_label = format!("{label}.s223");
    check_keys(
        s223,
        &[
            "pattern",
            "property_class",
            "quantitykind",
            "unit",
            "medium",
            "aspects",
        ],
        &["enumerationkind"],
        &s223_label,
        errors,
    );
    for field in ["pattern", "property_class"] {
        if !nonempty_string(s223.get(field)) {
            errors.push(format!("{s223_label}.{field}: must be a nonempty string"));
        }
    }
    for field in ["quantitykind", "unit", "medium"] {
        if !nullable_string(s223.get(field)) {
            errors.push(format!(
                "{s223_label}.{field}: must be a nonempty string or null"
            ));
        }
    }
    match s223.get("aspects").and_then(serde_json::Value::as_array) {
        Some(aspects) => {
            for (index, aspect) in aspects.iter().enumerate() {
                if !nonempty_string(Some(aspect)) {
                    errors.push(format!(
                        "{s223_label}.aspects[{index}]: must be a nonempty string"
                    ));
                }
            }
        }
        None => errors.push(format!("{s223_label}.aspects: must be an array")),
    }
    if s223
        .get("enumerationkind")
        .is_some_and(|value| !nonempty_string(Some(value)))
    {
        errors.push(format!(
            "{s223_label}.enumerationkind: must be a nonempty string"
        ));
    }
}

fn validate_imports(
    value: Option<&serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) -> Vec<Import> {
    let Some(imports) = value.and_then(serde_json::Value::as_array) else {
        errors.push(format!("{path}: imports must be an array"));
        return Vec::new();
    };
    let mut valid = Vec::new();
    let mut first_indices = BTreeMap::new();
    for (index, target) in imports.iter().enumerate() {
        let label = format!("{path}: imports[{index}]");
        let Some(target) = target.as_str() else {
            errors.push(format!(
                "{label}: must be a root-relative points/<family>.points.json path"
            ));
            continue;
        };
        if dictionary_family(target).is_none() {
            errors.push(format!(
                "{label}: must be a root-relative points/<family>.points.json path"
            ));
            continue;
        }
        if let Some(first) = first_indices.get(target) {
            errors.push(format!(
                "{label}: duplicate {target:?}; first used at index {first}"
            ));
            continue;
        }
        first_indices.insert(target, index);
        if target == path {
            errors.push(format!("{label}: self-import is forbidden"));
            continue;
        }
        valid.push(Import {
            index,
            path: target.to_string(),
        });
    }
    valid
}

fn validate_aliases(
    value: Option<&serde_json::Value>,
    local_points: &BTreeSet<String>,
    path: &str,
    errors: &mut Vec<String>,
) -> Vec<Alias> {
    let Some(aliases) = value.and_then(serde_json::Value::as_array) else {
        errors.push(format!("{path}: aliases must be an array"));
        return Vec::new();
    };
    let mut valid = Vec::new();
    let mut first_indices = BTreeMap::new();
    for (index, alias) in aliases.iter().enumerate() {
        let label = format!("{path}: aliases[{index}]");
        let Some(alias) = alias.as_object() else {
            errors.push(format!("{label}: must be an object"));
            continue;
        };
        check_keys(alias, &["name", "target"], &[], &label, errors);
        let name = alias.get("name").and_then(serde_json::Value::as_str);
        let target = alias.get("target").and_then(serde_json::Value::as_str);
        let mut name_valid = name.is_some_and(valid_name);
        if !name_valid {
            errors.push(format!(
                "{label}.name: must be a lower-case snake_case identifier"
            ));
        } else if let Some(first) = first_indices.get(name.unwrap()) {
            errors.push(format!(
                "{label}.name: duplicate {:?}; first used at index {first}",
                name.unwrap()
            ));
            name_valid = false;
        } else {
            first_indices.insert(name.unwrap(), index);
            if local_points.contains(name.unwrap()) {
                errors.push(format!(
                    "{label}.name: collides with local point {:?}",
                    name.unwrap()
                ));
                name_valid = false;
            }
        }
        let target_valid = target.is_some_and(|target| parse_ref(target).is_some());
        if !target_valid {
            errors.push(format!(
                "{label}.target: must be points/<family>.points.json#<name>"
            ));
        }
        if name_valid && target_valid {
            valid.push(Alias {
                index,
                name: name.unwrap().to_string(),
                target: target.unwrap().to_string(),
            });
        }
    }
    valid
}

fn validate_cross_dictionary_contract(
    dictionaries: &BTreeMap<String, PointDictionary>,
    errors: &mut Vec<String>,
) {
    for (path, dictionary) in dictionaries {
        for import in &dictionary.imports {
            if !dictionaries.contains_key(&import.path) {
                errors.push(format!(
                    "{path}: imports[{}]: target {:?} is missing",
                    import.index, import.path
                ));
            }
        }
        let imported: BTreeSet<&str> = dictionary
            .imports
            .iter()
            .map(|import| import.path.as_str())
            .collect();
        for alias in &dictionary.aliases {
            let (target_path, target_name) =
                parse_ref(&alias.target).expect("validated alias target");
            let label = format!("{path}: aliases[{}].target", alias.index);
            if !imported.contains(target_path) {
                errors.push(format!(
                    "{label}: target path {target_path:?} is not in imports"
                ));
                continue;
            }
            let Some(target) = dictionaries.get(target_path) else {
                errors.push(format!(
                    "{label}: target dictionary {target_path:?} is missing"
                ));
                continue;
            };
            if target.alias(target_name).is_some() {
                errors.push(format!(
                    "{label}: alias-to-alias target {:?} is forbidden",
                    alias.target
                ));
            } else if !target.points.contains(target_name) {
                errors.push(format!(
                    "{label}: concrete point {:?} is missing",
                    alias.target
                ));
            }
        }
    }
}

fn import_cycle_errors(dictionaries: &BTreeMap<String, PointDictionary>) -> Vec<String> {
    fn visit(
        path: &str,
        dictionaries: &BTreeMap<String, PointDictionary>,
        states: &mut BTreeMap<String, u8>,
        stack: &mut Vec<String>,
        cycles: &mut BTreeSet<Vec<String>>,
    ) {
        states.insert(path.to_string(), 1);
        stack.push(path.to_string());
        let mut targets: Vec<&str> = dictionaries[path]
            .imports
            .iter()
            .map(|import| import.path.as_str())
            .filter(|target| dictionaries.contains_key(*target))
            .collect();
        targets.sort();
        for target in targets {
            match states.get(target).copied().unwrap_or(0) {
                0 => visit(target, dictionaries, states, stack, cycles),
                1 => {
                    let start = stack.iter().position(|entry| entry == target).unwrap();
                    let members = stack[start..].to_vec();
                    let mut rotations = Vec::new();
                    for index in 0..members.len() {
                        let mut rotation = members[index..].to_vec();
                        rotation.extend_from_slice(&members[..index]);
                        rotations.push(rotation);
                    }
                    cycles.insert(rotations.into_iter().min().unwrap());
                }
                _ => {}
            }
        }
        stack.pop();
        states.insert(path.to_string(), 2);
    }

    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    let mut cycles = BTreeSet::new();
    for path in dictionaries.keys() {
        if states.get(path).copied().unwrap_or(0) == 0 {
            visit(path, dictionaries, &mut states, &mut stack, &mut cycles);
        }
    }
    cycles
        .into_iter()
        .map(|mut cycle| {
            cycle.push(cycle[0].clone());
            format!("points: import cycle: {}", cycle.join(" -> "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{PointResolver, ResolvedPoint};
    use serde_json::{Value, json};
    use std::path::Path;

    fn namespace() -> Value {
        json!({
            "brick": {"iri": "brick", "verified_version": "1"},
            "s223": {"iri": "s223", "verified_version": "1"},
            "quantitykind": {"iri": "quantitykind", "verified_version": "1"},
            "unit": {"iri": "unit", "verified_version": "1"}
        })
    }

    fn point(name: &str) -> Value {
        json!({
            "name": name,
            "description": "test point",
            "kind": "real",
            "unit": "1",
            "qudt_unit": null,
            "brick": null,
            "s223": null
        })
    }

    fn v1(equipment: &str, points: Vec<Value>) -> Value {
        json!({
            "schema": "cxf-library/points/v1",
            "equipment": equipment,
            "namespaces": namespace(),
            "points": points
        })
    }

    fn v2(equipment: &str, imports: Vec<&str>, aliases: Vec<Value>, points: Vec<Value>) -> Value {
        json!({
            "schema": "cxf-library/points/v2",
            "equipment": equipment,
            "namespaces": namespace(),
            "imports": imports,
            "aliases": aliases,
            "points": points
        })
    }

    fn documents(values: Vec<(&str, Value)>) -> Vec<(String, Vec<u8>)> {
        values
            .into_iter()
            .map(|(path, value)| (path.to_string(), serde_json::to_vec(&value).unwrap()))
            .collect()
    }

    fn assert_bad(values: Vec<(&str, Value)>, expected: &str) {
        let error = PointResolver::from_documents(documents(values))
            .err()
            .unwrap();
        assert!(
            error.contains(expected),
            "{expected:?} not found in {error:?}"
        );
    }

    #[test]
    fn production_aliases_and_local_points_resolve_to_canonical_records() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let resolver = PointResolver::load(&repo_root).expect("production point corpus");
        for (path, name, expected_path) in [
            (
                "points/vav.points.json",
                "zone_temp",
                "points/zone.points.json",
            ),
            (
                "points/vav.points.json",
                "zone_temp_sp_htg",
                "points/zone.points.json",
            ),
            (
                "points/vav.points.json",
                "zone_temp_sp_clg",
                "points/zone.points.json",
            ),
            (
                "points/sys.points.json",
                "occ_sensor",
                "points/zone.points.json",
            ),
            (
                "points/zone.points.json",
                "zone_temp",
                "points/zone.points.json",
            ),
            (
                "points/vav.points.json",
                "zone_airflow",
                "points/vav.points.json",
            ),
        ] {
            let resolved = resolver.resolve_bare(path, name).unwrap();
            assert_eq!(resolved.path, expected_path);
            assert_eq!(resolved.name, name);
        }
        assert_eq!(
            resolver
                .resolve_ref("points/vav.points.json#zone_temp")
                .unwrap(),
            ResolvedPoint {
                path: "points/zone.points.json".to_string(),
                name: "zone_temp".to_string(),
            }
        );
        assert!(
            resolver
                .resolve_bare("points/sys.points.json", "zone_temp")
                .is_err()
        );
    }

    #[test]
    fn versioned_shapes_and_duplicate_names_fail_closed() {
        let mut unexpected = v1("ahu", vec![point("sat")]);
        unexpected["aliases"] = json!([]);
        assert_bad(
            vec![("points/ahu.points.json", unexpected)],
            "unexpected key \"aliases\"",
        );

        let mut missing = v2("zone", vec![], vec![], vec![point("zone_temp")]);
        missing.as_object_mut().unwrap().remove("imports");
        assert_bad(
            vec![("points/zone.points.json", missing)],
            "missing required key \"imports\"",
        );

        let mut wrong_type = v2("zone", vec![], vec![], vec![point("zone_temp")]);
        wrong_type["aliases"] = json!({});
        assert_bad(
            vec![("points/zone.points.json", wrong_type)],
            "aliases must be an array",
        );

        assert_bad(
            vec![(
                "points/zone.points.json",
                v2(
                    "zone",
                    vec![],
                    vec![json!({"name": "zone_temp", "target": "points/ahu.points.json#sat"})],
                    vec![point("zone_temp")],
                ),
            )],
            "collides with local point",
        );
    }

    #[test]
    fn imports_reject_duplicates_bad_paths_self_missing_targets_and_cycles() {
        assert_bad(
            vec![
                (
                    "points/zone.points.json",
                    v2("zone", vec![], vec![], vec![point("zone_temp")]),
                ),
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec!["points/zone.points.json", "points/zone.points.json"],
                        vec![],
                        vec![point("zone_airflow")],
                    ),
                ),
            ],
            "duplicate \"points/zone.points.json\"",
        );
        for target in ["/points/zone.points.json", "points/../zone.points.json"] {
            assert_bad(
                vec![(
                    "points/zone.points.json",
                    v2("zone", vec![target], vec![], vec![]),
                )],
                "must be a root-relative",
            );
        }
        assert_bad(
            vec![(
                "points/zone.points.json",
                v2("zone", vec!["points/zone.points.json"], vec![], vec![]),
            )],
            "self-import is forbidden",
        );
        assert_bad(
            vec![(
                "points/zone.points.json",
                v2("zone", vec!["points/missing.points.json"], vec![], vec![]),
            )],
            "target \"points/missing.points.json\" is missing",
        );
        assert_bad(
            vec![
                (
                    "points/vav.points.json",
                    v2("vav", vec!["points/zone.points.json"], vec![], vec![]),
                ),
                (
                    "points/zone.points.json",
                    v2("zone", vec!["points/vav.points.json"], vec![], vec![]),
                ),
            ],
            "points: import cycle:",
        );
    }

    #[test]
    fn aliases_reject_duplicates_malformed_misimported_missing_and_chained_targets() {
        let zone = (
            "points/zone.points.json",
            v2("zone", vec![], vec![], vec![point("zone_temp")]),
        );
        assert_bad(
            vec![
                zone.clone(),
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec!["points/zone.points.json"],
                        vec![
                            json!({"name": "zone_temp", "target": "points/zone.points.json#zone_temp"}),
                            json!({"name": "zone_temp", "target": "points/zone.points.json#zone_temp"}),
                        ],
                        vec![],
                    ),
                ),
            ],
            "duplicate \"zone_temp\"",
        );
        assert_bad(
            vec![
                zone.clone(),
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec!["points/zone.points.json"],
                        vec![
                            json!({"name": "zone_temp", "target": "points/../zone.points.json#zone_temp"}),
                        ],
                        vec![],
                    ),
                ),
            ],
            "must be points/<family>.points.json#<name>",
        );
        assert_bad(
            vec![
                zone.clone(),
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec![],
                        vec![
                            json!({"name": "zone_temp", "target": "points/zone.points.json#zone_temp"}),
                        ],
                        vec![],
                    ),
                ),
            ],
            "is not in imports",
        );
        assert_bad(
            vec![
                zone,
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec!["points/zone.points.json"],
                        vec![
                            json!({"name": "zone_temp", "target": "points/zone.points.json#missing"}),
                        ],
                        vec![],
                    ),
                ),
            ],
            "concrete point \"points/zone.points.json#missing\" is missing",
        );

        assert_bad(
            vec![
                ("points/ahu.points.json", v1("ahu", vec![point("sat")])),
                (
                    "points/zone.points.json",
                    v2(
                        "zone",
                        vec!["points/ahu.points.json"],
                        vec![
                            json!({"name": "legacy_temp", "target": "points/ahu.points.json#sat"}),
                        ],
                        vec![],
                    ),
                ),
                (
                    "points/vav.points.json",
                    v2(
                        "vav",
                        vec!["points/zone.points.json"],
                        vec![
                            json!({"name": "zone_temp", "target": "points/zone.points.json#legacy_temp"}),
                        ],
                        vec![],
                    ),
                ),
            ],
            "alias-to-alias target",
        );
    }

    #[test]
    fn strict_json_rejects_duplicate_object_keys() {
        let error = PointResolver::from_documents(vec![(
            "points/zone.points.json".to_string(),
            br#"{"schema":"cxf-library/points/v2","schema":"cxf-library/points/v1"}"#.to_vec(),
        )])
        .err()
        .unwrap();
        assert!(error.contains("duplicate object key \"schema\""), "{error}");
    }
}
