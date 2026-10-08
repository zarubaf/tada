//! OpenAPI schemas from JSON Schemas: the input types of the `app` crate keep one shape for the API and MCP (ADR 0040).

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::Value;
use utoipa::openapi::RefOr;
use utoipa::openapi::extensions::Extensions;
use utoipa::openapi::schema::{Object, Schema};

/// The OpenAPI schema of `T`, from the JSON Schema that `schemars` derives for `T`.
///
/// OpenAPI 3.1 uses JSON Schema 2020-12, so the schema stays as it is, with all subschemas inline.
/// `utoipa` has no type for a raw JSON Schema, so the schema is the one item of an `allOf`.
/// One key keeps the generated document in a fixed order.
pub(crate) fn schema<T: JsonSchema>() -> RefOr<Schema> {
    let generator = SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    let mut json = generator.into_root_schema_for::<T>().to_value();
    if let Some(root) = json.as_object_mut() {
        root.remove("$schema");
        root.remove("title");
    }
    remove_defaults(&mut json);
    let mut object = Object::default();
    object.extensions = Some(Extensions::from_iter([("allOf", Value::Array(vec![json]))]));
    RefOr::T(Schema::Object(object))
}

/// Removes each `default` keyword. A client generator makes a property with a default a required one in the
/// type of the request, so a client would have to send `null` or an empty list. The server fills in the defaults.
fn remove_defaults(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            map.remove("default");
            for (key, value) in map.iter_mut() {
                if key == "properties" {
                    // The keys of `properties` are field names, not keywords.
                    if let Value::Object(properties) = value {
                        properties.values_mut().for_each(remove_defaults);
                    }
                } else {
                    remove_defaults(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(remove_defaults),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// A request with an optional field and a field with a default.
    #[derive(Deserialize, JsonSchema)]
    #[allow(dead_code)]
    struct Request {
        name: String,
        #[serde(default)]
        id: Option<uuid::Uuid>,
        #[serde(default)]
        default: Vec<String>,
    }

    /// The schema is the raw JSON Schema in an `allOf`, so a change of `utoipa` that drops the key fails here.
    #[test]
    fn the_schema_is_the_json_schema_without_defaults() {
        let schema = serde_json::to_value(schema::<Request>()).unwrap();
        let inner = &schema["allOf"][0];
        let properties = inner["properties"].as_object().unwrap();
        assert_eq!(
            properties.keys().collect::<Vec<_>>(),
            ["default", "id", "name"]
        );
        assert_eq!(inner["required"], serde_json::json!(["name"]));
        for property in properties.values() {
            assert!(property.get("default").is_none(), "{property}");
        }
    }
}
