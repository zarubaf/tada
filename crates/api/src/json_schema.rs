//! OpenAPI schemas from JSON Schemas: the input types of the `app` crate keep one shape for the API and MCP (ADR 0040).

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
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
    let mut object = Object::default();
    object.extensions = Some(Extensions::from_iter([(
        "allOf",
        serde_json::Value::Array(vec![json]),
    )]));
    RefOr::T(Schema::Object(object))
}
